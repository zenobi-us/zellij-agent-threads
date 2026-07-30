use serde::Serialize;

use crate::config::RenderConfig;
use crate::runtime::{basename, state_label, RuntimeState, ZellijSession};

/// Render-ready snapshot of runtime state.
///
/// This is the seam between plugin state and terminal drawing. It hides storage
/// details like `BTreeMap`/`VecDeque` from [`crate::render::Renderer`] and from
/// template rendering code.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RenderModel {
    pub(super) empty_message: String,
    pub(super) agents: Vec<AgentLine>,
    pub(super) sessions: Vec<SessionLine>,
    pub(super) zellij_session: String,
    pub(super) harness: String,
    pub(super) tabs: Vec<TabLine>,
    pub(super) events: Vec<String>,
    pub(super) has_error: bool,
    pub(super) last_error: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct TabLine {
    tab_id: Option<usize>,
    tab_name: String,
    agents: Vec<AgentLine>,
    active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct SessionLine {
    generation_id: String,
    name: String,
    status: &'static str,
    agent_count: usize,
    connected_client_count: usize,
    tab_count: usize,
    pane_count: usize,
    created_at_seconds: u64,
    current: bool,
}

/// One display row for a Pi agent.
///
/// Values are already formatted for compact terminal output so the painter does
/// not need to know about agent payload fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct AgentLine {
    agent_id: String,
    session_name: String,
    state: &'static str,
    pane: String,
    cwd: String,
    model: String,
    title: String,
    zellij_session: String,
    harness: String,
    current_tool: String,
    focused: bool,
    active_tab: bool,
}

impl RenderModel {
    /// Builds a testable render snapshot from runtime state and render config.
    pub(crate) fn from_runtime(state: &RuntimeState, config: &RenderConfig) -> Self {
        let agents: Vec<_> = state
            .agents
            .values()
            .map(|session| agent_line(session, state))
            .collect();
        let zellij_session = state
            .zellij_session
            .clone()
            .or_else(|| {
                state
                    .agents
                    .values()
                    .find_map(|session| session.zellij_session.clone())
            })
            .unwrap_or_else(|| "?".into());
        let harness = state
            .agents
            .values()
            .find_map(|session| session.harness.clone())
            .unwrap_or_else(|| "?".into());
        let mut sessions: Vec<_> = state
            .zellij_sessions
            .values()
            .map(|session| session_line(session, state))
            .collect();
        sessions.sort_by(|left, right| {
            right
                .current
                .cmp(&left.current)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
                .then_with(|| left.name.cmp(&right.name))
        });

        let mut tabs: Vec<_> = state
            .tabs
            .iter()
            .map(|(tab_id, tab_name)| TabLine {
                tab_id: Some(tab_id + 1),
                tab_name: tab_name.clone(),
                active: Some(*tab_id) == state.active_tab,
                agents: state
                    .agents
                    .values()
                    .filter(|session| session.tab_id == Some(*tab_id))
                    .map(|session| agent_line(session, state))
                    .collect(),
            })
            .collect();

        mark_single_agent_active_tabs(&mut tabs);
        Self {
            empty_message: config.empty_message.clone(),
            agents,
            sessions,
            zellij_session,
            harness,
            tabs,
            events: state.events.iter().rev().cloned().collect(),
            has_error: state.last_error.is_some(),
            last_error: state.last_error.clone().unwrap_or_default(),
        }
    }

    pub(super) fn active_tab(&self) -> Option<u32> {
        self.tabs
            .iter()
            .find(|tab| tab.active)
            .and_then(|tab| tab.tab_id)
            .and_then(|tab| u32::try_from(tab).ok())
    }

    pub(super) fn focused_pane(&self) -> Option<&str> {
        self.agents
            .iter()
            .find(|agent| agent.focused)
            .map(|agent| agent.pane.as_str())
    }

    pub(super) fn layout_fill(&self, viewport_rows: usize) -> String {
        let session_rows = if self.sessions.is_empty() {
            1
        } else {
            1 + self.sessions.len()
        };
        let agent_rows = if self.agents.is_empty() {
            0
        } else {
            1 + self
                .tabs
                .iter()
                .filter(|tab| !tab.agents.is_empty())
                .map(|tab| {
                    1 + tab
                        .agents
                        .iter()
                        .map(|agent| 3 + usize::from(agent.state == "running"))
                        .sum::<usize>()
                })
                .sum::<usize>()
        };
        let event_rows = if self.agents.is_empty() {
            0
        } else {
            1 + self.events.len() + usize::from(self.has_error)
        };
        blank_rows(viewport_rows.saturating_sub(4 + session_rows + agent_rows + event_rows))
    }
}

fn session_line(session: &ZellijSession, state: &RuntimeState) -> SessionLine {
    SessionLine {
        generation_id: session.generation_id.clone(),
        name: session.name.clone(),
        status: if session.current { "current" } else { "active" },
        agent_count: agent_count_for_session(session, state),
        connected_client_count: session.connected_client_count,
        tab_count: session.tab_count,
        pane_count: session.pane_count,
        created_at_seconds: session.created_at_seconds,
        current: session.current,
    }
}

