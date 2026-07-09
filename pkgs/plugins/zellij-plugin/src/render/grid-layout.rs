use minijinja::value::Kwargs;
use minijinja::{Error, ErrorKind, State, Value};

const MARKER_START: char = '\u{E000}';
const MARKER_END: char = '\u{E001}';

#[derive(Clone, Copy)]
struct Padding {
    x: usize,
    y: usize,
}

pub(super) fn add_grid_layout_functions(env: &mut minijinja::Environment<'_>) {
    env.add_function("Grid", grid);
    env.add_function("Stack", stack);
    env.add_function("Flex", flex);
}

fn grid(state: &State<'_, '_>, kwargs: Kwargs) -> Result<String, Error> {
    let cols = non_zero(kwargs.get::<Option<usize>>("cols")?.unwrap_or(1), "cols")?;
    let gap = kwargs.get::<Option<usize>>("gap")?.unwrap_or(2);
    let padding = padding(&kwargs)?;
    let (_viewport_rows, viewport_cols) = viewport(state)?;
    let caller: Value = kwargs.get("caller")?;
    kwargs.assert_all_used()?;

    let body = state.format(caller.call(state, &[])?)?;
    let cells = body_cells(&body);
    if cells.is_empty() {
        return Ok(String::new());
    }

    let widths = column_widths(cols, gap, viewport_cols.saturating_sub(padding.x * 2));
    let mut out = Vec::new();
    for row in cells.chunks(cols) {
        let mut line = String::new();
        for (col, cell) in row.iter().enumerate() {
            if col > 0 {
                line.push_str(&" ".repeat(gap));
            }
            line.push_str(&fit_cell(cell, widths[col]));
        }
        out.push(line.trim_end().to_string());
    }

    Ok(apply_padding(out.join("\n"), padding))
}

fn stack(state: &State<'_, '_>, kwargs: Kwargs) -> Result<String, Error> {
    let gap = kwargs.get::<Option<usize>>("gap")?.unwrap_or(1);
    let padding = padding(&kwargs)?;
    let grow = kwargs.get::<Option<bool>>("grow")?.unwrap_or(false);
    let (viewport_rows, _viewport_cols) = viewport(state)?;
    let caller: Value = kwargs.get("caller")?;
    kwargs.assert_all_used()?;

    let body = state.format(caller.call(state, &[])?)?;
    let cells = body_cells(&body);
    if cells.is_empty() {
        return Ok(String::new());
    }

    let rendered = cells.join(&"\n".repeat(gap + 1));
    let rendered = if grow {
        grow_lines(rendered, viewport_rows.saturating_sub(padding.y * 2))
    } else {
        rendered
    };
    Ok(apply_padding(rendered, padding))
}

fn flex(state: &State<'_, '_>, kwargs: Kwargs) -> Result<String, Error> {
    let direction = kwargs
        .get::<Option<String>>("direction")?
        .unwrap_or_else(|| "row".into());
    let gap = kwargs.get::<Option<usize>>("gap")?.unwrap_or(2);
    let padding = padding(&kwargs)?;
    let grow = kwargs.get::<Option<bool>>("grow")?.unwrap_or(false);
    let weights = kwargs.get::<Option<String>>("weights")?;
    let (viewport_rows, viewport_cols) = viewport(state)?;
    let caller: Value = kwargs.get("caller")?;
    kwargs.assert_all_used()?;

    let body = state.format(caller.call(state, &[])?)?;
    let cells = body_cells(&body);
    if cells.is_empty() {
        return Ok(String::new());
    }

    let weights = match weights {
        Some(weights) => parse_weights(&weights, cells.len())?,
        None => vec![1; cells.len()],
    };

    let rendered = match direction.as_str() {
        "row" | "horizontal" => Ok(join_row(
            &cells,
            if grow {
                flex_widths(viewport_cols.saturating_sub(padding.x * 2), gap, &weights)
            } else {
                natural_widths(&cells)
            },
            gap,
        )),
        "column" | "vertical" => Ok(join_column(
            &cells,
            if grow {
                flex_column_heights(
                    viewport_rows.saturating_sub(padding.y * 2),
                    gap,
                    &cells,
                    &weights,
                )
            } else {
                natural_heights(&cells)
            },
            gap,
        )),
        _ => Err(Error::new(
            ErrorKind::InvalidOperation,
            "Flex direction must be row or column",
        )),
    }?;

    Ok(apply_padding(rendered, padding))
}

