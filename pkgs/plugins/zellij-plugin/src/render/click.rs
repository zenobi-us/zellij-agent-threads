use std::sync::Mutex;

use minijinja::value::{Kwargs, Object};
use minijinja::{Error, ErrorKind, State, Value};

const MARKER_END: &str = "\u{E001}";
const BUTTON_START: &str = "\u{E000}B";
const BUTTON_END: &str = "\u{E000}E";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClickAction {
    SwitchTab { tab: u32 },
    FocusPane { pane: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Hitbox {
    pub(crate) row: usize,
    pub(crate) start_col: usize,
    pub(crate) end_col: usize,
    pub(crate) action: ClickAction,
}

#[derive(Debug, Default)]
struct ButtonRegistry {
    actions: Mutex<Vec<ClickAction>>,
}

impl Object for ButtonRegistry {}

pub(super) fn add_button_functions(env: &mut minijinja::Environment<'_>) {
    env.add_function("PaneButton", pane_button);
    env.add_function("TabButton", tab_button);
    env.add_filter("pane_button", pane_button_filter);
    env.add_filter("tab_button", tab_button_filter);
}

pub(crate) fn hitbox_at(hitboxes: &[Hitbox], row: isize, col: usize) -> Option<ClickAction> {
    let row = usize::try_from(row).ok()?;
    hitboxes
        .iter()
        .find(|hitbox| hitbox.row == row && col >= hitbox.start_col && col < hitbox.end_col)
        .map(|hitbox| hitbox.action.clone())
}

pub(super) fn wrap_button(state: &State<'_, '_>, label: String, action: ClickAction) -> String {
    let registry = state.get_or_set_temp_object("zat.buttons", ButtonRegistry::default);
    let mut actions = registry.actions.lock().unwrap();
    let id = actions.len();
    actions.push(action);
    format!("{BUTTON_START}{id}{MARKER_END}{label}{BUTTON_END}{id}{MARKER_END}")
}

pub(super) fn collect_hitboxes(state: &State<'_, '_>, rendered: &str) -> (String, Vec<Hitbox>) {
    let actions = state
        .get_temp("zat.buttons")
        .and_then(|value| value.downcast_object::<ButtonRegistry>())
        .map(|registry| registry.actions.lock().unwrap().clone())
        .unwrap_or_default();

    let mut clean = String::new();
    let mut hitboxes = Vec::new();
    let mut active = None;
    for (row, line) in rendered.lines().enumerate() {
        if row > 0 {
            clean.push('\n');
        }
        let (line, mut line_hitboxes) = strip_line_markers(row, line, &actions, &mut active);
        clean.push_str(&line);
        hitboxes.append(&mut line_hitboxes);
    }
    (clean, hitboxes)
}

fn pane_button(state: &State<'_, '_>, kwargs: Kwargs) -> Result<String, Error> {
    let pane: String = kwargs.get("pane")?;
    let label = caller_label(state, &kwargs)?;
    kwargs.assert_all_used()?;
    Ok(wrap_button(state, label, ClickAction::FocusPane { pane }))
}

fn tab_button(state: &State<'_, '_>, kwargs: Kwargs) -> Result<String, Error> {
    let tab = tab_arg(&kwargs)?;
    let label = caller_label(state, &kwargs)?;
    kwargs.assert_all_used()?;
    Ok(wrap_button(state, label, ClickAction::SwitchTab { tab }))
}

fn pane_button_filter(
    state: &State<'_, '_>,
    label: String,
    kwargs: Kwargs,
) -> Result<String, Error> {
    let pane: String = kwargs.get("pane")?;
    kwargs.assert_all_used()?;
    Ok(wrap_button(state, label, ClickAction::FocusPane { pane }))
}

fn tab_button_filter(
    state: &State<'_, '_>,
    label: String,
    kwargs: Kwargs,
) -> Result<String, Error> {
    let tab = tab_arg(&kwargs)?;
    kwargs.assert_all_used()?;
    Ok(wrap_button(state, label, ClickAction::SwitchTab { tab }))
}

fn tab_arg(kwargs: &Kwargs) -> Result<u32, Error> {
    let value: Value = kwargs.get("tab")?;
    if let Some(tab) = value.as_usize().and_then(|value| u32::try_from(value).ok()) {
        return Ok(tab);
    }
    value
        .as_str()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                "tab button expects numeric tab id",
            )
        })
}

