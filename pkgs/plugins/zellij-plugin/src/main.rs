use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
    time::Duration,
};

use zellij_tile::prelude::*;

mod config;
mod render;
mod runtime;

use config::PluginConfig;
use render::{
    error_frame, paint_frame, AgentRenderer, ClickAction, RenderModel, RenderedFrame, TemplateError,
};
use runtime::{PollCommand, RuntimeState, AGENT_PIPE_NAME, SUMMARY_PIPE_NAME};

const REFRESH_PIPE_NAME: &str = "agenthreads:refresh";
const TOGGLE_PIPE_NAME: &str = "agenthreads:toggle";

#[derive(Debug, Eq, PartialEq)]
enum ControlPipe {
    Refresh,
    Toggle,
}

const TIMER_BOUNDARY_PADDING: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TimerReason {
    render_refresh: bool,
    agent_expiry: bool,
    session_poll: bool,
    poll_timeout: bool,
    session_summary_expiry: bool,
}

impl TimerReason {
    const fn render_refresh() -> Self {
        Self {
            render_refresh: true,
            agent_expiry: false,
            session_poll: false,
            poll_timeout: false,
            session_summary_expiry: false,
        }
    }

    const fn agent_expiry() -> Self {
        Self {
            render_refresh: false,
            agent_expiry: true,
            session_poll: false,
            poll_timeout: false,
            session_summary_expiry: false,
        }
    }

    const fn session_poll() -> Self {
        Self {
            render_refresh: false,
            agent_expiry: false,
            session_poll: true,
            poll_timeout: false,
            session_summary_expiry: false,
        }
    }

    const fn poll_timeout() -> Self {
        Self {
            render_refresh: false,
            agent_expiry: false,
            session_poll: false,
            poll_timeout: true,
            session_summary_expiry: false,
        }
    }

    const fn session_summary_expiry() -> Self {
        Self {
            render_refresh: false,
            agent_expiry: false,
            session_poll: false,
            poll_timeout: false,
            session_summary_expiry: true,
        }
    }

    fn merge(&mut self, other: Self) {
        self.render_refresh |= other.render_refresh;
        self.agent_expiry |= other.agent_expiry;
        self.session_poll |= other.session_poll;
        self.poll_timeout |= other.poll_timeout;
        self.session_summary_expiry |= other.session_summary_expiry;
    }