fn body_cells(body: &str) -> Vec<String> {
    if body.lines().any(|line| line.trim() == "---") {
        return body
            .lines()
            .collect::<Vec<_>>()
            .split(|line| line.trim() == "---")
            .map(|group| group.join("\n").trim().to_string())
            .filter(|group| !group.is_empty())
            .collect();
    }

    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn non_zero(value: usize, name: &str) -> Result<usize, Error> {
    if value == 0 {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("Grid expects {name} > 0"),
        ))
    } else {
        Ok(value)
    }
}

fn viewport(state: &State<'_, '_>) -> Result<(usize, usize), Error> {
    Ok((
        viewport_value(state, "viewport_rows")?,
        viewport_value(state, "viewport_cols")?,
    ))
}

fn viewport_value(state: &State<'_, '_>, name: &str) -> Result<usize, Error> {
    let value = state.lookup(name).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("Grid expects {name} global"),
        )
    })?;
    value.as_usize().ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("Grid expects {name} to be a positive integer"),
        )
    })
}

fn padding(kwargs: &Kwargs) -> Result<Padding, Error> {
    let padding = kwargs.get::<Option<usize>>("padding")?.unwrap_or(0);
    Ok(Padding {
        x: kwargs.get::<Option<usize>>("paddingX")?.unwrap_or(padding),
        y: kwargs.get::<Option<usize>>("paddingY")?.unwrap_or(padding),
    })
}

fn apply_padding(rendered: String, padding: Padding) -> String {
    if padding.x == 0 && padding.y == 0 {
        return rendered;
    }

    let x = " ".repeat(padding.x);
    let mut lines = Vec::new();
    lines.extend(std::iter::repeat_n(String::new(), padding.y));
    lines.extend(rendered.lines().map(|line| format!("{x}{line}{x}")));
    lines.extend(std::iter::repeat_n(String::new(), padding.y));
    lines.join("\n")
}

fn column_widths(cols: usize, gap: usize, width: usize) -> Vec<usize> {
    let gaps = gap.saturating_mul(cols.saturating_sub(1));
    let cell_width = width.saturating_sub(gaps).checked_div(cols).unwrap_or(0);
    vec![cell_width; cols]
}

fn natural_widths(cells: &[String]) -> Vec<usize> {
    cells.iter().map(|cell| visible_width(cell)).collect()
}

fn natural_heights(cells: &[String]) -> Vec<usize> {
    cells
        .iter()
        .map(|cell| cell.lines().count().max(1))
        .collect()
}

fn flex_column_heights(
    width: usize,
    gap: usize,
    cells: &[String],
    weights: &[usize],
) -> Vec<usize> {
    let gaps = gap.saturating_mul(cells.len().saturating_sub(1));
    let natural = natural_heights(cells);
    let natural_total: usize = natural.iter().sum();
    let extra = width.saturating_sub(gaps).saturating_sub(natural_total);
    let total_weight: usize = weights.iter().sum();
    if total_weight == 0 {
        return natural;
    }

    let mut heights = natural;
    let mut used = 0;
    for (height, weight) in heights.iter_mut().zip(weights) {
        let share = extra.saturating_mul(*weight) / total_weight;
        *height += share;
        used += share;
    }
    if let Some((height, _)) = heights
        .iter_mut()
        .zip(weights)
        .rev()
        .find(|(_, weight)| **weight > 0)
    {
        *height += extra.saturating_sub(used);
    }
    heights
}

fn flex_widths(width: usize, gap: usize, weights: &[usize]) -> Vec<usize> {
    let gaps = gap.saturating_mul(weights.len().saturating_sub(1));
    let usable = width.saturating_sub(gaps);
    let total: usize = weights.iter().sum();
    if total == 0 {
        return vec![0; weights.len()];
    }

    let mut widths: Vec<_> = weights
        .iter()
        .map(|weight| usable.saturating_mul(*weight) / total)
        .collect();
    let used: usize = widths.iter().sum();
    if let Some(last) = widths.last_mut() {
        *last += usable.saturating_sub(used);
    }
    widths
}

