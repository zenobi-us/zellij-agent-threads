//! Owns plugin runtime state and protocol handling.
//!
//! Zellij calls `main.rs` through lifecycle callbacks; this module keeps the
//! state transitions behind a small interface so the callback glue stays boring.
//! It also owns the pipe payload schema used by the Pi extension.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};
use zellij_tile::prelude::{PaneId, PipeMessage, SessionInfo};

/// Name of the Zellij pipe that receives Pi Agent Reports.
pub(crate) const AGENT_PIPE_NAME: &str = "agenthreads:agent";

/// Mutable state for one running plugin instance.
///
/// This is the plugin's session database plus small UI state. Callers should use
/// methods on this type instead of mutating fields directly when behaviour has
/// side effects, such as recording event history.
#[derive(Default)]
pub(crate) struct RuntimeState {
    pub(crate) agents: BTreeMap<String, AgentReport>,
    pub(crate) tabs: BTreeMap<usize, String>,
    pub(crate) events: VecDeque<String>,
    pub(crate) pipe_count: u64,
    pub(crate) last_error: Option<String>,
    pub(crate) focused_pane: Option<String>,
    pub(crate) active_tab: Option<usize>,
    pub(crate) active_tab_position: Option<usize>,
    pub(crate) zellij_session: Option<String>,
}

impl RuntimeState {
    /// Records plugin startup in the event log.
    pub(crate) fn load(&mut self) {
        self.push_event("plugin loaded".into());
    }

    pub(crate) fn sync_pane_focus(
        &mut self,
        manifest: &zellij_tile::prelude::PaneManifest,
    ) -> bool {
        let focused = focused_pane_for_active_tab(manifest, self.active_tab_position).map(pane_key);
        if self.focused_pane == focused {
            return false;
        }
        self.focused_pane = focused;
        true
    }