fn caller_label(state: &State<'_, '_>, kwargs: &Kwargs) -> Result<String, Error> {
    let caller: Value = kwargs.get("caller")?;
    state.format(caller.call(state, &[])?)
}

fn strip_line_markers(
    row: usize,
    line: &str,
    actions: &[ClickAction],
    active: &mut Option<(usize, usize)>,
) -> (String, Vec<Hitbox>) {
    let mut clean = String::new();
    let mut hitboxes = Vec::new();
    let mut col = 0;
    let mut i = 0;

    while i < line.len() {
        let rest = &line[i..];
        if let Some((id, consumed)) = parse_marker(rest, BUTTON_START) {
            *active = Some((id, col));
            i += consumed;
            continue;
        }
        if let Some((id, consumed)) = parse_marker(rest, BUTTON_END) {
            if let (Some((active_id, start_col)), Some(action)) = (active.take(), actions.get(id)) {
                if active_id == id && col > start_col {
                    hitboxes.push(Hitbox {
                        row,
                        start_col,
                        end_col: col,
                        action: action.clone(),
                    });
                }
            }
            i += consumed;
            continue;
        }
        if rest.starts_with('\u{1b}') {
            let consumed = rest.find('m').map(|idx| idx + 1).unwrap_or(1);
            clean.push_str(&rest[..consumed]);
            i += consumed;
            continue;
        }
        let ch = rest.chars().next().unwrap();
        clean.push(ch);
        col += 1;
        i += ch.len_utf8();
    }

    if let Some((id, start_col)) = active {
        if let Some(action) = actions.get(*id) {
            if col > *start_col {
                hitboxes.push(Hitbox {
                    row,
                    start_col: *start_col,
                    end_col: col,
                    action: action.clone(),
                });
            }
        }
        *active = Some((*id, 0));
    }

    (clean, hitboxes)
}

fn parse_marker(input: &str, prefix: &str) -> Option<(usize, usize)> {
    let rest = input.strip_prefix(prefix)?;
    let end = rest.find(MARKER_END)?;
    let id = rest[..end].parse().ok()?;
    Some((id, prefix.len() + end + MARKER_END.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markers_and_registers_hitbox() {
        let action = ClickAction::FocusPane { pane: "1".into() };
        let input = format!("x{BUTTON_START}0{MARKER_END}abc{BUTTON_END}0{MARKER_END}y");

        let mut active = None;
        let (clean, hitboxes) =
            strip_line_markers(2, &input, std::slice::from_ref(&action), &mut active);

        assert_eq!(clean, "xabcy");
        assert_eq!(
            hitboxes,
            vec![Hitbox {
                row: 2,
                start_col: 1,
                end_col: 4,
                action
            }]
        );
    }

    #[test]
    fn ansi_escapes_do_not_count_as_columns() {
        let action = ClickAction::SwitchTab { tab: 3 };
        let input =
            format!("{BUTTON_START}0{MARKER_END}\u{1b}[31mred\u{1b}[0m{BUTTON_END}0{MARKER_END}");

        let mut active = None;
        let (clean, hitboxes) =
            strip_line_markers(0, &input, std::slice::from_ref(&action), &mut active);

        assert_eq!(clean, "\u{1b}[31mred\u{1b}[0m");
        assert_eq!(hitboxes[0].start_col, 0);
        assert_eq!(hitboxes[0].end_col, 3);
        assert_eq!(hitboxes[0].action, action);
    }

    #[test]
    fn multiline_button_registers_one_hitbox_per_visible_line() {
        let action = ClickAction::FocusPane { pane: "1".into() };
        let mut env = minijinja::Environment::new();
        add_button_functions(&mut env);
        let captured = env
            .template_from_str("x{% call PaneButton(pane=\"1\") %}ab\ncd{% endcall %}y")
            .unwrap()
            .render_captured(())
            .unwrap();

        let (clean, hitboxes) = collect_hitboxes(captured.state(), captured.output());

        assert_eq!(clean, "xab\ncdy");
        assert_eq!(
            hitboxes,
            vec![
                Hitbox {
                    row: 0,
                    start_col: 1,
                    end_col: 3,
                    action: action.clone(),
                },
                Hitbox {
                    row: 1,
                    start_col: 0,
                    end_col: 2,
                    action,
                },
            ]
        );
    }
}
