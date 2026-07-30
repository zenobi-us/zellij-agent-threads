//! Owns plugin runtime state and protocol handling.
//!
//! Zellij calls `main.rs` through lifecycle callbacks; this module keeps the
//! state transitions behind a small interface so the callback glue stays boring.
//! It also owns the pipe payload schema used by the Pi extension.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use zellij_tile::prelude::{PaneId, PipeMessage, SessionInfo};

/// Name of the Zellij pipe that receives Pi Agent Reports.
pub(crate) const AGENT_PIPE_NAME: &str = "agenthreads:agent";
pub(crate) const SUMMARY_PIPE_NAME: &str = "agenthreads:summary";
pub(crate) const AGENT_LEASE: Duration = Duration::from_secs(10);
pub(crate) const SESSION_POLL_INTERVAL: Duration = Duration::from_secs(10);
pub(crate) const SESSION_POLL_TIMEOUT: Duration = Duration::from_secs(3);
pub(crate) const SESSION_SUMMARY_LEASE: Duration = Duration::from_secs(30);

/// Mutable state for one running plugin instance.
///
/// This is the plugin's session database plus small UI state. Callers should use
/// methods on this type instead of mutating fields directly when behaviour has
/// side effects, such as recording event history.
#[derive(Default)]
pub(crate) struct RuntimeState {
    pub(crate) agents: BTreeMap<String, AgentReport>,
    pub(crate) agent_leases: BTreeMap<String, Duration>,
    pub(crate) session_summaries: BTreeMap<String, SessionSummary>,
    pub(crate) session_summary_leases: BTreeMap<String, Duration>,
    pub(crate) lease_clock: Duration,
    pub(crate) zellij_sessions: BTreeMap<String, ZellijSession>,
    pub(crate) poll_queue: VecDeque<PollTarget>,
    pub(crate) active_poll: Option<ActivePoll>,
    pub(crate) next_poll_at: Option<Duration>,
    pub(crate) next_poll_id: u64,
    pub(crate) session_polling_enabled: bool,
    pub(crate) tabs: BTreeMap<usize, String>,
    pub(crate) events: VecDeque<String>,
    pub(crate) pipe_count: u64,
    pub(crate) last_error: Option<String>,
    pub(crate) focused_pane: Option<String>,
    pub(crate) active_tab: Option<usize>,
    pub(crate) active_tab_position: Option<usize>,
    pub(crate) zellij_session: Option<String>,
}

/// Leased Agent counts reported by another active sidebar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SessionSummary {
    pub(crate) generation_id: String,
    pub(crate) agent_count: usize,
    pub(crate) running_agent_count: usize,
    pub(crate) fresh_at_millis: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PollTarget {
    session_name: String,
    generation_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActivePoll {
    id: u64,
    target: PollTarget,
    started_at: Duration,
}

/// Command data for one serialized Zellij session-summary poll.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PollCommand {
    pub(crate) id: u64,
    pub(crate) session_name: String,
    pub(crate) generation_id: String,
    pub(crate) payload: String,
}

impl PollCommand {
    pub(crate) fn context(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("operation".into(), "agenthreads:summary-poll".into()),
            ("poll_id".into(), self.id.to_string()),
            ("session".into(), self.session_name.clone()),
            ("generation_id".into(), self.generation_id.clone()),
        ])
    }
}

