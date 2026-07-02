use zellij_tile::prelude::*;

/// Returns the clickable collapse/expand label.
pub(super) fn collapse_button(collapsed: bool) -> &'static str {
    if collapsed {
        "[+]"
    } else {
        "[-]"
    }
}

/// Checks whether a mouse click landed on the collapse button.
///
/// Zellij mouse rows are signed while columns are unsigned in this API version;
/// keeping the conversion out of `main.rs` avoids callback clutter.
pub(crate) fn is_collapse_button_click(
    row: isize,
    col: usize,
    cols: usize,
    collapsed: bool,
) -> bool {
    row == 0 && col >= collapse_button_start_col(cols, collapsed)
}

/// Paints the collapse button in the far-right corner of a row.
pub(super) fn print_button(row: usize, cols: usize, button: &str) {
    print_text_with_coordinates(
        Text::new(button),
        collapse_button_start_col(cols, button == "[+]"),
        row,
        Some(button.len()),
        Some(1),
    );
}

/// Computes the first column occupied by the collapse button.
fn collapse_button_start_col(cols: usize, collapsed: bool) -> usize {
    cols.saturating_sub(collapse_button(collapsed).len())
}