fn agent_count_for_session(session: &ZellijSession, state: &RuntimeState) -> usize {
    if !session.current {
        return state
            .agents
            .values()
            .filter(|agent| agent.zellij_session.as_deref() == Some(session.name.as_str()))
            .count();
    }

    state
        .agents
        .values()
        .filter(|agent| {
            let Some(agent_session) = agent.zellij_session.as_deref() else {
                return true;
            };
            agent_session == session.name
                || !state
                    .zellij_sessions
                    .values()
                    .any(|native| !native.current && native.name == agent_session)
        })
        .count()
}

fn blank_rows(rows: usize) -> String {
    std::iter::repeat_n(" ", rows)
        .collect::<Vec<_>>()
        .join("\n")
}

fn agent_line(session: &crate::runtime::AgentReport, state: &RuntimeState) -> AgentLine {
    let pane = session.pane_id.clone().unwrap_or_else(|| "?".into());
    AgentLine {
        agent_id: session.agent_id.clone(),
        session_name: session.session_name.clone().unwrap_or_default(),
        state: state_label(&session.state),
        focused: state.focused_pane.as_deref() == Some(pane.as_str()),
        active_tab: session.tab_id == state.active_tab,
        pane,
        cwd: basename(&session.cwd).into(),
        model: session.model.clone().unwrap_or_else(|| "?".into()),
        zellij_session: state
            .zellij_session
            .clone()
            .or_else(|| session.zellij_session.clone())
            .unwrap_or_else(|| "?".into()),
        harness: session.harness.clone().unwrap_or_else(|| "?".into()),
        title: session
            .title
            .clone()
            .unwrap_or_else(|| basename(&session.cwd).into()),
        current_tool: session.current_tool.clone().unwrap_or_default(),
    }
}

