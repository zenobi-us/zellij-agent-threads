use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};
use zellij_tile::prelude::*;

const PIPE_NAME: &str = "pi-agent-session";

#[derive(Default)]
struct PluginState {
    sessions: BTreeMap<String, AgentSession>,
    events: VecDeque<String>,
    pipe_count: u64,
    last_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct AgentSession {
    version: u8,
    session: String,
    cwd: String,
    pane_id: Option<String>,
    state: AgentState,
    model: Option<String>,
    updated_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AgentState {
    Idle,
    Running,
    Shutdown,
}

register_plugin!(PluginState);

impl ZellijPlugin for PluginState {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        // runs once on plugin load, provides the configuration with which this plugin was loaded
        // (if any)
        //
        // this is a good place to `subscribe` (https://docs.rs/zellij-tile/latest/zellij_tile/shim/fn.subscribe.html)
        // to `Event`s (https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.Event.html)
        // and `request_permissions` (https://docs.rs/zellij-tile/latest/zellij_tile/shim/fn.request_permission.html)
        subscribe(&[EventType::PaneClosed]);
        set_selectable(false);
        push_event(&mut self.events, "plugin loaded".into());
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if pipe_message.name != PIPE_NAME {
            return false;
        }

        self.pipe_count += 1;
        let Some(payload) = pipe_message.payload else {
            self.last_error = Some("empty payload".into());
            push_event(
                &mut self.events,
                format!("pipe #{} empty payload", self.pipe_count),
            );
            return true;
        };

        push_event(
            &mut self.events,
            format!("pipe #{} bytes={}", self.pipe_count, payload.len()),
        );

        let Ok(session) = serde_json::from_str::<AgentSession>(&payload) else {
            self.last_error = Some("invalid json".into());
            push_event(
                &mut self.events,
                format!("pipe #{} invalid json", self.pipe_count),
            );
            return true;
        };

        push_event(
            &mut self.events,
            format!(
                "{} {}",
                state_label(&session.state).trim(),
                basename(&session.cwd)
            ),
        );
        self.last_error = None;
        apply_session_update(&mut self.sessions, session);
        true
    }

    fn render(&mut self, rows: usize, cols: usize) {

        clear_screen();

        if rows == 0 || cols == 0 {
            return;
        }

        let status = if let Some(error) = &self.last_error {
            format!("zellij-agent pipes={} error={}", self.pipe_count, error)
        } else {
            format!(
                "zellij-agent pipes={} sessions={}",
                self.pipe_count,
                self.sessions.len()
            )
        };
        print_line(0, cols, &status);

        if rows <= 1 {
            return;
        }

        if self.sessions.is_empty() {
            print_line(1, cols, "waiting for pi extension reports");
        } else {
            for (row, session) in self.sessions.values().take(rows - 1).enumerate() {
                let pane = session.pane_id.as_deref().unwrap_or("?");
                let model = session.model.as_deref().unwrap_or("?");
                let line = format!(
                    "{} pane={} {} {}",
                    state_label(&session.state),
                    pane,
                    basename(&session.cwd),
                    model,
                );
                print_line(row + 1, cols, &line);
            }
        }

        let first_event_row = if self.sessions.is_empty() {
            2
        } else {
            self.sessions.len() + 1
        };
        for (offset, event) in self
            .events
            .iter()
            .rev()
            .take(rows.saturating_sub(first_event_row))
            .enumerate()
        {
            print_line(first_event_row + offset, cols, event);
        }
    }

    fn update(&mut self, event: Event) -> bool {
        if let Event::PaneClosed(pane_id) = event {
            let removed = remove_sessions_for_pane(&mut self.sessions, pane_id);
            if removed > 0 {
                push_event(
                    &mut self.events,
                    format!("pane {} closed; removed {}", pane_id, removed),
                );
            }
        }

        true
    }

}

fn apply_session_update(sessions: &mut BTreeMap<String, AgentSession>, session: AgentSession) {
    if session.state == AgentState::Shutdown {
        sessions.remove(&session.session);
    } else {
        sessions.insert(session.session.clone(), session);
    }
}

fn remove_sessions_for_pane(
    sessions: &mut BTreeMap<String, AgentSession>,
    pane_id: PaneId,
) -> usize {
    let before = sessions.len();
    sessions.retain(|_, session| {
        session
            .pane_id
            .as_deref()
            .is_none_or(|session_pane_id| !pane_id_matches(session_pane_id, pane_id))
    });
    before - sessions.len()
}

fn pane_id_matches(session_pane_id: &str, pane_id: PaneId) -> bool {
    match pane_id {
        PaneId::Terminal(id) => {
            session_pane_id == id.to_string() || session_pane_id == format!("terminal_{id}")
        }
        PaneId::Plugin(id) => session_pane_id == format!("plugin_{id}"),
    }
}

fn push_event(events: &mut VecDeque<String>, event: String) {
    const MAX_EVENTS: usize = 6;
    if events.len() == MAX_EVENTS {
        events.pop_front();
    }
    events.push_back(event);
}

fn state_label(state: &AgentState) -> &'static str {
    match state {
        AgentState::Idle => "idle   ",
        AgentState::Running => "running",
        AgentState::Shutdown => "closed ",
    }
}

fn basename(path: &str) -> &str {
    path.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(path)
}

fn print_line(row: usize, cols: usize, line: &str) {
    print_text_with_coordinates(Text::new(truncate(line, cols)), 0, row, Some(cols), Some(1));
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(session: &str, pane_id: Option<&str>) -> AgentSession {
        AgentSession {
            version: 1,
            session: session.into(),
            cwd: "/tmp".into(),
            pane_id: pane_id.map(str::to_string),
            state: AgentState::Idle,
            model: None,
            updated_at: 0,
        }
    }

    #[test]
    fn removes_only_sessions_in_closed_terminal_pane() {
        let mut sessions = BTreeMap::from([
            ("a".into(), session("a", Some("1"))),
            ("b".into(), session("b", Some("terminal_1"))),
            ("c".into(), session("c", Some("2"))),
            ("d".into(), session("d", None)),
        ]);

        assert_eq!(remove_sessions_for_pane(&mut sessions, PaneId::Terminal(1)), 2);
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains_key("c"));
        assert!(sessions.contains_key("d"));
    }
}