fn parse_weights(weights: &str, expected: usize) -> Result<Vec<usize>, Error> {
    let parsed: Result<Vec<_>, _> = weights
        .split(',')
        .map(|part| part.trim().parse::<usize>())
        .collect();
    let parsed = parsed.map_err(|_| {
        Error::new(
            ErrorKind::InvalidOperation,
            "Flex weights must be comma-separated positive integers",
        )
    })?;
    if parsed.len() != expected || parsed.iter().all(|weight| *weight == 0) {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "Flex weights must match cell count and include a growable item",
        ));
    }
    Ok(parsed)
}

fn join_row(cells: &[String], widths: Vec<usize>, gap: usize) -> String {
    let mut line = String::new();
    for (idx, cell) in cells.iter().enumerate() {
        if idx > 0 {
            line.push_str(&" ".repeat(gap));
        }
        line.push_str(&fit_cell(cell, widths[idx]));
    }
    line.trim_end().to_string()
}

fn join_column(cells: &[String], heights: Vec<usize>, gap: usize) -> String {
    let mut lines = Vec::new();
    for (idx, cell) in cells.iter().enumerate() {
        if idx > 0 {
            lines.extend(std::iter::repeat_n(String::new(), gap));
        }
        let cell_lines: Vec<_> = cell.lines().map(str::to_string).collect();
        let cell_height = cell_lines.len().max(1);
        lines.extend(cell_lines);
        lines.extend(std::iter::repeat_n(
            String::new(),
            heights[idx].saturating_sub(cell_height),
        ));
    }
    lines.join("\n").trim_end().to_string()
}

fn fit_cell(cell: &str, width: usize) -> String {
    let mut value = truncate_visible(cell, width);
    let padding = width.saturating_sub(visible_width(&value));
    value.push_str(&" ".repeat(padding));
    value
}

fn grow_lines(rendered: String, viewport_rows: usize) -> String {
    let current_rows = rendered.lines().count();
    if current_rows >= viewport_rows {
        rendered
    } else {
        format!("{}{}", rendered, "\n".repeat(viewport_rows - current_rows))
    }
}

fn visible_width(value: &str) -> usize {
    let mut width = 0;
    let mut i = 0;
    while i < value.len() {
        let rest = &value[i..];
        if let Some(consumed) = zero_width_len(rest) {
            i += consumed;
            continue;
        }
        let ch = rest.chars().next().unwrap();
        width += 1;
        i += ch.len_utf8();
    }
    width
}

