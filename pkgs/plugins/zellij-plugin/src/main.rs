use std::collections::BTreeMap;

use zellij_tile::prelude::*;

mod config;
mod render;
mod runtime;

use config::PluginConfig;
use render::{hitbox_at, ClickAction, Hitbox, RenderModel, Renderer};
use runtime::RuntimeState;

#[derive(Default)]
struct PluginState {
    runtime: RuntimeState,
    plugin_id: Option<u32>,
    config: PluginConfig,
    hitboxes: Vec<Hitbox>,
    last_pane_manifest: Option<PaneManifest>,
}

register_plugin!(PluginState);

impl ZellijPlugin for PluginState {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.config = PluginConfig::parse(&configuration);
        set_selectable(true);
        let mut permissions = vec![
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
        ];
        if self.config.render.template_dir.is_some() {
            permissions.push(PermissionType::FullHdAccess);
        }
        request_permission(&permissions);
        subscribe(&[
            EventType::Mouse,
            EventType::PaneClosed,
            EventType::PaneUpdate,
            EventType::TabUpdate,
            EventType::SessionUpdate,
            EventType::PermissionRequestResult,
        ]);
        self.plugin_id = Some(get_plugin_ids().plugin_id);
        self.runtime.load();
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        self.runtime.handle_pipe(pipe_message)
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.runtime.set_last_cols(cols);
        let model = RenderModel::from_runtime(&self.runtime, &self.config.render);
        self.hitboxes = Renderer::render(&model, rows, cols);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::Mouse(Mouse::LeftClick(row, col)) => match hitbox_at(&self.hitboxes, row, col) {
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
            Event::PermissionRequestResult(_) => {
                set_selectable(false);
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