    fn any(self) -> bool {
        self.render_refresh
            || self.agent_expiry
            || self.session_poll
            || self.poll_timeout
            || self.session_summary_expiry
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpiredTimer {
    reason: TimerReason,
    fired_at: Duration,
}

#[derive(Default)]
struct RefreshTimer {
    active: Option<Duration>,
    active_due_at: Duration,
    active_reason: TimerReason,
    active_started_at: Duration,
    superseded: Vec<Duration>,
}

impl RefreshTimer {
    fn schedule(
        &mut self,
        requested: Option<(Duration, TimerReason)>,
        now: Duration,
    ) -> Option<Duration> {
        let (requested, reason) = requested?;
        let requested = requested + TIMER_BOUNDARY_PADDING;
        let requested_due_at = now + requested;
        match self.active {
            None => {
                self.active = Some(requested);
                self.active_due_at = requested_due_at;
                self.active_reason = reason;
                self.active_started_at = now;
                Some(requested)
            }
            Some(active) if requested_due_at < self.active_due_at => {
                self.superseded.push(active);
                self.active = Some(requested);
                self.active_due_at = requested_due_at;
                self.active_reason = reason;
                self.active_started_at = now;
                Some(requested)
            }
            Some(_) if requested_due_at == self.active_due_at => {
                self.active_reason.merge(reason);
                None
            }
            Some(_) => None,
        }
    }

    fn expired(&mut self, elapsed_seconds: f64) -> Option<ExpiredTimer> {
        let Ok(elapsed) = Duration::try_from_secs_f64(elapsed_seconds) else {
            return None;
        };
        let Some(active) = self.active else {
            if let Some((index, _)) = self
                .superseded
                .iter()
                .enumerate()
                .min_by_key(|(_, duration)| duration.abs_diff(elapsed))
            {
                self.superseded.swap_remove(index);
            }
            return None;
        };
        let active_distance = active.abs_diff(elapsed);
        let stale = self
            .superseded
            .iter()
            .enumerate()
            .min_by_key(|(_, duration)| duration.abs_diff(elapsed));

        // ponytail: Zellij timers have no IDs or cancellation. Match their elapsed duration;
        // replace this with opaque timer IDs if Zellij adds them.
        if let Some((index, _)) =
            stale.filter(|(_, duration)| duration.abs_diff(elapsed) <= active_distance)
        {
            self.superseded.swap_remove(index);
            None
        } else {
            let expired = ExpiredTimer {
                reason: self.active_reason,
                fired_at: self.active_started_at + elapsed,
            };
            self.active = None;
            Some(expired)
        }
    }

    fn cancel_poll_timeout(&mut self, now: Duration) -> Option<Duration> {
        if !self.active_reason.poll_timeout {
            return None;
        }
        self.active_reason.poll_timeout = false;
        let remaining_reason = self.active_reason;
        if let Some(active) = self.active.take() {
            self.superseded.push(active);
        }
        if !remaining_reason.any() {
            return None;
        }
        let remaining = self.active_due_at.saturating_sub(now);
        self.active = Some(remaining);
        self.active_due_at = now + remaining;
        self.active_started_at = now;
        self.active_reason = remaining_reason;
        Some(remaining)
    }
}

#[derive(Default)]
struct PluginState {
    runtime: RuntimeState,
    mode_info: ModeInfo,
    plugin_id: Option<u32>,
    config: PluginConfig,
    frame: RenderedFrame,
    renderer: Option<AgentRenderer>,
    template_error: Option<TemplateError>,
    renderer_configuration: BTreeMap<String, String>,
    pending_template: Option<PendingTemplate>,
    last_pane_manifest: Option<PaneManifest>,
    refresh_timer: RefreshTimer,
    pending_permissions: VecDeque<PermissionRequestKind>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PermissionRequestKind {
    Base,
}

struct PendingTemplate {
    host_folder: PathBuf,
    configuration: BTreeMap<String, String>,
}

fn prepare_external_template(
    mut configuration: BTreeMap<String, String>,
    home: Option<&Path>,
    config_dir: Option<&Path>,
) -> Result<PendingTemplate, String> {
    let configured_path = configuration
        .get("template_file")
        .ok_or_else(|| "template_file is missing".to_string())?;
    let path = Path::new(configured_path);
    let path = if let Ok(relative) = path.strip_prefix("~") {
        home.ok_or_else(|| "cannot expand template path without a home directory".to_string())?
            .join(relative)
    } else if path.is_relative() {
        config_dir
            .map(Path::to_path_buf)
            .or_else(|| home.map(|home| home.join(".config/zellij")))
            .ok_or_else(|| "relative template_file requires ZELLIJ_CONFIG_DIR or HOME".to_string())?
            .join(path)
    } else {
        path.to_path_buf()
    };
    let host_folder = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| format!("template_file has no parent directory: {}", path.display()))?
        .to_path_buf();
    let entry = path
        .file_name()
        .ok_or_else(|| format!("template_file has no file name: {}", path.display()))?;

    // Mount the template directory, not host root. Host-side symlinks resolve before WASI
    // capability checks, and the renderer sees a stable guest-root entry path.
    configuration.insert(
        "template_file".to_string(),
        Path::new("/").join(entry).to_string_lossy().into_owned(),
    );
    Ok(PendingTemplate {
        host_folder,
        configuration,
    })
}

impl PluginState {
    fn initialize_renderer(&mut self) {
        match AgentRenderer::from_configuration(&self.renderer_configuration) {
            Ok(renderer) => {
                self.renderer = Some(renderer);
                self.template_error = None;
            }
            Err(error) => {
                self.renderer = None;
                self.template_error = Some(error);
            }
        }
    }

    fn schedule_next_timer(&mut self, render_refresh: Option<Duration>) {
        let mut next = None;
        for (delay, reason) in render_refresh
            .map(|delay| (delay, TimerReason::render_refresh()))
            .into_iter()
            .chain(
                self.runtime
                    .next_agent_expiry()
                    .map(|delay| (delay, TimerReason::agent_expiry())),
            )
            .chain(
                self.runtime
                    .next_session_poll()
                    .map(|delay| (delay, TimerReason::session_poll())),
            )
            .chain(
                self.runtime
                    .next_poll_timeout()
                    .map(|delay| (delay, TimerReason::poll_timeout())),
            )
            .chain(
                self.runtime
                    .next_session_summary_expiry()
                    .map(|delay| (delay, TimerReason::session_summary_expiry())),
            )
        {
            match &mut next {
                None => next = Some((delay, reason)),
                Some((current_delay, _)) if delay < *current_delay => {
                    next = Some((delay, reason));
                }
                Some((current_delay, current_reason)) if delay == *current_delay => {
                    current_reason.merge(reason);
                }
                Some(_) => {}
            }
        }
        if let Some(delay) = self.refresh_timer.schedule(next, self.runtime.lease_time()) {
            schedule_timeout(delay.as_secs_f64());
        }
    }

    fn run_next_poll_command(&mut self) {
        if let Some(command) = self.runtime.next_poll_command() {
            run_session_poll(&command);
            self.schedule_next_timer(None);
        }
    }

    fn cancel_active_poll_timeout(&mut self) {
        if let Some(delay) = self
            .refresh_timer
            .cancel_poll_timeout(self.runtime.lease_time())
        {
            schedule_timeout(delay.as_secs_f64());
        }
    }

    fn template_unavailable_error(&self) -> TemplateError {
        if let Some(pending_template) = &self.pending_template {
            if self
                .pending_permissions
                .iter()
                .any(|request| *request == PermissionRequestKind::Base)
            {
                return template_config_error(format!(
                    "template_file is waiting for FullHdAccess permission to mount {}",
                    pending_template.host_folder.display()
                ));
            }
            return template_config_error(format!(
                "template_file permission granted; waiting for Zellij to mount {} as /host",
                pending_template.host_folder.display()
            ));
        }

        template_config_error(
            "template renderer unavailable; no renderer, error, or pending template",
        )
    }
}

register_plugin!(PluginState);

impl ZellijPlugin for PluginState {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.config = PluginConfig::parse(&configuration);
        self.renderer_configuration = configuration.clone();
        set_selectable(true);
        // Zellij rewrites the permission cache with the exact requested set, even for cached grants.
        // Keep this list stable so a launch without template_file does not drop FullHdAccess.
        let permissions = vec![
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
            PermissionType::RunCommands,
            PermissionType::FullHdAccess,
        ];
        let has_template_file = configuration.contains_key("template_file");
        let has_conflicting_template = has_template_file && configuration.contains_key("template");
        if has_template_file && !has_conflicting_template {
            match prepare_external_template(
                configuration,
                std::env::var_os("HOME").as_deref().map(Path::new),
                std::env::var_os("ZELLIJ_CONFIG_DIR")
                    .as_deref()
                    .map(Path::new),
            ) {
                Ok(pending_template) => self.pending_template = Some(pending_template),
                Err(error) => {
                    self.renderer = None;
                    self.template_error = Some(template_config_error(error));
                }
            }
        } else {
            self.initialize_renderer();
        }
        subscribe(&[
            EventType::Mouse,
            EventType::ModeUpdate,
            EventType::PaneClosed,
            EventType::PaneUpdate,
            EventType::TabUpdate,
            EventType::SessionUpdate,
            EventType::Timer,
            EventType::RunCommandResult,
            EventType::PermissionRequestResult,
            EventType::HostFolderChanged,
            EventType::FailedToChangeHostFolder,
        ]);
        self.pending_permissions
            .push_back(PermissionRequestKind::Base);
        request_permission(&permissions);
        self.plugin_id = Some(get_plugin_ids().plugin_id);
        self.runtime.load();
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if pipe_message.name == AGENT_PIPE_NAME {
            let changed = self.runtime.handle_pipe(pipe_message);
            self.schedule_next_timer(None);
            return changed;
        }

        if pipe_message.name == SUMMARY_PIPE_NAME {
            if matches!(pipe_message.source, PipeSource::Cli(_)) {
                if let Some(output) = self
                    .runtime
                    .session_summary_output(pipe_message.payload.as_deref())
                {
                    send_cli_pipe_output(SUMMARY_PIPE_NAME, &output);
                }
            }
            return false;
        }

        match control_pipe(&pipe_message.name) {
            Some(ControlPipe::Refresh) => {
                if let Some(plugin_id) = self.plugin_id {
                    reload_self(plugin_id);
                }
            }
            Some(ControlPipe::Toggle) => {
                let is_suppressed = self.plugin_id.and_then(|plugin_id| {
                    self.last_pane_manifest
                        .as_ref()
                        .and_then(|manifest| plugin_is_suppressed(manifest, plugin_id))
                });
                set_self_visible(is_suppressed != Some(false));
            }
            _ => {}
        }
        false
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let model = RenderModel::from_runtime(&self.runtime, &self.config.render);
        self.frame = if let Some(renderer) = &mut self.renderer {
            match renderer.render(&self.mode_info, &model, rows, cols) {
                Ok(frame) => frame,
                Err(error) => renderer.error_frame(&error, rows, cols),
            }
        } else if let Some(error) = &self.template_error {
            error_frame(error, rows, cols)
        } else {
            error_frame(&self.template_unavailable_error(), rows, cols)
        };
        self.schedule_next_timer(self.frame.refresh_after);
        paint_frame(&self.frame, rows, cols);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::ModeUpdate(mode_info) => {
                let changed = self.mode_info != mode_info;
                self.mode_info = mode_info;
                changed
            }
            Event::Mouse(Mouse::LeftClick(row, col)) => match usize::try_from(row)
                .ok()
                .and_then(|row| self.frame.hitboxes.get(row))
                .and_then(|line| line.get(col))
                .and_then(Clone::clone)
            {
                Some(ClickAction::SwitchTab { tab }) => {
                    switch_tab_to(tab);
                    true
                }
                Some(ClickAction::SwitchToSession { session }) => {
                    switch_session(Some(&session));
                    true
                }
                Some(ClickAction::FocusPane { pane }) => {
                    if let Some(pane_id) = parse_pane_id(&pane) {
                        focus_pane_with_id(pane_id, false, false);
                    }
                    false
                }
                None => false,
            },
            Event::PaneClosed(pane_id) => {
                self.runtime.remove_agents_for_pane(pane_id);
                true
            }
            Event::PaneUpdate(pane_manifest) => {
                let focus_changed = self.runtime.sync_pane_focus(&pane_manifest);
                self.last_pane_manifest = Some(pane_manifest.clone());
                focus_changed
            }
            Event::TabUpdate(tabs) => {
                let tab_changed = self.runtime.sync_tabs(&tabs);
                let focus_changed = match self.last_pane_manifest.as_ref() {
                    Some(manifest) => self.runtime.sync_pane_focus(manifest),
                    None => false,
                };
                tab_changed || focus_changed
            }
            Event::SessionUpdate(sessions, _) => {
                let outcome = self.runtime.sync_zellij_sessions_with_outcome(&sessions);
                if outcome.canceled_active_poll {
                    self.cancel_active_poll_timeout();
                }
                self.run_next_poll_command();
                outcome.changed
            }
            Event::Timer(elapsed) => {
                let Some(expired) = self.refresh_timer.expired(elapsed) else {
                    return false;
                };
                self.runtime.advance_lease_clock_to(expired.fired_at);
                if expired.reason.session_poll {
                    self.runtime.begin_session_poll_cycle();
                }
                let removed_agents = if expired.reason.agent_expiry {
                    self.runtime.expire_silent_agents() > 0
                } else {
                    false
                };
                let removed_summaries = if expired.reason.session_summary_expiry {
                    self.runtime.expire_session_summaries() > 0
                } else {
                    false
                };
                let poll_failed = if expired.reason.poll_timeout {
                    self.runtime.poll_timed_out()
                } else {
                    false
                };
                self.run_next_poll_command();
                self.schedule_next_timer(None);
                expired.reason.render_refresh || removed_agents || removed_summaries || poll_failed
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                let changed = self
                    .runtime
                    .handle_poll_result(exit_code, &stdout, &stderr, &context);
                if changed {
                    self.cancel_active_poll_timeout();
                }
                self.run_next_poll_command();
                self.schedule_next_timer(None);
                changed
            }
            Event::PermissionRequestResult(status) => {
                let request = finish_pending_permission(&mut self.pending_permissions);
                match request {
                    PermissionRequestKind::Base => {
                        if self.pending_template.is_some() {
                            match status {
                                PermissionStatus::Granted => change_host_folder(
                                    self.pending_template
                                        .as_ref()
                                        .expect("pending template checked above")
                                        .host_folder
                                        .clone(),
                                ),
                                PermissionStatus::Denied => {
                                    self.pending_template = None;
                                    self.renderer = None;
                                    self.template_error = Some(template_config_error(
                                        "template_file requires FullHdAccess permission",
                                    ));
                                }
                            }
                        }
                        let enabled = status == PermissionStatus::Granted;
                        self.runtime.set_session_polling_enabled(enabled);
                        if !enabled {
                            self.cancel_active_poll_timeout();
                        }
                        self.run_next_poll_command();
                    }
                }
                if self.pending_permissions.is_empty() {
                    set_selectable(false);
                }
                true
            }
            Event::HostFolderChanged(_) => {
                if let Some(pending_template) = self.pending_template.take() {
                    self.renderer_configuration = pending_template.configuration;
                    self.initialize_renderer();
                    true
                } else {
                    false
                }
            }
            Event::FailedToChangeHostFolder(error) if self.pending_template.take().is_some() => {
                self.renderer = None;
                self.template_error = Some(template_config_error(
                    error.unwrap_or_else(|| "failed to mount host filesystem".into()),
                ));
                true
            }
            _ => false,
        }
    }
}

