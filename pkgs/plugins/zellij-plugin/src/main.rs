use std::{collections::BTreeMap, path::PathBuf};

use zellij_tile::prelude::*;

mod config;
mod render;
mod runtime;

use config::PluginConfig;
use render::{
    error_frame, paint_frame, AgentRenderer, ClickAction, RenderModel, RenderedFrame, TemplateError,
};
use runtime::RuntimeState;

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
    last_pane_manifest: Option<PaneManifest>,
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
        if !configuration.contains_key("template_file") {
            self.initialize_renderer();
        }
        set_selectable(true);
        let mut permissions = vec![
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
        ];
        if configuration.contains_key("template_file") {
            permissions.push(PermissionType::FullHdAccess);
        }
        subscribe(&[
            EventType::Mouse,
            EventType::ModeUpdate,
            EventType::PaneClosed,
            EventType::PaneUpdate,
            EventType::TabUpdate,
            EventType::SessionUpdate,
            EventType::PermissionRequestResult,
            EventType::HostFolderChanged,
            EventType::FailedToChangeHostFolder,
        ]);
        request_permission(&permissions);
        self.plugin_id = Some(get_plugin_ids().plugin_id);
        self.runtime.load();
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        self.runtime.handle_pipe(pipe_message)
    }

    fn render(&mut self, rows: usize, cols: usize) {
        let model = RenderModel::from_runtime(&self.runtime, &self.config.render);
        self.frame = if let Some(renderer) = &mut self.renderer {
            match renderer.render(&self.mode_info, &model, rows, cols) {
                Ok(frame) => frame,
                Err(error) => error_frame(&error, rows, cols),
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
                Some(ClickAction::FocusPane { pane }) => {
                    if let Some(pane_id) = parse_pane_id(&pane) {
                        focus_pane_with_id(pane_id, false, false);
                    }
                    false
                }
                None => false,
            },
            Event::PaneClosed(pane_id) => {
                self.runtime.remove_sessions_for_pane(pane_id);
                true
            }
            Event::PaneUpdate(pane_manifest) => {
                let focus_changed = self.runtime.sync_pane_focus(&pane_manifest);
                self.last_pane_manifest = Some(pane_manifest.clone());
                focus_changed
            }
            Event::TabUpdate(tabs) => {
                let tab_changed = self.runtime.sync_active_tab(&tabs);
                let focus_changed = match self.last_pane_manifest.as_ref() {
                    Some(manifest) => self.runtime.sync_pane_focus(manifest),
                    None => false,
                };
                tab_changed || focus_changed
            }
            Event::SessionUpdate(sessions, _) => self.runtime.sync_current_session(&sessions),
            Event::PermissionRequestResult(status) => {
                if self.renderer_configuration.contains_key("template_file") {
                    match status {
                        PermissionStatus::Granted => change_host_folder(PathBuf::from("/")),
                        PermissionStatus::Denied => {
                            self.renderer = None;
                            self.template_error = Some(TemplateError::new(
                                zellij_template_render::ErrorKind::InvalidOperation,
                                "template_file requires FullHdAccess permission",
                            ));
                        }
                    }
                }
                set_selectable(false);
                true
            }
            Event::HostFolderChanged(_) => {
                if self.renderer_configuration.contains_key("template_file") {
                    self.initialize_renderer();
                    true
                } else {
                    false
                }
            }
            Event::FailedToChangeHostFolder(error)
                if self.renderer_configuration.contains_key("template_file") =>
            {
                self.renderer = None;
                self.template_error = Some(TemplateError::new(
                    zellij_template_render::ErrorKind::InvalidOperation,
                    error.unwrap_or_else(|| "failed to mount host filesystem".into()),
                ));
                true
            }
            _ => false,
        }
    }
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
