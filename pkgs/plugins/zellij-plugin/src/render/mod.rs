//! Builds and paints the plugin UI.

mod model;
mod template;

pub(crate) use model::RenderModel;
pub(crate) use template::{error_frame, AgentRenderer, ClickAction, RenderedFrame, TemplateError};

use zellij_tile::prelude::*;

pub(crate) fn paint_frame(frame: &RenderedFrame, rows: usize, cols: usize) {
    if rows == 0 || cols == 0 {
        return;
    }

    clear_plugin_rows(rows, cols);
    for (row, line) in frame.lines.iter().take(rows).enumerate() {
        print_text_with_coordinates(Text::new(line), 0, row, Some(cols), Some(1));
    }
}

fn clear_plugin_rows(rows: usize, cols: usize) {
    let blank = " ".repeat(cols);
    for row in 0..rows {
        print_text_with_coordinates(
            Text::new(blank.clone()).opaque(),
            0,
            row,
            Some(cols),
            Some(1),
        );
    }
}
