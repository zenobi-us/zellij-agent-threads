use std::collections::BTreeMap;

use serde::Serialize;

use crate::config::RenderConfig;
use crate::runtime::{basename, state_label, RuntimeState};

pub(crate) const DEFAULT_TEMPLATE: &str = r#"{% if sessions | length == 0 -%}
  0 Agents
{% else -%}

{% for group in groups %}
{% call TabButton(tab=group.tab_id) -%}{{ " %s " | format(group.tab_name) | bg("cyan") | fg("black") }}{%- endcall %}
{% for session in group.sessions -%}
     {% call PaneButton(pane=session.pane) -%}{{ "%3s" | format(session.pane) }} {{ session.state | remap({ "running": "🏃", "idle": "⏸️" }) }} {{ " %s " | format(session.title) }}{%- endcall %}
     🍱 {{ session.model }} {{ session.thinking_level }}
     📁 {{ session.cwd }}
     {% if session.state == "running" %}☑️  {{ session.current_task }}{% endif %}
{% endfor -%}

{% endfor %}
{% endif -%}"#;

/// Render-ready snapshot of runtime state.
///
/// This is the seam between plugin state and terminal drawing. It hides storage
/// details like `BTreeMap`/`VecDeque` from [`crate::render::Renderer`] and from
/// template rendering code.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RenderModel {
    pub(crate) collapsed: bool,
    pub(super) empty_message: String,
    pub(super) sessions: Vec<SessionLine>,
    pub(super) groups: Vec<TabGroup>,
    pub(super) events: Vec<String>,
    pub(super) template: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct TabGroup {
    tab_id: String,
    tab_name: String,
    sessions: Vec<SessionLine>,
}

/// One display row for a Pi agent session.
///
/// Values are already formatted for compact terminal output so the painter does
/// not need to know about agent payload fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct SessionLine {
    state: &'static str,
    pane: String,
    cwd: String,
    model: String,
    title: String,
    current_task: String,
}

impl RenderModel {
    /// Builds a testable render snapshot from runtime state and render config.
    pub(crate) fn from_runtime(state: &RuntimeState, config: &RenderConfig) -> Self {
        let sessions: Vec<_> = state.sessions.values().map(session_line).collect();

        let mut groups = BTreeMap::<String, TabGroup>::new();
        for session in state.sessions.values() {
            let tab_id = session
                .tab_id
                .map(|id| (id + 1).to_string())
                .unwrap_or_else(|| "?".into());
            let tab_name = session
                .tab_name
                .clone()
                .unwrap_or_else(|| "unknown tab".into());
            let key = format!("{tab_id}\0{tab_name}");
            groups
                .entry(key)
                .or_insert_with(|| TabGroup {
                    tab_id,
                    tab_name,
                    sessions: Vec::new(),
                })
                .sessions
                .push(session_line(session));
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

fn session_line(session: &crate::runtime::AgentSession) -> SessionLine {
    SessionLine {
        state: state_label(&session.state),
        pane: session.pane_id.clone().unwrap_or_else(|| "?".into()),
        cwd: basename(&session.cwd).into(),
        model: session.model.clone().unwrap_or_else(|| "?".into()),
        title: session
            .title
            .clone()
            .unwrap_or_else(|| basename(&session.cwd).into()),
        current_task: session.current_task.clone().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{AgentSession, AgentState};
    use std::collections::{BTreeMap, VecDeque};

    pub(super) fn sample_model() -> RenderModel {
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
                    title: Some("First Message Title".into()),
                    current_task: Some("Latest Task".into()),
                    updated_at: 0,
                },
            )]),
            events: VecDeque::from(["old".into(), "new".into()]),
            pipe_count: 2,
            last_error: None,
            collapsed: false,
            last_cols: 0,
        };

        RenderModel::from_runtime(&runtime, &RenderConfig::default())
    }

    #[test]
    fn builds_render_model_from_runtime() {
        let model = sample_model();

        assert_eq!(model.sessions.len(), 1);
        assert_eq!(model.groups.len(), 1);
        assert_eq!(model.events, vec!["new", "old"]);
    }
}
