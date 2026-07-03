use zellij_tile::prelude::*;

/// Returns the clickable collapse/expand label.
pub(super) fn collapse_button(collapsed: bool) -> &'static str {
    if collapsed {
        "[+]"
    } else {
        "[-]"
    }
}

/// Paints the collapse button in the far-right corner of a row.
pub(super) fn print_button(row: usize, cols: usize, button: &str) {
    print_text_with_coordinates(
        Text::new(button),
        cols.saturating_sub(button.len()),
        row,
        Some(button.len()),
        Some(1),
    );
}
