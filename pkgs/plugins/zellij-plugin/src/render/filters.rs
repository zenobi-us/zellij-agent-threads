use minijinja::value::ValueKind;
use minijinja::{Environment, Error, ErrorKind, Value};

pub(super) fn add_filters(env: &mut Environment<'_>) {
    env.add_filter("remap", remap);
    env.add_filter("fg", fg);
    env.add_filter("bg", bg);
    env.add_filter("dim", dim);
    env.add_filter("bold", bold);
    env.add_filter("italic", italic);
}

fn dim(value: Value) -> Result<Value, Error> {
    Ok(style(value, 2))
}

fn bold(value: Value) -> Result<Value, Error> {
    Ok(style(value, 1))
}

fn italic(value: Value) -> Result<Value, Error> {
    Ok(style(value, 3))
}

fn style(value: Value, code: u8) -> Value {
    Value::from(format!("\u{1b}[{}m{}\u{1b}[0m", code, value))
}

use super::click::add_button_functions;
use super::grid_layout::add_grid_layout_functions;

pub(super) fn add_template_helpers(env: &mut Environment<'_>) {
    add_filters(env);
    add_button_functions(env);
    add_grid_layout_functions(env);
}

fn remap(value: Value, mapping: Value) -> Result<Value, Error> {
    if mapping.kind() != ValueKind::Map {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "remap expects a map",
        ));
    }

    let mapped = mapping.get_item(&Value::from(value.to_string()))?;
    if mapped.is_undefined() {
        Ok(value)
    } else {
        Ok(mapped)
    }
}

fn fg(value: Value, color: Value) -> Result<Value, Error> {
    colorize(value, color, 30, 90, 38)
}

fn bg(value: Value, color: Value) -> Result<Value, Error> {
    colorize(value, color, 40, 100, 48)
}

fn colorize(
    value: Value,
    color: Value,
    normal_base: u8,
    bright_base: u8,
    indexed_prefix: u8,
) -> Result<Value, Error> {
    let code = color_code(&color.to_string(), normal_base, bright_base, indexed_prefix)?;
    Ok(Value::from(format!("\u{1b}[{}m{}\u{1b}[0m", code, value)))
}

fn color_code(
    color: &str,
    normal_base: u8,
    bright_base: u8,
    indexed_prefix: u8,
) -> Result<String, Error> {
    let color = color.trim().to_ascii_lowercase();
    let code = match color.as_str() {
        "black" => normal_base,
        "red" => normal_base + 1,
        "green" => normal_base + 2,
        "yellow" => normal_base + 3,
        "blue" => normal_base + 4,
        "magenta" => normal_base + 5,
        "cyan" => normal_base + 6,
        "white" => normal_base + 7,
        "bright-black" | "gray" | "grey" => bright_base,
        "bright-red" => bright_base + 1,
        "bright-green" => bright_base + 2,
        "bright-yellow" => bright_base + 3,
        "bright-blue" => bright_base + 4,
        "bright-magenta" => bright_base + 5,
        "bright-cyan" => bright_base + 6,
        "bright-white" => bright_base + 7,
        _ => {
            let index = color.parse::<u8>().map_err(|_| {
                Error::new(
                    ErrorKind::InvalidOperation,
                    "fg/bg expects a color name or 0-255 color index",
                )
            })?;
            return Ok(format!("{};5;{}", indexed_prefix, index));
        }
    };
    Ok(code.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::context;

    #[test]
    fn remaps_matching_word_and_keeps_misses() {
        let mut env = Environment::new();
        add_filters(&mut env);

        let rendered = env
            .render_str(
                "{{ hit | remap({\"running\": \"RUN\"}) }} {{ miss | remap({\"idle\": \"IDLE\"}) }}",
                context! { hit => "running", miss => "other" },
            )
            .unwrap();

        assert_eq!(rendered, "RUN other");
    }

    #[test]
    fn colors_text_with_fg_and_bg() {
        let mut env = Environment::new();
        add_filters(&mut env);

        let rendered = env
            .render_str(
                "{{ 'run' | fg('green') }} {{ 'stop' | bg('red') }} {{ 'idx' | fg('34') }}",
                context! {},
            )
            .unwrap();

        assert_eq!(
            rendered,
            "\u{1b}[32mrun\u{1b}[0m \u{1b}[41mstop\u{1b}[0m \u{1b}[38;5;34midx\u{1b}[0m"
        );
    }

    #[test]
    fn styles_text_with_sgr_attributes() {
        let mut env = Environment::new();
        add_filters(&mut env);

        let rendered = env
            .render_str(
                "{{ 'muted' | dim }} {{ 'loud' | bold }} {{ 'tilt' | italic }}",
                context! {},
            )
            .unwrap();

        assert_eq!(
            rendered,
            "\u{1b}[2mmuted\u{1b}[0m \u{1b}[1mloud\u{1b}[0m \u{1b}[3mtilt\u{1b}[0m"
        );
    }

    #[test]
    fn rejects_unknown_color() {
        let mut env = Environment::new();
        add_filters(&mut env);

        let error = env
            .render_str("{{ 'run' | fg('nope') }}", context! {})
            .unwrap_err();

        assert!(error.to_string().contains("fg/bg expects a color"));
    }
}