fn template_config_error(message: impl Into<String>) -> TemplateError {
    TemplateError::new(
        zellij_template_render::ErrorKind::InvalidOperation,
        message.into(),
    )
}

fn parse_pane_id(value: &str) -> Option<PaneId> {
    if let Some(id) = value.strip_prefix("terminal_") {
        return id.parse().ok().map(PaneId::Terminal);
    }
    if let Some(id) = value.strip_prefix("plugin_") {
        return id.parse().ok().map(PaneId::Plugin);
    }
    value.parse().ok().map(PaneId::Terminal)
}

fn finish_pending_permission(
    pending_permissions: &mut VecDeque<PermissionRequestKind>,
) -> PermissionRequestKind {
    pending_permissions
        .pop_front()
        .unwrap_or(PermissionRequestKind::Base)
}

fn schedule_timeout(seconds: f64) {
    #[cfg(target_arch = "wasm32")]
    set_timeout(seconds);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = seconds;
}

fn send_cli_pipe_output(pipe_name: &str, output: &str) {
    #[cfg(target_arch = "wasm32")]
    cli_pipe_output(pipe_name, output);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (pipe_name, output);
}

fn run_session_poll(command: &PollCommand) {
    #[cfg(target_arch = "wasm32")]
    {
        let args = [
            "zellij",
            "--session",
            command.session_name.as_str(),
            "pipe",
            "--name",
            SUMMARY_PIPE_NAME,
            "--",
            command.payload.as_str(),
        ];
        run_command(&args, command.context());
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = command;
}

fn reload_self(plugin_id: u32) {
    #[cfg(target_arch = "wasm32")]
    reload_plugin_with_id(plugin_id);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = plugin_id;
}

fn set_self_visible(visible: bool) {
    #[cfg(target_arch = "wasm32")]
    if visible {
        show_self(false);
    } else {
        hide_self();
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = visible;
}

fn control_pipe(name: &str) -> Option<ControlPipe> {
    match name {
        REFRESH_PIPE_NAME => Some(ControlPipe::Refresh),
        TOGGLE_PIPE_NAME => Some(ControlPipe::Toggle),
        _ => None,
    }
}

fn plugin_is_suppressed(manifest: &PaneManifest, plugin_id: u32) -> Option<bool> {
    manifest
        .panes
        .values()
        .flatten()
        .find(|pane| pane.is_plugin && pane.id == plugin_id)
        .map(|pane| pane.is_suppressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template_config(path: &str) -> BTreeMap<String, String> {
        BTreeMap::from([("template_file".to_string(), path.to_string())])
    }

    fn schedule_render(
        timer: &mut RefreshTimer,
        delay: Duration,
        now: Duration,
    ) -> Option<Duration> {
        timer.schedule(Some((delay, TimerReason::render_refresh())), now)
    }

    fn schedule_expiry(
        timer: &mut RefreshTimer,
        delay: Duration,
        now: Duration,
    ) -> Option<Duration> {
        timer.schedule(Some((delay, TimerReason::agent_expiry())), now)
    }

    #[test]
    fn faster_refresh_supersedes_armed_timer() {
        let mut timer = RefreshTimer::default();

        assert_eq!(
            schedule_render(&mut timer, Duration::from_secs(1), Duration::ZERO),
            Some(Duration::from_millis(1_010))
        );
        assert_eq!(
            schedule_render(&mut timer, Duration::from_millis(125), Duration::ZERO),
            Some(Duration::from_millis(135))
        );
        assert_eq!(
            schedule_render(&mut timer, Duration::from_millis(500), Duration::ZERO),
            None
        );
        assert_eq!(timer.active, Some(Duration::from_millis(135)));
    }

    #[test]
    fn superseded_timer_does_not_start_second_render_loop() {
        let mut timer = RefreshTimer::default();
        schedule_render(&mut timer, Duration::from_secs(1), Duration::ZERO);
        schedule_render(&mut timer, Duration::from_millis(125), Duration::ZERO);

        assert!(timer.expired(1.01).is_none());
        assert_eq!(timer.active, Some(Duration::from_millis(135)));
        assert_eq!(
            timer.expired(0.135).map(|expired| expired.reason),
            Some(TimerReason::render_refresh())
        );
        assert_eq!(timer.active, None);
    }

    #[test]
    fn equal_refresh_does_not_arm_duplicate_timer() {
        let mut timer = RefreshTimer::default();
        assert!(schedule_render(&mut timer, Duration::from_millis(125), Duration::ZERO).is_some());
        assert_eq!(
            schedule_render(&mut timer, Duration::from_millis(125), Duration::ZERO),
            None
        );
    }

    #[test]
    fn equal_timers_merge_reasons() {
        let mut timer = RefreshTimer::default();
        assert!(schedule_render(&mut timer, Duration::from_secs(1), Duration::ZERO).is_some());
        assert_eq!(
            schedule_expiry(&mut timer, Duration::from_secs(1), Duration::ZERO),
            None
        );

        assert_eq!(
            timer.expired(1.01).map(|expired| expired.reason),
            Some(TimerReason {
                render_refresh: true,
                agent_expiry: true,
                ..TimerReason::default()
            })
        );
    }

    #[test]
    fn cancel_poll_timeout_supersedes_poll_only_timer() {
        let mut timer = RefreshTimer::default();
        assert_eq!(
            timer.schedule(
                Some((Duration::from_secs(3), TimerReason::poll_timeout())),
                Duration::ZERO,
            ),
            Some(Duration::from_millis(3_010))
        );

        assert_eq!(timer.cancel_poll_timeout(Duration::ZERO), None);

        assert_eq!(timer.active, None);
        assert!(timer.expired(3.01).is_none());
    }

    #[test]
    fn canceled_poll_timeout_does_not_fire_for_next_equal_timeout() {
        let mut timer = RefreshTimer::default();
        assert!(timer
            .schedule(
                Some((Duration::from_secs(3), TimerReason::poll_timeout())),
                Duration::ZERO,
            )
            .is_some());
        assert_eq!(timer.cancel_poll_timeout(Duration::ZERO), None);
        assert!(timer
            .schedule(
                Some((Duration::from_secs(3), TimerReason::poll_timeout())),
                Duration::from_millis(500),
            )
            .is_some());

        assert!(timer.expired(3.01).is_none());
        assert_eq!(
            timer.expired(3.01).map(|expired| expired.reason),
            Some(TimerReason::poll_timeout())
        );
    }

    #[test]
    fn cancel_poll_timeout_keeps_other_timer_reasons() {
        let mut timer = RefreshTimer::default();
        assert!(timer
            .schedule(
                Some((Duration::from_secs(3), TimerReason::poll_timeout())),
                Duration::ZERO,
            )
            .is_some());
        assert_eq!(
            timer.schedule(
                Some((Duration::from_secs(3), TimerReason::render_refresh())),
                Duration::ZERO,
            ),
            None
        );

        assert_eq!(
            timer.cancel_poll_timeout(Duration::ZERO),
            Some(Duration::from_millis(3_010))
        );

        assert!(timer.expired(3.01).is_none());
        assert_eq!(
            timer.expired(3.01).map(|expired| expired.reason),
            Some(TimerReason::render_refresh())
        );
    }

    #[test]
    fn expiry_timer_reuses_refresh_timer_identity() {
        let mut timer = RefreshTimer::default();

        assert_eq!(
            schedule_expiry(&mut timer, Duration::from_secs(10), Duration::ZERO),
            Some(Duration::from_millis(10_010))
        );
        assert_eq!(
            schedule_expiry(&mut timer, Duration::from_secs(10), Duration::from_secs(2)),
            None
        );
        assert_eq!(
            timer.expired(10.01).map(|expired| expired.reason),
            Some(TimerReason::agent_expiry())
        );
    }

    #[test]
    fn recognizes_control_pipe_names() {
        assert_eq!(
            control_pipe("agenthreads:refresh"),
            Some(ControlPipe::Refresh)
        );
        assert_eq!(
            control_pipe("agenthreads:toggle"),
            Some(ControlPipe::Toggle)
        );
        assert_eq!(control_pipe("unknown"), None);
    }

    #[test]
    fn finds_plugin_suppression_state() {
        let manifest = PaneManifest {
            panes: std::collections::HashMap::from([(
                0,
                vec![PaneInfo {
                    id: 42,
                    is_plugin: true,
                    is_suppressed: true,
                    ..PaneInfo::default()
                }],
            )]),
        };

        assert_eq!(plugin_is_suppressed(&manifest, 42), Some(true));
        assert_eq!(plugin_is_suppressed(&manifest, 7), None);
    }

    #[test]
    fn permission_queue_reports_base_request() {
        let mut pending_permissions = VecDeque::from([PermissionRequestKind::Base]);

        assert_eq!(
            finish_pending_permission(&mut pending_permissions),
            PermissionRequestKind::Base
        );
        assert!(pending_permissions.is_empty());
    }

    #[test]
    fn pending_external_template_reports_waiting_step() {
        let pending_template = PendingTemplate {
            host_folder: PathBuf::from("/var/home/q/.config/zellij/plugins/agent-threads"),
            configuration: template_config("/main.jinja"),
        };
        let state = PluginState {
            pending_template: Some(pending_template),
            pending_permissions: VecDeque::from([PermissionRequestKind::Base]),
            ..PluginState::default()
        };

        assert!(state
            .template_unavailable_error()
            .to_string()
            .contains("waiting for FullHdAccess permission"));

        let pending_template = PendingTemplate {
            host_folder: PathBuf::from("/var/home/q/.config/zellij/plugins/agent-threads"),
            configuration: template_config("/main.jinja"),
        };
        let state = PluginState {
            pending_template: Some(pending_template),
            pending_permissions: VecDeque::new(),
            ..PluginState::default()
        };

        assert!(state
            .template_unavailable_error()
            .to_string()
            .contains("waiting for Zellij to mount"));
    }

    #[test]
    fn external_template_mounts_parent_and_uses_guest_root_entry() {
        let pending = prepare_external_template(
            template_config("~/.config/zellij/agent-threads/main.jinja"),
            Some(Path::new("/var/home/q")),
            None,
        )
        .unwrap();

        assert_eq!(
            pending.host_folder,
            PathBuf::from("/var/home/q/.config/zellij/agent-threads")
        );
        assert_eq!(
            pending
                .configuration
                .get("template_file")
                .map(String::as_str),
            Some("/main.jinja")
        );
    }

    #[test]
    fn dot_relative_template_uses_zellij_config_dir() {
        let pending = prepare_external_template(
            template_config("./agent-threads/main.jinja"),
            Some(Path::new("/var/home/q")),
            Some(Path::new("/etc/zellij")),
        )
        .unwrap();

        assert_eq!(
            pending.host_folder,
            PathBuf::from("/etc/zellij/./agent-threads")
        );
        assert_eq!(
            pending
                .configuration
                .get("template_file")
                .map(String::as_str),
            Some("/main.jinja")
        );
    }

    #[test]
    fn relative_template_falls_back_to_home_zellij_config() {
        let pending = prepare_external_template(
            template_config("./agent-threads/main.jinja"),
            Some(Path::new("/var/home/q")),
            None,
        )
        .unwrap();

        assert_eq!(
            pending.host_folder,
            PathBuf::from("/var/home/q/.config/zellij/./agent-threads")
        );
        assert_eq!(
            pending
                .configuration
                .get("template_file")
                .map(String::as_str),
            Some("/main.jinja")
        );
    }

    #[test]
    fn absolute_template_mounts_its_parent() {
        let pending = prepare_external_template(
            template_config("/opt/zellij/templates/main.jinja"),
            Some(Path::new("/var/home/q")),
            Some(Path::new("/etc/zellij")),
        )
        .unwrap();

        assert_eq!(pending.host_folder, PathBuf::from("/opt/zellij/templates"));
        assert_eq!(
            pending
                .configuration
                .get("template_file")
                .map(String::as_str),
            Some("/main.jinja")
        );
    }

    #[test]
    fn relative_template_requires_config_dir_or_home() {
        let error = prepare_external_template(template_config("main.jinja"), None, None)
            .err()
            .unwrap();

        assert_eq!(
            error,
            "relative template_file requires ZELLIJ_CONFIG_DIR or HOME"
        );
    }
}
