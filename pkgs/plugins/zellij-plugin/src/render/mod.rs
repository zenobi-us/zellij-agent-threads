//! Builds and paints the plugin UI.
//!
//! Rendering has two layers on purpose: [`RenderModel`] prepares plain data that
//! host tests can inspect, while [`Renderer`] performs the Zellij terminal calls.
//! This keeps UX decisions testable even though Zellij drawing itself is a host
//! side effect.

mod click;
mod filters;
#[path = "grid-layout.rs"]
mod grid_layout;
mod model;
mod template;

pub(crate) use click::{hitbox_at, ClickAction, Hitbox};
pub(crate) use model::{RenderModel, DEFAULT_TEMPLATE};

use zellij_tile::prelude::*;

use template::render_template;

/// Paints a [`RenderModel`] into the Zellij plugin pane.
///
/// This type intentionally contains no runtime knowledge. If a future change can
/// be tested by checking [`RenderModel`], keep it out of this painter.
pub(crate) struct Renderer;

impl Renderer {
    /// Paints visible rows for the current terminal size.
    ///
    /// `rows` and `cols` come from Zellij, so zero-sized panes are valid during
    /// layout churn and should render nothing rather than panic.
    pub(crate) fn render(model: &RenderModel, rows: usize, cols: usize) -> Vec<Hitbox> {
        if rows == 0 || cols == 0 {
            return Vec::new();
        }

        clear_plugin_rows(rows, cols);

        let (rendered, hitboxes) = render_template(model, rows, cols)
            .unwrap_or_else(|error| (format!("template error: {}", error), Vec::new()));

        for (row, line) in rendered.lines().take(rows).enumerate() {
            let line_cols = cols;
            print_line(row, line_cols, line);
        }
        hitboxes
    }
}

/// Clears only the plugin render area.
///
/// Do not call Zellij's `clear_screen()` here: in zellij-tile 0.44 it clears the
/// focused pane's scroll buffer, which can be a neighboring terminal pane.
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
    use crate::config::RenderConfig;
    use crate::runtime::{AgentSession, AgentState, RuntimeState};
    use std::collections::{BTreeMap, VecDeque};

    #[test]
    fn renders_template_with_project_and_remap_filter() {
        let runtime = RuntimeState {
            sessions: BTreeMap::from([(
                "s".into(),
                AgentSession {
                    version: 1,
                    harness: Some("pi".into()),
                    session: "s".into(),
                    cwd: "/tmp/project".into(),
                    pane_id: Some("1".into()),
                    tab_id: Some(7),
                    tab_name: Some("Agents".into()),
                    zellij_session: Some("z".into()),
                    state: AgentState::Running,
                    model: Some("m".into()),
                    title: Some("First Message Title".into()),
                    current_task: Some("Latest Task".into()),
                    updated_at: 0,
                },
            )]),
            events: VecDeque::from(["old".into(), "new".into()]),
            pipe_count: 2,
            last_error: None,
            last_cols: 0,
            focused_pane: Some("1".into()),
            active_tab: Some(7),
            active_tab_position: Some(0),
            zellij_session: None,
        };

        let model = RenderModel::from_runtime(&runtime, &RenderConfig::default());
        assert!(render_template(&model, 10, 80)
            .unwrap()
            .0
            .contains("project"));
        let (_, default_hitboxes) = render_template(&model, 10, 80).unwrap();
        assert_eq!(
            default_hitboxes[0].action,
            ClickAction::SwitchTab { tab: 8 }
        );

        let remap_config = RenderConfig {
            template: "{{ sessions[0].state | trim | remap({\"running\": \"RUN\"}) }}".into(),
            ..RenderConfig::default()
        };
        let remap_model = RenderModel::from_runtime(&runtime, &remap_config);
        assert_eq!(render_template(&remap_model, 10, 80).unwrap().0, "RUN");

        let filter_config = RenderConfig {
            template: "{{ sessions[0].title | pane_button(pane=sessions[0].pane) }}".into(),
            ..RenderConfig::default()
        };
        let filter_model = RenderModel::from_runtime(&runtime, &filter_config);
        let (rendered, hitboxes) = render_template(&filter_model, 10, 80).unwrap();
        assert_eq!(rendered, "First Message Title");
        assert_eq!(
            hitboxes[0].action,
            ClickAction::FocusPane { pane: "1".into() }
        );

        let call_config = RenderConfig {
            template: "{%- call TabButton(tab=7) -%}{{ groups[0].tab_name }}{%- endcall -%}".into(),
            ..RenderConfig::default()
        };
        let call_model = RenderModel::from_runtime(&runtime, &call_config);
        let (rendered, hitboxes) = render_template(&call_model, 10, 80).unwrap();
        assert_eq!(rendered, "Agents");
        assert_eq!(hitboxes[0].action, ClickAction::SwitchTab { tab: 7 });
    }

    #[test]
    fn default_template_renders_sessions_without_tab_id() {
        let runtime = RuntimeState {
            sessions: BTreeMap::from([(
                "s".into(),
                AgentSession {
                    version: 1,
                    harness: Some("pi".into()),
                    session: "s".into(),
                    cwd: "/tmp/project".into(),
                    pane_id: Some("1".into()),
                    tab_id: None,
                    tab_name: Some("Agents".into()),
                    zellij_session: Some("z".into()),
                    state: AgentState::Idle,
                    model: Some("m".into()),
                    title: Some("First Message Title".into()),
                    current_task: None,
                    updated_at: 0,
                },
            )]),
            ..RuntimeState::default()
        };

        let model = RenderModel::from_runtime(&runtime, &RenderConfig::default());
        let (rendered, hitboxes) = render_template(&model, 10, 80).unwrap();

        assert!(rendered.contains("Agents"));
        assert!(!hitboxes
            .iter()
            .any(|hitbox| matches!(hitbox.action, ClickAction::SwitchTab { .. })));
    }

    #[test]
    fn renders_templates_loaded_from_disk_with_include_and_import() {
        let dir = std::env::temp_dir().join(format!(
            "zellij-agent-threads-template-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("main.j2"),
            "{% import \"macros.j2\" as ui %}{% include \"header.j2\" %} {{ ui.empty(empty_message) }}",
        )
        .unwrap();
        std::fs::write(dir.join("header.j2"), "Header").unwrap();
        std::fs::write(
            dir.join("macros.j2"),
            "{% macro empty(message) %}{{ message | upper }}{% endmacro %}",
        )
        .unwrap();

        let config = RenderConfig {
            template_dir: Some(dir.display().to_string()),
            ..RenderConfig::default()
        };
        let model = RenderModel::from_runtime(&RuntimeState::default(), &config);

        assert_eq!(
            render_template(&model, 10, 80).unwrap().0,
            "Header WAITING FOR PI EXTENSION REPORTS"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