    pub(crate) fn sync_tabs(&mut self, tabs: &[zellij_tile::prelude::TabInfo]) -> bool {
        let active = tabs.iter().find(|tab| tab.active);
        let active_tab = active.map(|tab| tab.tab_id);
        let active_position = active.map(|tab| tab.position);
        let next_tabs = tabs
            .iter()
            .map(|tab| (tab.tab_id, tab.name.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut changed = self.active_tab != active_tab
            || self.active_tab_position != active_position
            || self.tabs != next_tabs;
        self.active_tab = active_tab;
        self.active_tab_position = active_position;
        self.tabs = next_tabs;

        for session in self.agents.values_mut() {
            let Some(tab) = session
                .tab_id
                .and_then(|tab_id| tabs.iter().find(|tab| tab.tab_id == tab_id))
            else {
                continue;
            };
            if session.tab_name.as_deref() != Some(tab.name.as_str()) {
                session.tab_name = Some(tab.name.clone());
                changed = true;
            }
        }

        changed
    }

    pub(crate) fn sync_current_session(&mut self, sessions: &[SessionInfo]) -> bool {
        let current = sessions
            .iter()
            .find(|session| session.is_current_session)
            .map(|session| session.name.clone());
        if self.zellij_session == current {
            return false;
        }
        self.zellij_session = current;
        true
    }

    /// Handles one Zellij pipe message.
    ///
    /// Returns `false` when the pipe name is not ours or when Zellij reports the
    /// end of a pipe stream. Bad payloads are consumed and recorded as runtime
    /// errors because retrying the same malformed message cannot help.
    pub(crate) fn handle_pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if pipe_message.name != AGENT_PIPE_NAME {
            return false;
        }

        let Some(payload) = pipe_message.payload else {
            return false;
        };

        let Ok(value) = serde_json::from_str::<serde_json::Value>(&payload) else {
            self.pipe_count += 1;
            self.last_error = Some("invalid json".into());
            self.push_event(format!("pipe #{} invalid json", self.pipe_count));
            return true;
        };

        let version = value.get("version").and_then(serde_json::Value::as_u64);
        if version != Some(2) {
            self.pipe_count += 1;
            let version = version
                .map(|value| value.to_string())
                .unwrap_or_else(|| "missing".into());
            self.last_error = Some(format!("unsupported agent report version {version}"));
            self.push_event(format!(
                "pipe #{} rejected version {version}",
                self.pipe_count
            ));
            return true;
        }

        let Ok(session) = serde_json::from_value::<AgentReport>(value) else {
            self.pipe_count += 1;
            self.last_error = Some("invalid agent report".into());
            self.push_event(format!("pipe #{} invalid agent report", self.pipe_count));
            return true;
        };

        if !self.agent_update_changes_render(&session) {
            self.apply_agent_update(session);
            return false;
        }

        self.pipe_count += 1;
        self.push_event(format!("pipe #{} bytes={}", self.pipe_count, payload.len()));
        self.push_event(format!(
            "{} {}",
            state_label(&session.state).trim(),
            basename(&session.cwd)
        ));
        self.last_error = None;
        self.apply_agent_update(session);
        true
    }

    /// Removes agents owned by a pane Zellij says has closed.
    ///
    /// Pi reports terminal pane IDs as plain numbers or `terminal_<id>` depending
    /// on source, so matching is centralized here instead of spread through
    /// callers.
    pub(crate) fn remove_agents_for_pane(&mut self, pane_id: PaneId) -> usize {
        let before = self.agents.len();
        self.agents.retain(|_, session| {
            session
                .pane_id
                .as_deref()
                .is_none_or(|session_pane_id| !pane_id_matches(session_pane_id, pane_id))
        });
        let removed = before - self.agents.len();
        if removed > 0 {
            self.push_event(format!("pane {} closed; removed {}", pane_id, removed));
        }
        removed
    }

    /// Applies the latest report for a Pi agent.
    ///
    /// `shutdown` is represented as deletion because the UI tracks active
    /// agents only; keeping closed agents would make the pane noisy over long
    /// Zellij sessions.
    fn apply_agent_update(&mut self, session: AgentReport) {
        let key = session.cache_key();
        if session.state == AgentState::Shutdown {
            self.agents.remove(&key);
        } else {
            self.agents.insert(key, session);
        }
    }

    /// Returns whether a decoded Agent Report changes anything the plugin draws.
    fn agent_update_changes_render(&self, session: &AgentReport) -> bool {
        let key = session.cache_key();
        match session.state {
            AgentState::Shutdown => self.agents.contains_key(&key),
            _ => self
                .agents
                .get(&key)
                .is_none_or(|current| !current.same_render_fields(session)),
        }
    }

    /// Appends a short diagnostic event while keeping the log bounded for tiny panes.
    fn push_event(&mut self, event: String) {
        const MAX_EVENTS: usize = 6;
        if self.events.len() == MAX_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }
}

/// JSON payload sent by the Pi extension over the Zellij pipe.
///
/// Field names intentionally mirror the TypeScript publisher. Version two is a
/// breaking protocol contract; removed version-one names are not aliases.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentReport {
    pub(crate) version: u8,
    pub(crate) harness: Option<String>,
    pub(crate) agent_id: String,
    pub(crate) session_name: Option<String>,
    pub(crate) cwd: String,
    pub(crate) zellij_session: Option<String>,
    pub(crate) pane_id: Option<String>,
    pub(crate) tab_id: Option<usize>,
    pub(crate) tab_name: Option<String>,
    pub(crate) state: AgentState,
    pub(crate) model: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) current_tool: Option<String>,
    pub(crate) updated_at: u64,
}

/// Lifecycle state for one Pi agent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AgentState {
    Idle,
    Running,
    Shutdown,
}

impl AgentReport {
    fn cache_key(&self) -> String {
        self.pane_id
            .clone()
            .unwrap_or_else(|| self.agent_id.clone())
    }
    /// Compares only fields used by the default render model/template.
    fn same_render_fields(&self, other: &Self) -> bool {
        self.cwd == other.cwd
            && self.pane_id == other.pane_id
            && self.tab_id == other.tab_id
            && self.tab_name == other.tab_name
            && self.zellij_session == other.zellij_session
            && self.harness == other.harness
            && self.agent_id == other.agent_id
            && self.session_name == other.session_name
            && self.state == other.state
            && self.model == other.model
            && self.title == other.title
            && self.current_tool == other.current_tool
    }
}

