use std::{
    collections::BTreeMap,
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
use runtime::{RuntimeState, AGENT_PIPE_NAME};

const REFRESH_PIPE_NAME: &str = "agenthreads:refresh";
const TOGGLE_PIPE_NAME: &str = "agenthreads:toggle";

#[derive(Debug, Eq, PartialEq)]
enum ControlPipe {
    Refresh,
    Toggle,
}

const TIMER_BOUNDARY_PADDING: Duration = Duration::from_millis(10);

#[derive(Default)]
struct RefreshTimer {
    active: Option<Duration>,
    superseded: Vec<Duration>,
}

impl RefreshTimer {
    fn schedule(&mut self, requested: Option<Duration>) -> Option<Duration> {
        let requested = requested? + TIMER_BOUNDARY_PADDING;
        match self.active {
            None => {
                self.active = Some(requested);
                Some(requested)
            }
            Some(active) if requested < active => {
                self.superseded.push(active);
                self.active = Some(requested);
                Some(requested)
            }
            Some(_) => None,
        }
    }

    fn expired(&mut self, elapsed_seconds: f64) -> bool {
        let Ok(elapsed) = Duration::try_from_secs_f64(elapsed_seconds) else {
            return false;
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
            return false;
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
            stale.filter(|(_, duration)| duration.abs_diff(elapsed) < active_distance)
        {
            self.superseded.swap_remove(index);
            false
        } else {
            self.active = None;
            true
        }
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
}

register_plugin!(PluginState);

impl ZellijPlugin for PluginState {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.config = PluginConfig::parse(&configuration);
        self.renderer_configuration = configuration.clone();
        set_selectable(true);
        let mut permissions = vec![
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
        ];
        let has_template_file = configuration.contains_key("template_file");
        let has_conflicting_template = has_template_file && configuration.contains_key("template");
        if has_template_file && !has_conflicting_template {
            permissions.push(PermissionType::FullHdAccess);
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
            EventType::PermissionRequestResult,
            EventType::HostFolderChanged,
            EventType::FailedToChangeHostFolder,
        ]);
        request_permission(&permissions);
        self.plugin_id = Some(get_plugin_ids().plugin_id);
        self.runtime.load();
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if pipe_message.name == AGENT_PIPE_NAME {
            return self.runtime.handle_pipe(pipe_message);
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
            let error = TemplateError::new(
                zellij_template_render::ErrorKind::InvalidOperation,
                "template renderer unavailable",
            );
            error_frame(&error, rows, cols)
        };
        if let Some(delay) = self.refresh_timer.schedule(self.frame.refresh_after) {
            set_timeout(delay.as_secs_f64());
        }
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
            Event::SessionUpdate(sessions, _) => self.runtime.sync_zellij_sessions(&sessions),
            Event::Timer(elapsed) => self.refresh_timer.expired(elapsed),
            Event::PermissionRequestResult(status) => {
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
                set_selectable(false);
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

    #[test]
    fn faster_refresh_supersedes_armed_timer() {
        let mut timer = RefreshTimer::default();

        assert_eq!(
            timer.schedule(Some(Duration::from_secs(1))),
            Some(Duration::from_millis(1_010))
        );
        assert_eq!(
            timer.schedule(Some(Duration::from_millis(125))),
            Some(Duration::from_millis(135))
        );
        assert_eq!(timer.schedule(Some(Duration::from_millis(500))), None);
        assert_eq!(timer.active, Some(Duration::from_millis(135)));
    }

    #[test]
    fn superseded_timer_does_not_start_second_render_loop() {
        let mut timer = RefreshTimer::default();
        timer.schedule(Some(Duration::from_secs(1)));
        timer.schedule(Some(Duration::from_millis(125)));

        assert!(!timer.expired(1.01));
        assert_eq!(timer.active, Some(Duration::from_millis(135)));
        assert!(timer.expired(0.135));
        assert_eq!(timer.active, None);
    }

    #[test]
    fn equal_refresh_does_not_arm_duplicate_timer() {
        let mut timer = RefreshTimer::default();
        assert!(timer.schedule(Some(Duration::from_millis(125))).is_some());
        assert_eq!(timer.schedule(Some(Duration::from_millis(125))), None);
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
