use std::collections::BTreeMap;

use zellij_tile::prelude::*;

mod config;
mod render;
mod runtime;

use config::PluginConfig;
use render::{is_collapse_button_click, RenderModel, Renderer};
use runtime::RuntimeState;

#[derive(Default)]
struct PluginState {
    runtime: RuntimeState,
    plugin_id: Option<u32>,
    config: PluginConfig,
}

register_plugin!(PluginState);

impl ZellijPlugin for PluginState {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.config = PluginConfig::parse(&configuration);
        request_permission(&[PermissionType::ChangeApplicationState]);
        subscribe(&[EventType::Mouse, EventType::PaneClosed]);
        set_selectable(true);
        self.plugin_id = Some(get_plugin_ids().plugin_id);
        self.runtime.load();
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        self.runtime.handle_pipe(pipe_message)
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.runtime.set_last_cols(cols);
        let model = RenderModel::from_runtime(&self.runtime, &self.config.render);
        Renderer::render(&model, rows, cols);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::Mouse(Mouse::LeftClick(row, col))
                if is_collapse_button_click(
                    row,
                    col,
                    self.runtime.last_cols,
                    self.runtime.collapsed,
                ) =>
            {
                let collapsed = self.runtime.toggle_collapsed();
                resize_plugin_pane(self.plugin_id, collapsed, self.config.resize_steps);
                true
            }
            Event::PaneClosed(pane_id) => {
                self.runtime.remove_sessions_for_pane(pane_id);
                true
            }
            _ => false,
        }
    }
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