fn mark_single_agent_active_tabs(tabs: &mut [TabLine]) {
    for tab in tabs {
        if tab.active && tab.agents.len() == 1 && !tab.agents[0].focused {
            tab.agents[0].focused = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{AgentReport, AgentState, ZellijSession};
    use std::collections::{BTreeMap, VecDeque};

    pub(super) fn sample_model() -> RenderModel {
        let runtime = RuntimeState {
            agents: BTreeMap::from([(
                "s".into(),
                AgentReport {
                    session_name: Some("diagnostic-session".into()),
                    current_tool: Some("bash".into()),
                    ..agent_report("s", "1", "First Message Title")
                },
            )]),
            events: VecDeque::from(["old".into(), "new".into()]),
            pipe_count: 2,
            last_error: None,
            focused_pane: Some("1".into()),
            active_tab: Some(7),
            active_tab_position: Some(0),
            tabs: BTreeMap::from([(7, "Agents".into())]),
            zellij_session: None,
            zellij_sessions: BTreeMap::new(),
            ..RuntimeState::default()
        };
        RenderModel::from_runtime(&runtime, &RenderConfig::default())
    }

    fn agent_report(agent_id: &str, pane: &str, title: &str) -> AgentReport {
        AgentReport {
            version: 2,
            harness: Some("pi".into()),
            agent_id: agent_id.into(),
            session_name: None,
            cwd: "/tmp/project".into(),
            pane_id: Some(pane.into()),
            tab_id: Some(7),
            tab_name: Some("Agents".into()),
            zellij_session: Some("z".into()),
            state: AgentState::Running,
            model: Some("m".into()),
            title: Some(title.into()),
            current_tool: None,
            updated_at: 0,
        }
    }

    #[test]
    fn builds_render_model_from_runtime() {
        let model = sample_model();

        assert_eq!(model.agents.len(), 1);
        assert_eq!(model.tabs.len(), 1);
        assert_eq!(model.events, vec!["new", "old"]);
        assert!(model.tabs[0].active);
        assert!(model.agents[0].focused);
        assert!(model.agents[0].active_tab);
        assert_eq!(model.agents[0].zellij_session, "z");
        assert_eq!(model.zellij_session, "z");

        assert_eq!(model.agents[0].harness, "pi");
        assert_eq!(model.agents[0].agent_id, "s");
        assert_eq!(model.agents[0].session_name, "diagnostic-session");
        assert_eq!(model.agents[0].current_tool, "bash");
        assert_eq!(model.tabs[0].agents.len(), 1);
        assert_eq!(model.harness, "pi");
    }

    #[test]
    fn sessions_sort_current_first_then_case_insensitive_name() {
        let mut remote_agent = agent_report("remote-agent", "2", "Other");
        remote_agent.zellij_session = Some("beta".into());
        let mut local_agent = agent_report("local", "1", "Current");
        local_agent.zellij_session = Some("Alpha".into());
        let runtime = RuntimeState {
            agents: BTreeMap::from([
                ("local".into(), local_agent),
                ("remote".into(), remote_agent),
            ]),
            zellij_sessions: BTreeMap::from([
                (
                    "beta:1".into(),
                    ZellijSession {
                        generation_id: "beta:1".into(),
                        name: "beta".into(),
                        connected_client_count: 2,
                        tab_count: 3,
                        pane_count: 4,
                        created_at_seconds: 10,
                        current: false,
                    },
                ),
                (
                    "Alpha:1".into(),
                    ZellijSession {
                        generation_id: "Alpha:1".into(),
                        name: "Alpha".into(),
                        connected_client_count: 1,
                        tab_count: 1,
                        pane_count: 1,
                        created_at_seconds: 20,
                        current: true,
                    },
                ),
                (
                    "aardvark:1".into(),
                    ZellijSession {
                        generation_id: "aardvark:1".into(),
                        name: "aardvark".into(),
                        connected_client_count: 0,
                        tab_count: 0,
                        pane_count: 0,
                        created_at_seconds: 30,
                        current: false,
                    },
                ),
            ]),
            zellij_session: Some("Alpha".into()),
            ..RuntimeState::default()
        };

        let model = RenderModel::from_runtime(&runtime, &RenderConfig::default());

        assert_eq!(model.sessions[0].name, "Alpha");
        assert_eq!(model.sessions[1].name, "aardvark");
        assert_eq!(model.sessions[2].name, "beta");
        assert_eq!(model.sessions[0].agent_count, 1);
        assert_eq!(model.sessions[1].agent_count, 0);
        assert_eq!(model.sessions[2].agent_count, 1);
        assert_eq!(model.sessions[0].created_at_seconds, 20);
    }

    #[test]
    fn current_session_counts_agents_when_pipe_session_name_is_stale() {
        let mut stale_agent = agent_report("local", "1", "Current");
        stale_agent.zellij_session = Some("old-name".into());
        let runtime = RuntimeState {
            agents: BTreeMap::from([("local".into(), stale_agent)]),
            zellij_sessions: BTreeMap::from([(
                "10".into(),
                ZellijSession {
                    generation_id: "10".into(),
                    name: "new-name".into(),
                    connected_client_count: 1,
                    tab_count: 1,
                    pane_count: 1,
                    created_at_seconds: 10,
                    current: true,
                },
            )]),
            zellij_session: Some("new-name".into()),
            ..RuntimeState::default()
        };

        let model = RenderModel::from_runtime(&runtime, &RenderConfig::default());

        assert_eq!(model.sessions[0].name, "new-name");
        assert_eq!(model.sessions[0].agent_count, 1);
    }

    #[test]
    fn single_agent_in_active_tab_is_marked_focused_when_zellij_focus_missing() {
        let runtime = RuntimeState {
            active_tab: Some(7),
            tabs: BTreeMap::from([(7, "Agents".into())]),
            agents: BTreeMap::from([(
                "s".into(),
                AgentReport {
                    version: 2,
                    harness: Some("pi".into()),
                    agent_id: "s".into(),
                    session_name: None,
                    cwd: "/tmp/project".into(),
                    pane_id: Some("1".into()),
                    tab_id: Some(7),
                    tab_name: Some("Agents".into()),
                    zellij_session: Some("z".into()),
                    state: AgentState::Running,
                    model: None,
                    title: None,
                    current_tool: None,
                    updated_at: 0,
                },
            )]),
            ..RuntimeState::default()
        };

        let model = RenderModel::from_runtime(&runtime, &RenderConfig::default());

        assert!(model.tabs[0].active);
        assert!(model.tabs[0].agents[0].focused);
    }

    #[test]
    fn agents_without_matching_tab_metadata_remain_flat_only() {
        let runtime = RuntimeState {
            tabs: BTreeMap::from([(9, "Empty".into())]),
            agents: BTreeMap::from([(
                "s".into(),
                AgentReport {
                    version: 2,
                    harness: Some("pi".into()),
                    agent_id: "s".into(),
                    session_name: None,
                    cwd: "/tmp/project".into(),
                    pane_id: Some("1".into()),
                    tab_id: Some(7),
                    tab_name: Some("Stale".into()),
                    zellij_session: Some("z".into()),
                    state: AgentState::Idle,
                    model: None,
                    title: None,
                    current_tool: None,
                    updated_at: 0,
                },
            )]),
            ..RuntimeState::default()
        };

        let model = RenderModel::from_runtime(&runtime, &RenderConfig::default());

        assert_eq!(model.agents.len(), 1);
        assert_eq!(model.tabs.len(), 1);
        assert!(model.tabs[0].agents.is_empty());
    }

    #[test]
    fn zellij_session_event_name_overrides_stale_pipe_name() {
        let runtime = RuntimeState {
            zellij_session: Some("renamed".into()),
            ..RuntimeState::default()
        };
        let model = RenderModel::from_runtime(&runtime, &RenderConfig::default());
        assert_eq!(model.zellij_session, "renamed");
    }
}