/// One native Zellij session generation reported by `SessionUpdate`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ZellijSession {
    pub(crate) generation_id: String,
    pub(crate) name: String,
    pub(crate) connected_client_count: usize,
    pub(crate) tab_count: usize,
    pub(crate) pane_count: usize,
    pub(crate) created_at_seconds: u64,
    pub(crate) current: bool,
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

    pub(crate) fn sync_zellij_sessions(&mut self, sessions: &[SessionInfo]) -> bool {
        let current = sessions
            .iter()
            .find(|session| session.is_current_session)
            .map(|session| session.name.clone());
        let next_sessions = sessions
            .iter()
            .map(native_session)
            .collect::<BTreeMap<_, _>>();
        if self.zellij_session == current && self.zellij_sessions == next_sessions {
            return false;
        }
        self.zellij_session = current;
        self.zellij_sessions = next_sessions;
        self.drop_removed_session_work();
        true
    }

    pub(crate) fn set_session_polling_enabled(&mut self, enabled: bool) -> bool {
        self.session_polling_enabled = enabled;
        if enabled {
            self.next_poll_at = Some(self.lease_clock + SESSION_POLL_INTERVAL);
            if self.last_error.as_deref() == Some("session synchronization unavailable") {
                self.last_error = None;
            }
        } else {
            self.poll_queue.clear();
            self.active_poll = None;
            self.next_poll_at = None;
            self.last_error = Some("session synchronization unavailable".into());
        }
        true
    }

    pub(crate) fn next_session_poll(&self) -> Option<Duration> {
        if !self.session_polling_enabled || !self.has_remote_sessions() {
            return None;
        }
        self.next_poll_at
            .map(|due| due.saturating_sub(self.lease_clock))
    }

    pub(crate) fn begin_session_poll_cycle(&mut self) {
        if !self.session_polling_enabled {
            return;
        }
        self.next_poll_at = Some(self.lease_clock + SESSION_POLL_INTERVAL);
        let active_generation = self
            .active_poll
            .as_ref()
            .map(|poll| poll.target.generation_id.as_str());
        let queued = self
            .poll_queue
            .iter()
            .map(|target| target.generation_id.as_str())
            .collect::<BTreeSet<_>>();
        let targets = self
            .zellij_sessions
            .values()
            .filter(|session| !session.current)
            .filter(|session| Some(session.generation_id.as_str()) != active_generation)
            .filter(|session| !queued.contains(session.generation_id.as_str()))
            .map(|session| PollTarget {
                session_name: session.name.clone(),
                generation_id: session.generation_id.clone(),
            })
            .collect::<Vec<_>>();
        self.poll_queue.extend(targets);
    }

    pub(crate) fn next_poll_command(&mut self) -> Option<PollCommand> {
        if !self.session_polling_enabled || self.active_poll.is_some() {
            return None;
        }
        while let Some(target) = self.poll_queue.pop_front() {
            let Some(session) = self.zellij_sessions.get(&target.generation_id) else {
                continue;
            };
            if session.current || session.name != target.session_name {
                continue;
            }
            self.next_poll_id += 1;
            let active = ActivePoll {
                id: self.next_poll_id,
                target,
                started_at: self.lease_clock,
            };
            let payload = serde_json::to_string(&SessionSummaryRequest {
                version: 1,
                generation_id: active.target.generation_id.clone(),
            })
            .expect("summary request serializes");
            let command = PollCommand {
                id: active.id,
                session_name: active.target.session_name.clone(),
                generation_id: active.target.generation_id.clone(),
                payload,
            };
            self.active_poll = Some(active);
            return Some(command);
        }
        None
    }

    pub(crate) fn next_poll_timeout(&self) -> Option<Duration> {
        self.active_poll.as_ref().map(|poll| {
            SESSION_POLL_TIMEOUT.saturating_sub(self.lease_clock.saturating_sub(poll.started_at))
        })
    }

    pub(crate) fn poll_timed_out(&mut self) -> bool {
        let Some(active) = &self.active_poll else {
            return false;
        };
        if self.lease_clock.saturating_sub(active.started_at) < SESSION_POLL_TIMEOUT {
            return false;
        }
        let session = active.target.session_name.clone();
        self.active_poll = None;
        self.last_error = Some(format!("session synchronization timed out for {session}"));
        true
    }

    pub(crate) fn handle_poll_result(
        &mut self,
        exit_code: Option<i32>,
        stdout: &[u8],
        stderr: &[u8],
        context: &BTreeMap<String, String>,
    ) -> bool {
        if context.get("operation").map(String::as_str) != Some("agenthreads:summary-poll") {
            return false;
        }
        let Some(active) = &self.active_poll else {
            return false;
        };
        if context.get("poll_id").and_then(|id| id.parse::<u64>().ok()) != Some(active.id) {
            return false;
        }
        if context.get("generation_id").map(String::as_str)
            != Some(active.target.generation_id.as_str())
        {
            self.active_poll = None;
            self.last_error = Some("session synchronization correlation mismatch".into());
            return true;
        }
        let generation_id = active.target.generation_id.clone();
        let session_name = active.target.session_name.clone();
        self.active_poll = None;

        if exit_code != Some(0) {
            let stderr = String::from_utf8_lossy(stderr).trim().to_string();
            self.last_error = Some(if stderr.is_empty() {
                format!("session synchronization failed for {session_name}")
            } else {
                format!("session synchronization failed for {session_name}: {stderr}")
            });
            return true;
        }

        let Ok(output) = std::str::from_utf8(stdout) else {
            self.last_error = Some(format!(
                "session synchronization returned invalid utf8 for {session_name}"
            ));
            return true;
        };
        let Ok(reply) = serde_json::from_str::<SessionSummaryReply>(output.trim()) else {
            self.last_error = Some(format!(
                "session synchronization returned malformed output for {session_name}"
            ));
            return true;
        };
        if reply.version != 1 || reply.generation_id != generation_id {
            self.last_error = Some(format!(
                "session synchronization rejected stale reply for {session_name}"
            ));
            return true;
        }

        self.session_summaries.insert(
            generation_id.clone(),
            SessionSummary {
                generation_id: reply.generation_id,
                agent_count: reply.agent_count,
                running_agent_count: reply.running_agent_count,
                fresh_at_millis: reply.fresh_at_millis,
            },
        );
        self.session_summary_leases
            .insert(generation_id, self.lease_clock);
        self.last_error = None;
        true
    }

    pub(crate) fn session_summary_output(&self, payload: Option<&str>) -> Option<String> {
        let request = serde_json::from_str::<SessionSummaryRequest>(payload?).ok()?;
        if request.version != 1 {
            return None;
        }
        let current = self
            .zellij_sessions
            .values()
            .find(|session| session.current)?;
        let running_agent_count = self
            .agents
            .values()
            .filter(|agent| agent.state == AgentState::Running)
            .count();
        serde_json::to_string(&SessionSummaryReply {
            version: 1,
            generation_id: current.generation_id.clone(),
            agent_count: self.agents.len(),
            running_agent_count,
            fresh_at_millis: self.lease_clock.as_millis() as u64,
        })
        .ok()
    }

    pub(crate) fn next_session_summary_expiry(&self) -> Option<Duration> {
        self.session_summary_leases
            .values()
            .map(|seen_at| {
                SESSION_SUMMARY_LEASE.saturating_sub(self.lease_clock.saturating_sub(*seen_at))
            })
            .min()
    }

    pub(crate) fn expire_session_summaries(&mut self) -> usize {
        let expired = self
            .session_summary_leases
            .iter()
            .filter(|(_, seen_at)| {
                self.lease_clock.saturating_sub(**seen_at) >= SESSION_SUMMARY_LEASE
            })
            .map(|(generation_id, _)| generation_id.clone())
            .collect::<Vec<_>>();
        for generation_id in &expired {
            self.session_summaries.remove(generation_id);
            self.session_summary_leases.remove(generation_id);
        }
        expired.len()
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

        self.renew_agent_lease(&session);

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
        self.agent_leases
            .retain(|key, _| self.agents.contains_key(key));
        let removed = before - self.agents.len();
        if removed > 0 {
            self.push_event(format!("pane {} closed; removed {}", pane_id, removed));
        }
        removed
    }

    pub(crate) fn advance_lease_clock_to(&mut self, now: Duration) {
        self.lease_clock = self.lease_clock.max(now);
    }

    pub(crate) fn lease_time(&self) -> Duration {
        self.lease_clock
    }

    pub(crate) fn next_agent_expiry(&self) -> Option<Duration> {
        self.agents
            .keys()
            .filter_map(|key| self.agent_leases.get(key))
            .map(|seen_at| AGENT_LEASE.saturating_sub(self.lease_clock.saturating_sub(*seen_at)))
            .min()
    }

    pub(crate) fn expire_silent_agents(&mut self) -> usize {
        let expired = self
            .agents
            .keys()
            .filter(|key| {
                self.agent_leases
                    .get(*key)
                    .is_some_and(|seen_at| self.lease_clock.saturating_sub(*seen_at) >= AGENT_LEASE)
            })
            .cloned()
            .collect::<Vec<_>>();
        for key in &expired {
            self.agents.remove(key);
            self.agent_leases.remove(key);
        }
        if !expired.is_empty() {
            self.push_event(format!("expired {} silent agent(s)", expired.len()));
        }
        expired.len()
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
            self.agent_leases.remove(&key);
        } else {
            self.agents.insert(key, session);
        }
    }

    fn renew_agent_lease(&mut self, session: &AgentReport) {
        let key = session.cache_key();
        let reported_at = Duration::from_millis(session.updated_at);
        self.advance_lease_clock_to(reported_at);
        if session.state == AgentState::Shutdown {
            self.agent_leases.remove(&key);
        } else {
            self.agent_leases.insert(key, self.lease_clock);
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

    fn has_remote_sessions(&self) -> bool {
        self.zellij_sessions
            .values()
            .any(|session| !session.current)
    }

    fn drop_removed_session_work(&mut self) {
        let live = self
            .zellij_sessions
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        self.session_summaries
            .retain(|generation_id, _| live.contains(generation_id));
        self.session_summary_leases
            .retain(|generation_id, _| live.contains(generation_id));
        self.poll_queue
            .retain(|target| live.contains(&target.generation_id));
        if self
            .active_poll
            .as_ref()
            .is_some_and(|poll| !live.contains(&poll.target.generation_id))
        {
            self.active_poll = None;
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionSummaryRequest {
    version: u8,
    generation_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionSummaryReply {
    version: u8,
    generation_id: String,
    agent_count: usize,
    running_agent_count: usize,
    fresh_at_millis: u64,
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

fn native_session(session: &SessionInfo) -> (String, ZellijSession) {
    let generation_id = session_generation_id(&session.name, session.creation_time);
    (
        generation_id.clone(),
        ZellijSession {
            generation_id,
            name: session.name.clone(),
            connected_client_count: session.connected_clients + session.web_client_count,
            tab_count: session.tabs.len(),
            pane_count: session.panes.panes.values().map(Vec::len).sum(),
            created_at_seconds: session.creation_time.as_secs(),
            current: session.is_current_session,
        },
    )
}

fn session_generation_id(_name: &str, creation_time: Duration) -> String {
    creation_time.as_nanos().to_string()
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

    fn zellij_session(name: &str, current: bool, created_at_seconds: u64) -> SessionInfo {
        SessionInfo {
            name: name.into(),
            is_current_session: current,
            creation_time: Duration::from_secs(created_at_seconds),
            ..Default::default()
        }
    }

    fn poll_reply(generation_id: &str, agent_count: usize, running_agent_count: usize) -> Vec<u8> {
        serde_json::json!({
            "version": 1,
            "generation_id": generation_id,
            "agent_count": agent_count,
            "running_agent_count": running_agent_count,
            "fresh_at_millis": 42
        })
        .to_string()
        .into_bytes()
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
    fn accepted_agent_report_renews_lease_without_render_change() {
        let mut runtime = RuntimeState::default();
        let mut first = session("a", Some("1"));
        first.updated_at = 1_000;
        assert!(runtime.handle_pipe(pipe_message(first.clone())));

        let mut heartbeat = first;
        heartbeat.updated_at = 6_000;
        assert!(!runtime.handle_pipe(pipe_message(heartbeat)));
        runtime.advance_lease_clock_to(Duration::from_millis(15_999));
        assert_eq!(runtime.expire_silent_agents(), 0);
        runtime.advance_lease_clock_to(Duration::from_millis(16_000));
        assert_eq!(runtime.expire_silent_agents(), 1);
    }

    #[test]
    fn session_polling_queues_remote_sessions_one_at_a_time() {
        let mut runtime = RuntimeState::default();
        runtime.sync_zellij_sessions(&[
            zellij_session("local", true, 1),
            zellij_session("alpha", false, 2),
            zellij_session("beta", false, 3),
        ]);
        runtime.set_session_polling_enabled(true);
        runtime.advance_lease_clock_to(SESSION_POLL_INTERVAL);

        assert_eq!(runtime.next_session_poll(), Some(Duration::ZERO));
        runtime.begin_session_poll_cycle();
        runtime.begin_session_poll_cycle();

        let first = runtime.next_poll_command().unwrap();
        assert_eq!(first.session_name, "alpha");
        assert!(runtime.next_poll_command().is_none());

        assert!(runtime.handle_poll_result(
            Some(0),
            &poll_reply(&first.generation_id, 2, 1),
            &[],
            &first.context(),
        ));
        assert_eq!(
            runtime.session_summaries[&first.generation_id].agent_count,
            2
        );
        assert_eq!(
            runtime.next_session_summary_expiry(),
            Some(SESSION_SUMMARY_LEASE)
        );

        let second = runtime.next_poll_command().unwrap();
        assert_eq!(second.session_name, "beta");
        assert!(runtime.next_poll_command().is_none());
    }

    #[test]
    fn poll_failures_and_timeouts_do_not_stall_queue() {
        let mut runtime = RuntimeState::default();
        runtime.sync_zellij_sessions(&[
            zellij_session("local", true, 1),
            zellij_session("alpha", false, 2),
            zellij_session("beta", false, 3),
        ]);
        runtime.set_session_polling_enabled(true);
        runtime.begin_session_poll_cycle();

        let first = runtime.next_poll_command().unwrap();
        assert!(runtime.handle_poll_result(Some(1), &[], b"no sidebar", &first.context(),));
        assert_eq!(
            runtime.last_error.as_deref(),
            Some("session synchronization failed for alpha: no sidebar")
        );

        let second = runtime.next_poll_command().unwrap();
        assert_eq!(second.session_name, "beta");
        runtime.advance_lease_clock_to(SESSION_POLL_TIMEOUT);
        assert!(runtime.poll_timed_out());
        assert!(runtime.next_poll_command().is_none());
    }

    #[test]
    fn stale_or_malformed_poll_replies_are_rejected() {
        let mut runtime = RuntimeState::default();
        runtime.sync_zellij_sessions(&[
            zellij_session("local", true, 1),
            zellij_session("alpha", false, 2),
        ]);
        runtime.set_session_polling_enabled(true);
        runtime.begin_session_poll_cycle();

        let command = runtime.next_poll_command().unwrap();
        assert!(runtime.handle_poll_result(Some(0), b"not-json", &[], &command.context()));
        assert!(runtime.session_summaries.is_empty());

        runtime.begin_session_poll_cycle();
        let command = runtime.next_poll_command().unwrap();
        assert!(runtime.handle_poll_result(
            Some(0),
            &poll_reply("old-generation", 1, 1),
            &[],
            &command.context(),
        ));
        assert!(runtime.session_summaries.is_empty());
    }

    #[test]
    fn session_removal_drops_summary_and_queued_poll_work() {
        let mut runtime = RuntimeState::default();
        runtime.sync_zellij_sessions(&[
            zellij_session("local", true, 1),
            zellij_session("alpha", false, 2),
            zellij_session("beta", false, 3),
        ]);
        runtime.session_summaries.insert(
            "2000000000".into(),
            SessionSummary {
                generation_id: "2000000000".into(),
                agent_count: 1,
                running_agent_count: 1,
                fresh_at_millis: 0,
            },
        );
        runtime
            .session_summary_leases
            .insert("2000000000".into(), Duration::ZERO);
        runtime.set_session_polling_enabled(true);
        runtime.begin_session_poll_cycle();
        let active = runtime.next_poll_command().unwrap();
        assert_eq!(active.generation_id, "2000000000");

        runtime.sync_zellij_sessions(&[zellij_session("local", true, 1)]);

        assert!(runtime.session_summaries.is_empty());
        assert!(runtime.session_summary_leases.is_empty());
        assert!(runtime.next_poll_command().is_none());
        assert!(runtime.next_poll_timeout().is_none());
    }

    #[test]
    fn permission_denial_keeps_remote_counts_unavailable() {
        let mut runtime = RuntimeState::default();
        runtime.sync_zellij_sessions(&[
            zellij_session("local", true, 1),
            zellij_session("alpha", false, 2),
        ]);

        assert!(runtime.set_session_polling_enabled(false));
        runtime.begin_session_poll_cycle();

        assert!(runtime.next_poll_command().is_none());
        assert_eq!(
            runtime.last_error.as_deref(),
            Some("session synchronization unavailable")
        );
    }

    #[test]
    fn summary_pipe_returns_only_generation_counts_and_freshness() {
        let mut runtime = RuntimeState::default();
        let mut running = session("running", Some("1"));
        running.state = AgentState::Running;
        runtime.agents.insert("1".into(), running);
        runtime
            .agents
            .insert("2".into(), session("idle", Some("2")));
        runtime.sync_zellij_sessions(&[zellij_session("local", true, 7)]);
        runtime.advance_lease_clock_to(Duration::from_millis(12));

        let output = runtime
            .session_summary_output(Some(r#"{"version":1,"generation_id":"7000000000"}"#))
            .unwrap();
        let value = serde_json::from_str::<serde_json::Value>(&output).unwrap();

        assert_eq!(value["generation_id"], "7000000000");
        assert_eq!(value["agent_count"], 2);
        assert_eq!(value["running_agent_count"], 1);
        assert_eq!(value["fresh_at_millis"], 12);
        assert_eq!(value.as_object().unwrap().len(), 5);
    }

    #[test]
    fn agent_expires_after_ten_seconds_without_report() {
        let mut runtime = RuntimeState::default();
        assert!(runtime.handle_pipe(pipe_message(session("a", Some("1")))));

        runtime.advance_lease_clock_to(AGENT_LEASE - Duration::from_millis(1));
        assert_eq!(runtime.expire_silent_agents(), 0);
        assert!(runtime.agents.contains_key("1"));

        runtime.advance_lease_clock_to(AGENT_LEASE);
        assert_eq!(runtime.expire_silent_agents(), 1);
        assert!(runtime.agents.is_empty());
    }

    #[test]
    fn expiry_removes_only_stale_agents() {
        let mut runtime = RuntimeState::default();
        let mut old = session("old", Some("1"));
        old.updated_at = 1_000;
        let mut fresh = session("fresh", Some("2"));
        fresh.updated_at = 9_000;

        assert!(runtime.handle_pipe(pipe_message(old)));
        assert!(runtime.handle_pipe(pipe_message(fresh)));
        runtime.advance_lease_clock_to(Duration::from_millis(11_000));

        assert_eq!(runtime.expire_silent_agents(), 1);
        assert!(!runtime.agents.contains_key("1"));
        assert!(runtime.agents.contains_key("2"));
        assert_eq!(
            runtime.next_agent_expiry(),
            Some(Duration::from_millis(8_000))
        );
    }

    #[test]
    fn shutdown_and_pane_closure_remove_leases_immediately() {
        let mut runtime = RuntimeState::default();
        assert!(runtime.handle_pipe(pipe_message(session("a", Some("1")))));
        assert!(runtime.next_agent_expiry().is_some());

        let mut shutdown = session("a", Some("1"));
        shutdown.state = AgentState::Shutdown;
        assert!(runtime.handle_pipe(pipe_message(shutdown)));
        assert!(runtime.agents.is_empty());
        assert_eq!(runtime.next_agent_expiry(), None);

        assert!(runtime.handle_pipe(pipe_message(session("b", Some("2")))));
        assert_eq!(runtime.remove_agents_for_pane(PaneId::Terminal(2)), 1);
        assert_eq!(runtime.next_agent_expiry(), None);
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
            creation_time: std::time::Duration::from_secs(10),
            ..Default::default()
        }];
        assert!(runtime.sync_zellij_sessions(&sessions));
        assert_eq!(runtime.zellij_session.as_deref(), Some("old"));
        assert!(!runtime.sync_zellij_sessions(&sessions));
        let original_generation = runtime
            .zellij_sessions
            .values()
            .next()
            .unwrap()
            .generation_id
            .clone();

        let renamed = vec![zellij_tile::prelude::SessionInfo {
            name: "new".into(),
            is_current_session: true,
            creation_time: std::time::Duration::from_secs(10),
            ..Default::default()
        }];
        assert!(runtime.sync_zellij_sessions(&renamed));
        assert_eq!(runtime.zellij_session.as_deref(), Some("new"));
        assert_eq!(runtime.zellij_sessions.len(), 1);
        assert!(runtime
            .zellij_sessions
            .values()
            .any(|session| session.name == "new"));
        assert_eq!(
            runtime
                .zellij_sessions
                .values()
                .next()
                .unwrap()
                .generation_id,
            original_generation
        );
    }

    #[test]
    fn native_sessions_replace_removed_records_and_keep_generation_identity() {
        let mut runtime = RuntimeState::default();
        let first = zellij_tile::prelude::SessionInfo {
            name: "work".into(),
            is_current_session: true,
            connected_clients: 2,
            web_client_count: 1,
            creation_time: std::time::Duration::from_secs(10),
            tabs: vec![zellij_tile::prelude::TabInfo::default()],
            panes: zellij_tile::prelude::PaneManifest {
                panes: HashMap::from([(0, vec![zellij_tile::prelude::PaneInfo::default()])]),
            },
            ..Default::default()
        };
        let other = zellij_tile::prelude::SessionInfo {
            name: "other".into(),
            creation_time: std::time::Duration::from_secs(20),
            ..Default::default()
        };

        assert!(runtime.sync_zellij_sessions(&[first.clone(), other]));
        assert_eq!(runtime.zellij_sessions.len(), 2);
        let original_generation = runtime
            .zellij_sessions
            .values()
            .find(|session| session.name == "work")
            .unwrap()
            .generation_id
            .clone();

        let recreated = zellij_tile::prelude::SessionInfo {
            creation_time: std::time::Duration::from_secs(30),
            ..first
        };
        assert!(runtime.sync_zellij_sessions(&[recreated]));
        assert_eq!(runtime.zellij_sessions.len(), 1);
        let session = runtime.zellij_sessions.values().next().unwrap();
        assert_eq!(session.name, "work");
        assert_ne!(session.generation_id, original_generation);
        assert_eq!(session.connected_client_count, 3);
        assert_eq!(session.tab_count, 1);
        assert_eq!(session.pane_count, 1);
        assert_eq!(session.created_at_seconds, 30);
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
