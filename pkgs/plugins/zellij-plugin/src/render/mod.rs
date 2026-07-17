//! Builds and paints the plugin UI.

mod model;
mod template;

pub(crate) use model::RenderModel;
pub(crate) use template::{error_frame, AgentRenderer, ClickAction, RenderedFrame, TemplateError};

pub(crate) fn paint_frame(frame: &RenderedFrame, rows: usize, cols: usize) {
    if rows == 0 || cols == 0 {
        return;
    }

    // Frame lines contain ANSI styling; Zellij's Text component counts those bytes toward width.
    print!("{}", terminal_output(frame, rows));
}

fn terminal_output(frame: &RenderedFrame, rows: usize) -> String {
    (0..rows)
        .map(|row| {
            let line = frame.lines.get(row).map_or("", String::as_str);
            format!("\u{1b}[2K{line}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_output_preserves_ansi_styled_text() {
        let frame = RenderedFrame {
            lines: vec!["\u{1b}[38;5;0m\u{1b}[48;5;3mpolite-rhinoceros\u{1b}[49m\u{1b}[39m".into()],
            ..RenderedFrame::default()
        };

        assert_eq!(
            terminal_output(&frame, 2),
            "\u{1b}[2K\u{1b}[38;5;0m\u{1b}[48;5;3mpolite-rhinoceros\u{1b}[49m\u{1b}[39m\n\u{1b}[2K"
        );
    }
}
