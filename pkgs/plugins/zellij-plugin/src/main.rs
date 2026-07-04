use std::collections::BTreeMap;

use zellij_tile::prelude::*;

mod config;
mod render;
mod pane_size;
mod runtime;

use config::PluginConfig;
use pane_size::{PaneSizeConfig, PaneSizeService};
use render::{hitbox_at, ClickAction, Hitbox, RenderModel, Renderer};
use runtime::RuntimeState;

#[derive(Default)]
struct PluginState {
    runtime: RuntimeState,
    plugin_id: Option<u32>,
    config: PluginConfig,
    pane_size: PaneSizeService,
    hitboxes: Vec<Hitbox>,
}

register_plugin!(PluginState);

impl ZellijPlugin for PluginState {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.config = PluginConfig::parse(&configuration);
        set_selectable(true);
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::MessageAndLaunchOtherPlugins,
        ]);
        subscribe(&[
            EventType::Mouse,
            EventType::PaneClosed,
            EventType::PaneUpdate,
            EventType::PermissionRequestResult,
        ]);
        self.plugin_id = Some(get_plugin_ids().plugin_id);
        self.pane_size = PaneSizeService::new(PaneSizeConfig {
            collapsed_cols: self.config.collapsed_cols,
            expanded_cols: self.config.expanded_cols,
        });
        self.runtime.load();
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if let Some(collapsed) =
            self.pane_size
                .handle_pipe(&pipe_message, self.plugin_id, self.runtime.last_cols)
        {
            self.runtime.set_collapsed(collapsed);
            return true;
        }
        self.runtime.handle_pipe(pipe_message)
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.runtime.set_last_cols(cols);
        self.pane_size.set_current_cols(cols);
        let model = RenderModel::from_runtime(&self.runtime, &self.config.render);
        self.hitboxes = Renderer::render(&model, rows, cols);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::Mouse(Mouse::LeftClick(row, col)) => match hitbox_at(&self.hitboxes, row, col) {
                Some(ClickAction::ToggleCollapse) => {
                    let collapsed = self.runtime.toggle_collapsed();
                    self.pane_size
                        .local_toggle(self.plugin_id, collapsed, self.runtime.last_cols);
                    true
                }
                Some(ClickAction::SwitchTab { tab }) => {
                    switch_tab_to(tab);
                    false
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
                self.pane_size.sync_peers(self.plugin_id, pane_manifest);
                false
            }
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
