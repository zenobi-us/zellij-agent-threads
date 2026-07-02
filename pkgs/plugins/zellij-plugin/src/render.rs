//! Builds and paints the plugin UI.
//!
//! Rendering has two layers on purpose: [`RenderModel`] prepares plain data that
//! host tests can inspect, while [`Renderer`] performs the Zellij terminal calls.
//! This keeps UX decisions testable even though Zellij drawing itself is a host
//! side effect.

use std::collections::BTreeMap;
use minijinja::Environment;
use serde::Serialize;

use zellij_tile::prelude::*;

use crate::config::RenderConfig;
use crate::runtime::{basename, state_label, RuntimeState};

pub(crate) const DEFAULT_TEMPLATE: &str = r#"
{% if sessions | length == 0 -%}
  0 Agents
{% else -%}
{% for group in groups %}
{{ group.tab_name }} (#{{ group.tab_id }})
{% for session in group.sessions -%}
  {{ session.pane }} {{ session.state }} {{ session.model }} {{ session.cwd }}
{% endfor -%}

{% endfor %}
{% endif -%}
"#;

/// Render-ready snapshot of runtime state.
///
/// This is the seam between plugin state and terminal drawing. It hides storage
/// details like `BTreeMap`/`VecDeque` from [`Renderer`] and from future template
/// rendering code.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RenderModel {
    pub(crate) collapsed: bool,
    empty_message: String,
    sessions: Vec<SessionLine>,
    groups: Vec<TabGroup>,
    events: Vec<String>,
    template: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct TabGroup {
    tab_id: String,
    tab_name: String,
    sessions: Vec<SessionLine>,
}

/// One display row for a Pi agent session.
///
/// Values are already formatted for compact terminal output so the painter does
/// not need to know about agent payload fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct SessionLine {
    state: &'static str,
    pane: String,
    cwd: String,
    model: String,
}

impl RenderModel {
    /// Builds a testable render snapshot from runtime state and render config.
    ///
    /// This is where UX decisions live: status wording, event ordering, fallback
    /// text, and path compaction. Keeping those choices here lets future template
    /// support consume the same model instead of raw plugin state.
    pub(crate) fn from_runtime(state: &RuntimeState, config: &RenderConfig) -> Self {
        let sessions: Vec<_> = state
            .sessions
            .values()
            .map(|session| SessionLine {
                state: state_label(&session.state),
                pane: session.pane_id.clone().unwrap_or_else(|| "?".into()),
                cwd: basename(&session.cwd).into(),
                model: session.model.clone().unwrap_or_else(|| "?".into()),
            })
            .collect();

        let mut groups = BTreeMap::<String, TabGroup>::new();
        for session in state.sessions.values() {
            let tab_id = session.tab_id.map(|id| id.to_string()).unwrap_or_else(|| "?".into());
            let tab_name = session.tab_name.clone().unwrap_or_else(|| "unknown tab".into());
            let key = format!("{tab_id}\0{tab_name}");
            groups
                .entry(key)
                .or_insert_with(|| TabGroup {
                    tab_id,
                    tab_name,
                    sessions: Vec::new(),
                })
                .sessions
                .push(SessionLine {
                    state: state_label(&session.state),
                    pane: session.pane_id.clone().unwrap_or_else(|| "?".into()),
                    cwd: basename(&session.cwd).into(),
                    model: session.model.clone().unwrap_or_else(|| "?".into()),
                });
        }

        Self {
            collapsed: state.collapsed,
            empty_message: config.empty_message.clone(),
            sessions,
            groups: groups.into_values().collect(),
            events: state.events.iter().rev().cloned().collect(),
            template: config.template.clone(),
        }
    }
}

/// Paints a [`RenderModel`] into the Zellij plugin pane.
///
/// This type intentionally contains no runtime knowledge. If a future change can
/// be tested by checking [`RenderModel`], keep it out of this painter.
pub(crate) struct Renderer;

impl Renderer {
    /// Clears the pane and paints visible rows for the current terminal size.
    ///
    /// `rows` and `cols` come from Zellij, so zero-sized panes are valid during
    /// layout churn and should render nothing rather than panic.
    pub(crate) fn render(model: &RenderModel, rows: usize, cols: usize) {
        clear_screen();

        if rows == 0 || cols == 0 {
            return;
        }

        let button = collapse_button(model.collapsed);
        let rendered =
            render_template(model).unwrap_or_else(|error| format!("template error: {}", error));

        for (row, line) in rendered.lines().take(rows).enumerate() {
            let line_cols = if row == 0 {
                cols.saturating_sub(button.len() + 1)
            } else {
                cols
            };
            print_line(row, line_cols, line);
        }
        print_button(0, cols, button);
    }
}

fn render_template(model: &RenderModel) -> Result<String, minijinja::Error> {
    Environment::new().render_str(&model.template, model)
}

/// Returns the clickable collapse/expand label.
pub(crate) fn collapse_button(collapsed: bool) -> &'static str {
    if collapsed {
        "[+]"
    } else {
        "[-]"
    }
}

/// Computes the first column occupied by the collapse button.
fn collapse_button_start_col(cols: usize, collapsed: bool) -> usize {
    cols.saturating_sub(collapse_button(collapsed).len())
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
fn print_button(row: usize, cols: usize, button: &str) {
    print_text_with_coordinates(
        Text::new(button),
        collapse_button_start_col(cols, button == "[+]"),
        row,
        Some(button.len()),
        Some(1),
    );
}

/// Paints one clipped line at column zero.
fn print_line(row: usize, cols: usize, line: &str) {
    print_text_with_coordinates(Text::new(truncate(line, cols)), 0, row, Some(cols), Some(1));
}

/// Truncates by characters so UTF-8 input is never sliced mid-codepoint.
fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{AgentSession, AgentState};
    use std::collections::{BTreeMap, VecDeque};

    #[test]
    fn builds_render_model_from_runtime() {
        let runtime = RuntimeState {
            sessions: BTreeMap::from([(
                "s".into(),
                AgentSession {
                    version: 1,
                    session: "s".into(),
                    cwd: "/tmp/project".into(),
                    pane_id: Some("1".into()),
                    tab_id: Some(7),
                    tab_name: Some("Agents".into()),
                    zellij_session: Some("z".into()),
                    state: AgentState::Running,
                    model: Some("m".into()),
                    updated_at: 0,
                },
            )]),
            events: VecDeque::from(["old".into(), "new".into()]),
            pipe_count: 2,
            last_error: None,
            collapsed: false,
            last_cols: 0,
        };

        let model = RenderModel::from_runtime(&runtime, &RenderConfig::default());

        assert_eq!(model.sessions.len(), 1);
        assert_eq!(model.events, vec!["new", "old"]);
        assert!(render_template(&model).unwrap().contains("project"));
    }
}
