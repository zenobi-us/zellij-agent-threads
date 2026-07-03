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
}

register_plugin!(PluginState);

impl ZellijPlugin for PluginState {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.config = PluginConfig::parse(&configuration);
        request_permission(&[PermissionType::ChangeApplicationState]);
        subscribe(&[EventType::Mouse, EventType::PaneClosed]);
        set_selectable(false);
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
                Some(ClickAction::ToggleCollapse) => {
                    let collapsed = self.runtime.toggle_collapsed();
                    resize_plugin_pane(self.plugin_id, collapsed, self.config.resize_steps);
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

fn resize_plugin_pane(plugin_id: Option<u32>, collapsed: bool, resize_steps: usize) {
    let Some(plugin_id) = plugin_id else { return };
    let resize = if collapsed {
        Resize::Decrease
    } else {
        Resize::Increase
    };
    for _ in 0..resize_steps {
        resize_pane_with_id(ResizeStrategy::new(resize, None), PaneId::Plugin(plugin_id));
    }
}