/// Returns the lowercase state label used in events and templates.
pub(crate) fn state_label(state: &AgentState) -> &'static str {
    match state {
        AgentState::Idle => "idle",
        AgentState::Running => "running",
        AgentState::Shutdown => "closed",
    }
}

/// Returns the last non-empty path segment for compact pane display.
pub(crate) fn basename(path: &str) -> &str {
    path.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(path)
}

/// Matches pane IDs across Pi's environment value and Zellij's typed pane ID.
fn pane_id_matches(session_pane_id: &str, pane_id: PaneId) -> bool {
    match pane_id {
        PaneId::Terminal(id) => {
            session_pane_id == id.to_string() || session_pane_id == format!("terminal_{id}")
        }
        PaneId::Plugin(id) => session_pane_id == format!("plugin_{id}"),
    }
}

fn pane_key(pane: &zellij_tile::prelude::PaneInfo) -> String {
    if pane.is_plugin {
        format!("plugin_{}", pane.id)
    } else {
        pane.id.to_string()
    }
}

fn focused_pane_for_active_tab(
    manifest: &zellij_tile::prelude::PaneManifest,
    active_tab_position: Option<usize>,
) -> Option<&zellij_tile::prelude::PaneInfo> {
    if let Some(position) = active_tab_position {
        return manifest
            .panes
            .get(&position)
            .and_then(|panes| largest_focused_terminal_pane(panes.iter()));
    }

    largest_focused_terminal_pane(manifest.panes.values().flat_map(|panes| panes.iter()))
}

