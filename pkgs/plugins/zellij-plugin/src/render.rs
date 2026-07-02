//! Builds and paints the plugin UI.
//!
//! Rendering has two layers on purpose: [`RenderModel`] prepares plain data that
//! host tests can inspect, while [`Renderer`] performs the Zellij terminal calls.
//! This keeps UX decisions testable even though Zellij drawing itself is a host
//! side effect.

use zellij_tile::prelude::*;

use crate::config::RenderConfig;
use crate::runtime::{basename, state_label, RuntimeState};

/// Render-ready snapshot of runtime state.
///
/// This is the seam between plugin state and terminal drawing. It hides storage
/// details like `BTreeMap`/`VecDeque` from [`Renderer`] and from future template
/// rendering code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RenderModel {
    pub(crate) collapsed: bool,
    status: String,
    empty_message: String,
    sessions: Vec<SessionLine>,
    events: Vec<String>,
}

/// One display row for a Pi agent session.
///
/// Values are already formatted for compact terminal output so the painter does
/// not need to know about agent payload fields.
#[derive(Clone, Debug, Eq, PartialEq)]
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
        let status = if let Some(error) = &state.last_error {
            format!(
                "{} pipes={} collapsed={} error={}",
                config.title, state.pipe_count, state.collapsed, error
            )
        } else {
            format!(
                "{} pipes={} sessions={} collapsed={}",
                config.title,
                state.pipe_count,
                state.sessions.len(),
                state.collapsed
            )
        };

        let sessions = state
            .sessions
            .values()
            .map(|session| SessionLine {
                state: state_label(&session.state),
                pane: session.pane_id.clone().unwrap_or_else(|| "?".into()),
                cwd: basename(&session.cwd).into(),
                model: session.model.clone().unwrap_or_else(|| "?".into()),
            })
            .collect();

        Self {
            collapsed: state.collapsed,
            status,
            empty_message: config.empty_message.clone(),
            sessions,
            events: state.events.iter().rev().cloned().collect(),
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
        print_line(0, cols.saturating_sub(button.len() + 1), &model.status);
        print_button(0, cols, button);

        if rows <= 1 || model.collapsed {
            return;
        }

        if model.sessions.is_empty() {
            print_line(1, cols, &model.empty_message);
        } else {
            for (row, session) in model.sessions.iter().take(rows - 1).enumerate() {
                print_line(row + 1, cols, &session.to_line());
            }
        }

        let first_event_row = if model.sessions.is_empty() {
            2
        } else {
            model.sessions.len() + 1
        };
        for (offset, event) in model
            .events
            .iter()
            .take(rows.saturating_sub(first_event_row))
            .enumerate()
        {
            print_line(first_event_row + offset, cols, event);
        }
    }
}

impl SessionLine {
    /// Formats one session row using the compact default layout.
    fn to_line(&self) -> String {
        format!(
            "{} pane={} {} {}",
            self.state, self.pane, self.cwd, self.model
        )
    }
}

/// Returns the clickable collapse/expand label.
pub(crate) fn collapse_button(collapsed: bool) -> &'static str {
    if collapsed { "[+]" } else { "[-]" }
}

/// Computes the first column occupied by the collapse button.
fn collapse_button_start_col(cols: usize, collapsed: bool) -> usize {
    cols.saturating_sub(collapse_button(collapsed).len())
}

/// Checks whether a mouse click landed on the collapse button.
///
/// Zellij mouse rows are signed while columns are unsigned in this API version;
/// keeping the conversion out of `main.rs` avoids callback clutter.
pub(crate) fn is_collapse_button_click(row: isize, col: usize, cols: usize, collapsed: bool) -> bool {
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

        assert_eq!(model.status, "zellij-agent pipes=2 sessions=1 collapsed=false");
        assert_eq!(model.sessions[0].to_line(), "running pane=1 project m");
        assert_eq!(model.events, vec!["new", "old"]);
    }
}
