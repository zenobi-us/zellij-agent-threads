use std::collections::BTreeMap;

use serde::Serialize;

use crate::config::RenderConfig;
use crate::runtime::{basename, state_label, RuntimeState};

pub(crate) const DEFAULT_TEMPLATE: &str = r#"{% if sessions | length == 0 -%}
  0 Agents
{% else -%}
{% if has_error %}{{ " pipe error " | bg("red") | fg("white") }} {{ last_error | italic }}
{% endif -%}
{{ " %s " | format(zellij_session) | bg("yellow") | fg("black") }}
{% for group in groups %}
{% set tab_label = " %s " | format(group.tab_name) -%}
{% call TabButton(tab=group.tab_id) -%}{{ tab_label | bg("cyan") | fg("black") if group.active else tab_label | dim }}{%- endcall %}

{% for session in group.sessions -%}
     {% call PaneButton(pane=session.pane) -%}
     {% set title = " %s " | format(session.title) -%}
     {{ "%3s" | format(session.pane) | bold if session.focused else "%3s" | format(session.pane) }} {{ session.state | remap({ "running": "🏃", "idle": "⏸️" }) | fg("green") if session.state == "running" else session.state | remap({ "running": "🏃", "idle": "⏸️" }) | dim }} {{ title | bold if session.focused else title | dim if not session.active_tab else title }}
     {{ "🍱 %s" | format(session.model) | dim }}
     {{ "📁 %s" | format(session.cwd) | dim }}
     {% if session.state == "running" %}☑️  {{ session.current_task }}{% endif %}
     {%- endcall %}

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
    pub(super) zellij_session: String,
    pub(super) harness: String,
    pub(super) groups: Vec<TabGroup>,
    pub(super) events: Vec<String>,
    pub(super) template: String,
    pub(super) has_error: bool,
    pub(super) last_error: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct TabGroup {
    tab_id: String,
    tab_name: String,
    sessions: Vec<SessionLine>,
    active: bool,
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
    zellij_session: String,
    harness: String,
    current_task: String,
    focused: bool,
    active_tab: bool,
}

impl RenderModel {
    /// Builds a testable render snapshot from runtime state and render config.
    pub(crate) fn from_runtime(state: &RuntimeState, config: &RenderConfig) -> Self {
        let sessions: Vec<_> = state
            .sessions
            .values()
            .map(|session| session_line(session, state))
            .collect();
        let zellij_session = state
            .sessions
            .values()
            .find_map(|session| session.zellij_session.clone())
            .unwrap_or_else(|| "?".into());
        let harness = state
            .sessions
            .values()
            .find_map(|session| session.harness.clone())
            .unwrap_or_else(|| "?".into());

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
            let active = session.tab_id == state.active_tab;
            let key = format!("{tab_id}\0{tab_name}");
            groups
                .entry(key)
                .or_insert_with(|| TabGroup {
                    tab_id,
                    tab_name,
                    active,
                    sessions: Vec::new(),
                })
                .sessions
                .push(session_line(session, state));
        }

        Self {
            collapsed: state.collapsed,
            empty_message: config.empty_message.clone(),
            sessions,
            zellij_session,
            harness,
            groups: groups.into_values().collect(),
            events: state.events.iter().rev().cloned().collect(),
            template: config.template.clone(),
            has_error: state.last_error.is_some(),
            last_error: state.last_error.clone().unwrap_or_default(),
        }
    }
}

fn session_line(session: &crate::runtime::AgentSession, state: &RuntimeState) -> SessionLine {
    let pane = session.pane_id.clone().unwrap_or_else(|| "?".into());
    SessionLine {
        state: state_label(&session.state),
        focused: state.focused_pane.as_deref() == Some(pane.as_str()),
        active_tab: session.tab_id == state.active_tab,
        pane,
        cwd: basename(&session.cwd).into(),
        model: session.model.clone().unwrap_or_else(|| "?".into()),
        zellij_session: session.zellij_session.clone().unwrap_or_else(|| "?".into()),
        harness: session.harness.clone().unwrap_or_else(|| "?".into()),
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
            collapsed: false,
            last_cols: 0,
            focused_pane: Some("1".into()),
            active_tab: Some(7),
        };

        RenderModel::from_runtime(&runtime, &RenderConfig::default())
    }

    #[test]
    fn builds_render_model_from_runtime() {
        let model = sample_model();

        assert_eq!(model.sessions.len(), 1);
        assert_eq!(model.groups.len(), 1);
        assert_eq!(model.events, vec!["new", "old"]);
        assert!(model.groups[0].active);
        assert!(model.sessions[0].focused);
        assert!(model.sessions[0].active_tab);
        assert_eq!(model.sessions[0].zellij_session, "z");
        assert_eq!(model.zellij_session, "z");
        assert_eq!(model.sessions[0].harness, "pi");
        assert_eq!(model.harness, "pi");
    }
}