fn largest_focused_terminal_pane<'a>(
    panes: impl Iterator<Item = &'a zellij_tile::prelude::PaneInfo>,
) -> Option<&'a zellij_tile::prelude::PaneInfo> {
    panes
        .filter(|pane| pane.is_focused && !pane.is_plugin)
        .max_by_key(|pane| pane.pane_content_rows * pane.pane_content_columns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use zellij_tile::prelude::PipeSource;

    fn session(session: &str, pane_id: Option<&str>) -> AgentReport {
        AgentReport {
            version: 2,
            harness: Some("pi".into()),
            agent_id: session.into(),
            session_name: Some(format!("{session}.jsonl")),
            cwd: "/tmp".into(),
            pane_id: pane_id.map(str::to_string),
            tab_id: None,
            tab_name: None,
            zellij_session: None,
            state: AgentState::Idle,
            model: None,
            title: None,
            current_tool: None,
            updated_at: 0,
        }
    }

    fn pipe_message(payload: AgentReport) -> PipeMessage {
        PipeMessage {
            source: PipeSource::Cli("test".into()),
            name: AGENT_PIPE_NAME.into(),
            payload: Some(serde_json::to_string(&payload).unwrap()),
            args: BTreeMap::new(),
            is_private: false,
        }
    }

    #[test]
    fn pipe_end_message_does_not_request_render() {
        let mut runtime = RuntimeState::default();
        let mut message = pipe_message(session("a", Some("1")));
        message.payload = None;

        assert!(!runtime.handle_pipe(message));
        assert_eq!(runtime.pipe_count, 0);
        assert!(runtime.last_error.is_none());
    }

    #[test]
    fn unnamespaced_agent_pipe_name_is_rejected() {
        let mut runtime = RuntimeState::default();
        let mut message = pipe_message(session("a", Some("1")));
        message.name = "agent".into();

        assert!(!runtime.handle_pipe(message));
        assert!(runtime.agents.is_empty());
    }

    #[test]
    fn current_tool_payload_updates_rendered_activity() {
        let payload = serde_json::json!({
            "version": 2,
            "harness": "pi",
            "agent_id": "a",
            "session_name": "diagnostic-session",
            "cwd": "/tmp",
            "state": "running",
            "current_tool": "bash",
            "updated_at": 0
        });

        let session: AgentReport = serde_json::from_value(payload).unwrap();

        assert_eq!(session.current_tool.as_deref(), Some("bash"));
    }

    #[test]
    fn legacy_current_task_payload_is_rejected() {
        let payload = serde_json::json!({
            "version": 2,
            "harness": "pi",
            "agent_id": "a",
            "cwd": "/tmp",
            "state": "running",
            "current_task": "legacy activity",
            "updated_at": 0
        });

        let session = serde_json::from_value::<AgentReport>(payload);

        assert!(session.is_err());
    }

    #[test]
    fn version_one_agent_reports_are_rejected_explicitly() {
        let mut runtime = RuntimeState::default();
        let mut payload = session("old", Some("1"));
        payload.version = 1;

        assert!(runtime.handle_pipe(pipe_message(payload)));
        assert!(runtime.agents.is_empty());
        assert_eq!(
            runtime.last_error.as_deref(),
            Some("unsupported agent report version 1")
        );
    }

    #[test]
    fn unchanged_agent_pipe_does_not_request_render() {
        let mut runtime = RuntimeState::default();
        let mut first = session("a", Some("1"));
        first.updated_at = 1;
        assert!(runtime.handle_pipe(pipe_message(first.clone())));

        let mut unchanged = first;
        unchanged.updated_at = 2;
        assert!(!runtime.handle_pipe(pipe_message(unchanged)));
        assert_eq!(runtime.pipe_count, 1);
    }

    #[test]
    fn same_pane_replaces_new_agent_report() {
        let mut runtime = RuntimeState::default();
        assert!(runtime.handle_pipe(pipe_message(session("old", Some("1")))));
        assert!(runtime.handle_pipe(pipe_message(session("new", Some("1")))));

        assert_eq!(runtime.agents.len(), 1);
        assert_eq!(runtime.agents["1"].agent_id, "new");
    }

    #[test]
    fn zellij_session_change_requests_render() {
        let mut runtime = RuntimeState::default();
        let first = session("a", Some("1"));
        assert!(runtime.handle_pipe(pipe_message(first)));

        let mut hidden_change = session("a", Some("1"));
        hidden_change.zellij_session = Some("renamed".into());
        assert!(runtime.handle_pipe(pipe_message(hidden_change)));
        assert_eq!(runtime.pipe_count, 2);
        assert_eq!(
            runtime.agents["1"].zellij_session.as_deref(),
            Some("renamed")
        );
    }

    #[test]
    fn current_session_rename_requests_render() {
        let mut runtime = RuntimeState::default();
        let sessions = vec![zellij_tile::prelude::SessionInfo {
            name: "old".into(),
            is_current_session: true,
            ..Default::default()
        }];
        assert!(runtime.sync_current_session(&sessions));
        assert_eq!(runtime.zellij_session.as_deref(), Some("old"));
        assert!(!runtime.sync_current_session(&sessions));

        let renamed = vec![zellij_tile::prelude::SessionInfo {
            name: "new".into(),
            is_current_session: true,
            ..Default::default()
        }];
        assert!(runtime.sync_current_session(&renamed));
        assert_eq!(runtime.zellij_session.as_deref(), Some("new"));
    }

    #[test]
    fn harness_change_requests_render() {
        let mut runtime = RuntimeState::default();
        let first = session("a", Some("1"));
        assert!(runtime.handle_pipe(pipe_message(first)));

        let mut harness_change = session("a", Some("1"));
        harness_change.harness = Some("codex".into());
        assert!(runtime.handle_pipe(pipe_message(harness_change)));
        assert_eq!(runtime.pipe_count, 2);
        assert_eq!(runtime.agents["1"].harness.as_deref(), Some("codex"));
    }
    #[test]
    fn removes_only_agents_in_closed_terminal_pane() {
        let mut runtime = RuntimeState {
            agents: BTreeMap::from([
                ("a".into(), session("a", Some("1"))),
                ("b".into(), session("b", Some("terminal_1"))),
                ("c".into(), session("c", Some("2"))),
                ("d".into(), session("d", None)),
            ]),
            ..RuntimeState::default()
        };

        assert_eq!(runtime.remove_agents_for_pane(PaneId::Terminal(1)), 2);
        assert_eq!(runtime.agents.len(), 2);
        assert!(runtime.agents.contains_key("c"));
        assert!(runtime.agents.contains_key("d"));
    }

    #[test]
    fn tracks_focused_pane_from_manifest() {
        let mut runtime = RuntimeState::default();
        let pane = zellij_tile::prelude::PaneInfo {
            id: 7,
            is_focused: true,
            ..Default::default()
        };
        let manifest = zellij_tile::prelude::PaneManifest {
            panes: HashMap::from([(0, vec![pane])]),
        };

        assert!(runtime.sync_pane_focus(&manifest));
        assert_eq!(runtime.focused_pane.as_deref(), Some("7"));
        assert_eq!(runtime.active_tab_position, None);
        assert!(!runtime.sync_pane_focus(&manifest));
    }

    #[test]
    fn tracks_focused_pane_only_from_active_tab() {
        let mut runtime = RuntimeState {
            active_tab_position: Some(1),
            ..RuntimeState::default()
        };
        let inactive_tab_pane = zellij_tile::prelude::PaneInfo {
            id: 7,
            is_focused: true,
            ..Default::default()
        };
        let active_tab_pane = zellij_tile::prelude::PaneInfo {
            id: 8,
            is_focused: true,
            ..Default::default()
        };
        let manifest = zellij_tile::prelude::PaneManifest {
            panes: HashMap::from([(0, vec![inactive_tab_pane]), (1, vec![active_tab_pane])]),
        };

        assert!(runtime.sync_pane_focus(&manifest));
        assert_eq!(runtime.focused_pane.as_deref(), Some("8"));
    }

    #[test]
    fn chooses_largest_focused_pane_when_zellij_marks_multiple_in_active_tab() {
        let mut runtime = RuntimeState {
            active_tab_position: Some(0),
            ..RuntimeState::default()
        };
        let small = zellij_tile::prelude::PaneInfo {
            id: 2,
            is_focused: true,
            pane_content_rows: 1,
            pane_content_columns: 130,
            ..Default::default()
        };
        let large = zellij_tile::prelude::PaneInfo {
            id: 9,
            is_focused: true,
            pane_content_rows: 56,
            pane_content_columns: 130,
            ..Default::default()
        };
        let manifest = zellij_tile::prelude::PaneManifest {
            panes: HashMap::from([(0, vec![small, large])]),
        };

        assert!(runtime.sync_pane_focus(&manifest));
        assert_eq!(runtime.focused_pane.as_deref(), Some("9"));
    }

    #[test]
    fn tracks_active_tab() {
        let mut runtime = RuntimeState::default();
        let tabs = vec![zellij_tile::prelude::TabInfo {
            tab_id: 3,
            active: true,
            ..Default::default()
        }];

        assert!(runtime.sync_tabs(&tabs));
        assert_eq!(runtime.active_tab, Some(3));
        assert_eq!(runtime.active_tab_position, Some(0));
        assert!(!runtime.sync_tabs(&tabs));
    }

    #[test]
    fn tab_rename_requests_render_and_updates_session() {
        let mut runtime = RuntimeState::default();
        let mut agent = session("a", Some("1"));
        agent.tab_id = Some(3);
        agent.tab_name = Some("old".into());
        runtime.agents.insert(agent.cache_key(), agent);

        let old = vec![zellij_tile::prelude::TabInfo {
            tab_id: 3,
            name: "old".into(),
            active: true,
            ..Default::default()
        }];
        assert!(runtime.sync_tabs(&old));
        assert!(!runtime.sync_tabs(&old));

        let renamed = vec![zellij_tile::prelude::TabInfo {
            name: "renamed".into(),
            ..old[0].clone()
        }];
        assert!(runtime.sync_tabs(&renamed));
        assert_eq!(runtime.agents["1"].tab_name.as_deref(), Some("renamed"));
        assert!(!runtime.sync_tabs(&renamed));
    }
}