fn truncate_visible(value: &str, max_width: usize) -> String {
    let mut out = String::new();
    let mut width = 0;
    let mut i = 0;
    while i < value.len() {
        let rest = &value[i..];
        if let Some(consumed) = zero_width_len(rest) {
            out.push_str(&rest[..consumed]);
            i += consumed;
            continue;
        }

        if width == max_width {
            break;
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        width += 1;
        i += ch.len_utf8();
    }
    out
}

fn zero_width_len(value: &str) -> Option<usize> {
    if value.starts_with('\u{1b}') {
        return Some(value.find('m').map(|idx| idx + 1).unwrap_or(1));
    }
    if value.starts_with(MARKER_START) {
        return value
            .find(MARKER_END)
            .map(|idx| idx + MARKER_END.len_utf8());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::click::{collect_hitboxes, ClickAction};

    #[test]
    fn lays_out_call_body_as_fixed_columns() {
        let mut env = minijinja::Environment::new();
        add_grid_layout_functions(&mut env);
        env.add_global("viewport_rows", 10usize);
        env.add_global("viewport_cols", 7usize);

        let rendered = env
            .render_str(
                "{% call Grid(cols=2, gap=1) %}A\nBB\nCCC\nD{% endcall %}",
                (),
            )
            .unwrap();

        assert_eq!(rendered, "A   BB\nCCC D");
    }

    #[test]
    fn uses_viewport_width() {
        let mut env = minijinja::Environment::new();
        add_grid_layout_functions(&mut env);
        env.add_global("viewport_rows", 10usize);
        env.add_global("viewport_cols", 8usize);

        let rendered = env
            .render_str("{% call Grid(cols=2, gap=2) %}abcd\nef{% endcall %}", ())
            .unwrap();

        assert_eq!(rendered, "abc  ef");
    }

    #[test]
    fn applies_padding_to_layout_helpers() {
        let mut env = minijinja::Environment::new();
        add_grid_layout_functions(&mut env);
        env.add_global("viewport_rows", 5usize);
        env.add_global("viewport_cols", 8usize);

        let rendered = env
            .render_str(
                "{% call Flex(gap=1, grow=true, paddingX=1, paddingY=1) %}A\nB{% endcall %}",
                (),
            )
            .unwrap();

        assert_eq!(rendered, "\n A  B \n");
    }

    #[test]
    fn rejects_zero_columns() {
        let mut env = minijinja::Environment::new();
        add_grid_layout_functions(&mut env);

        let error = env
            .render_str("{% call Grid(cols=0) %}x{% endcall %}", ())
            .unwrap_err();

        assert!(error.to_string().contains("Grid expects cols > 0"));
    }

    #[test]
    fn keeps_button_markers_zero_width_for_hitboxes() {
        let mut env = minijinja::Environment::new();
        add_grid_layout_functions(&mut env);
        env.add_global("viewport_rows", 10usize);
        env.add_global("viewport_cols", 5usize);
        crate::render::click::add_button_functions(&mut env);

        let captured = env
            .template_from_str(
                "{% call Grid(cols=2, gap=1) %}{% call PaneButton(pane=\"1\") %}X{% endcall %}\nYY{% endcall %}",
            )
            .unwrap()
            .render_captured(())
            .unwrap();
        let (clean, hitboxes) = collect_hitboxes(captured.state(), captured.output());

        assert_eq!(clean, "X  YY");
        assert_eq!(hitboxes[0].start_col, 0);
        assert_eq!(hitboxes[0].end_col, 1);
        assert_eq!(
            hitboxes[0].action,
            ClickAction::FocusPane { pane: "1".into() }
        );
    }

    #[test]
    fn stack_renders_vertical() {
        let mut env = minijinja::Environment::new();
        add_grid_layout_functions(&mut env);
        env.add_global("viewport_rows", 10usize);
        env.add_global("viewport_cols", 8usize);

        let rendered = env
            .render_str("{% call Stack(gap=1) %}A\nB{% endcall %}", ())
            .unwrap();

        assert_eq!(rendered, "A\n\nB");
    }

    #[test]
    fn stack_grows_to_viewport_height() {
        let mut env = minijinja::Environment::new();
        add_grid_layout_functions(&mut env);
        env.add_global("viewport_rows", 4usize);
        env.add_global("viewport_cols", 8usize);

        let rendered = env
            .render_str("{% call Stack(gap=0, grow=true) %}A\nB{% endcall %}", ())
            .unwrap();

        assert_eq!(rendered, "A\nB\n\n");
    }

    #[test]
    fn flex_distributes_viewport_by_weights() {
        let mut env = minijinja::Environment::new();
        add_grid_layout_functions(&mut env);
        env.add_global("viewport_rows", 10usize);
        env.add_global("viewport_cols", 11usize);

        let rendered = env
            .render_str(
                "{% call Flex(gap=1, grow=true, weights=\"1,2\") %}aa\nbbbbbb{% endcall %}",
                (),
            )
            .unwrap();

        assert_eq!(rendered, "aa  bbbbbb");
    }

    #[test]
    fn flex_supports_column_direction() {
        let mut env = minijinja::Environment::new();
        add_grid_layout_functions(&mut env);
        env.add_global("viewport_rows", 5usize);
        env.add_global("viewport_cols", 20usize);

        let rendered = env
            .render_str(
                "{% call Flex(direction=\"column\", gap=1, grow=true, weights=\"1,2\") %}top\nbottom{% endcall %}",
                (),
            )
            .unwrap();

        assert_eq!(rendered, "top\n\nbottom");
    }

    #[test]
    fn flex_rejects_mismatched_weights() {
        let mut env = minijinja::Environment::new();
        add_grid_layout_functions(&mut env);
        env.add_global("viewport_rows", 10usize);
        env.add_global("viewport_cols", 11usize);

        let error = env
            .render_str("{% call Flex(weights=\"1\") %}A\nB{% endcall %}", ())
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("Flex weights must match cell count"));
    }
}
