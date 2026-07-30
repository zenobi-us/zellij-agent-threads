use std::collections::BTreeMap;

use zellij_template_render::{
    error_frame as shared_error_frame, ActionRegistry, ButtonPresentation, ButtonView, Environment,
    Error, ErrorKind, Frame, Renderer, TemplateContext, TemplateHost, Value, Viewport,
};
use zellij_tile::prelude::ModeInfo;

use super::model::RenderModel;

const DEFAULT_TEMPLATE_NAME: &str = "main.jinja";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClickAction {
    SwitchTab { tab: u32 },
    SwitchToSession { session: String },
    FocusPane { pane: String },
}

pub(crate) type RenderedFrame = Frame<ClickAction>;
pub(crate) type TemplateError = Error;

pub(crate) struct AgentRenderer {
    host: TemplateHost<ClickAction>,
}

impl AgentRenderer {
    pub(crate) fn from_configuration(
        configuration: &BTreeMap<String, String>,
    ) -> Result<Self, Error> {
        reject_legacy_configuration(configuration)?;

        let mut embedded = Environment::new();
        minijinja_embed::load_templates!(&mut embedded);
        Ok(Self {
            host: TemplateHost::from_configuration(
                Renderer::new(
                    ActionRegistry::new()
                        .with("switch_tab", decode_switch_tab)
                        .with("switch_to_session", decode_switch_to_session)
                        .with("focus_pane", decode_focus_pane),
                ),
                configuration,
                embedded,
                DEFAULT_TEMPLATE_NAME,
            )?,
        })
    }

    pub(crate) fn render(
        &mut self,
        mode_info: &ModeInfo,
        model: &RenderModel,
        rows: usize,
        cols: usize,
    ) -> Result<RenderedFrame, Error> {
        let active_tab = model.active_tab();
        let active_session = model.zellij_session.clone();
        let focused_pane = model.focused_pane().map(str::to_owned);

        self.host.render(
            template_context(model, rows),
            mode_info,
            Viewport { rows, cols },
            move |button| {
                present_button(button, active_tab, &active_session, focused_pane.as_deref())
            },
        )
    }

    pub(crate) fn error_frame(&self, error: &Error, rows: usize, cols: usize) -> RenderedFrame {
        let mut frame = shared_error_frame(error, Viewport { rows, cols });
        frame.refresh_after = self.host.refresh_after();
        frame
    }
}

pub(crate) fn error_frame(error: &Error, rows: usize, cols: usize) -> RenderedFrame {
    shared_error_frame(error, Viewport { rows, cols })
}

fn template_context(model: &RenderModel, rows: usize) -> TemplateContext {
    TemplateContext::new()
        .with("empty_message", model.empty_message.clone())
        .with("agents", Value::from_serialize(&model.agents))
        .with("sessions", Value::from_serialize(&model.sessions))
        .with("zellij_session", model.zellij_session.clone())
        .with("harness", model.harness.clone())
        .with("tabs", Value::from_serialize(&model.tabs))
        .with("events", Value::from_serialize(&model.events))
        .with("has_error", model.has_error)
        .with("last_error", model.last_error.clone())
        .with("padding_rows", " \n ")
        .with("layout_fill", model.layout_fill(rows))
}

fn present_button(
    button: ButtonView<'_, ClickAction>,
    active_tab: Option<u32>,
    active_session: &str,
    focused_pane: Option<&str>,
) -> Result<ButtonPresentation, Error> {
    let focused = button.focused.unwrap_or_else(|| match button.action {
        ClickAction::SwitchTab { tab } => active_tab == Some(*tab),
        ClickAction::SwitchToSession { session } => active_session == session,
        ClickAction::FocusPane { pane } => focused_pane == Some(pane.as_str()),
    });
    Ok(ButtonPresentation {
        label: button.label.to_owned(),
        focused,
    })
}

fn decode_switch_tab(args: &[Value]) -> Result<ClickAction, Error> {
    let tab = one_argument(args, "switch_tab")?
        .as_usize()
        .and_then(|tab| u32::try_from(tab).ok())
        .ok_or_else(|| invalid_action("switch_tab expects one unsigned 32-bit integer"))?;
    Ok(ClickAction::SwitchTab { tab })
}

fn decode_switch_to_session(args: &[Value]) -> Result<ClickAction, Error> {
    let session = one_argument(args, "switch_to_session")?
        .as_str()
        .filter(|session| !session.is_empty())
        .ok_or_else(|| invalid_action("switch_to_session expects one non-empty session name"))?;
    Ok(ClickAction::SwitchToSession {
        session: session.to_owned(),
    })
}

fn decode_focus_pane(args: &[Value]) -> Result<ClickAction, Error> {
    let pane = one_argument(args, "focus_pane")?
        .as_str()
        .filter(|pane| !pane.is_empty())
        .ok_or_else(|| invalid_action("focus_pane expects one non-empty pane ID"))?;
    Ok(ClickAction::FocusPane {
        pane: pane.to_owned(),
    })
}

fn one_argument<'a>(args: &'a [Value], name: &str) -> Result<&'a Value, Error> {
    if args.len() != 1 {
        return Err(invalid_action(format!(
            "{name} expects exactly one argument"
        )));
    }
    Ok(&args[0])
}

fn invalid_action(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidOperation, message.into())
}

fn reject_legacy_configuration(configuration: &BTreeMap<String, String>) -> Result<(), Error> {
    if configuration.contains_key("template_dir") || configuration.contains_key("template_name") {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "template_dir/template_name were removed; use template_file",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::RenderConfig;
    use crate::runtime::{AgentReport, AgentState, RuntimeState, ZellijSession};

    use super::*;

    const SPINNER_FRAMES: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";

    #[test]
    fn default_template_renders_typed_actions() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::new()).unwrap();
        let frame = renderer
            .render(&ModeInfo::default(), &sample_model(), 20, 80)
            .unwrap();
        let output = frame
            .lines
            .iter()
            .map(|line| plain_text(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            output.contains("Agents"),
            "rendered lines: {:?}",
            frame.lines
        );
        assert!(
            output.contains("First Message Title"),
            "rendered lines: {:?}",
            frame.lines
        );
        assert!(output.contains("bash"), "rendered lines: {:?}", frame.lines);
        assert!(
            SPINNER_FRAMES.chars().any(|icon| output.contains(icon)),
            "rendered lines: {:?}",
            frame.lines
        );
        assert!(frame.refresh_after.is_some_and(|delay| {
            !delay.is_zero() && delay <= std::time::Duration::from_millis(125)
        }));
        assert!(
            output.contains("Events"),
            "rendered lines: {:?}",
            frame.lines
        );
        assert!(frame
            .hitboxes
            .iter()
            .flatten()
            .any(|action| { action == &Some(ClickAction::SwitchTab { tab: 8 }) }));
        assert!(frame
            .hitboxes
            .iter()
            .flatten()
            .any(|action| { action == &Some(ClickAction::FocusPane { pane: "1".into() }) }));
        let pane_row = frame
            .hitboxes
            .iter()
            .position(|line| {
                line.iter().any(|action| {
                    matches!(
                        action,
                        Some(ClickAction::FocusPane { pane }) if pane == "1"
                    )
                })
            })
            .unwrap();
        assert!(pane_row > 0);
    }

    #[test]
    fn idle_agents_do_not_request_animation_refresh() {
        let mut idle = agent_session("s", "1", "Idle");
        idle.state = AgentState::Idle;
        idle.current_tool = None;
        let runtime = RuntimeState {
            agents: BTreeMap::from([("s".into(), idle)]),
            focused_pane: Some("1".into()),
            active_tab: Some(7),
            tabs: BTreeMap::from([(7, "Agents".into())]),
            zellij_session: Some("z".into()),
            ..RuntimeState::default()
        };
        let model = RenderModel::from_runtime(&runtime, &RenderConfig::default());
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::new()).unwrap();
        let frame = renderer
            .render(&ModeInfo::default(), &model, 20, 80)
            .unwrap();
        let output = frame
            .lines
            .iter()
            .map(|line| plain_text(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(frame.refresh_after, None);
        assert!(!SPINNER_FRAMES.chars().any(|icon| output.contains(icon)));
    }

    #[test]
    fn inline_template_keeps_top_level_data_and_builtin_format() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::from([(
            "template".into(),
            "{{ \" %s \" | format(zellij_session) }} {{ 1700000000 | format_time(\"%s\") }}".into(),
        )]))
        .unwrap();

        let frame = renderer
            .render(&ModeInfo::default(), &sample_model(), 1, 30)
            .unwrap();
        assert_eq!(frame.lines, [" z  1700000000"]);
    }

    #[test]
    fn inline_template_uses_agent_and_tab_contract() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::from([(
            "template".into(),
            "{{ agents | length }}/{{ tabs | length }}/{{ tabs[0].agents | length }}".into(),
        )]))
        .unwrap();

        let frame = renderer
            .render(&ModeInfo::default(), &sample_model(), 1, 30)
            .unwrap();
        assert_eq!(frame.lines, ["1/1/1"]);
    }

    #[test]
    fn inline_template_uses_sessions_contract() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::from([(
            "template".into(),
            "{{ sessions[0].generation_id }} {{ sessions[0].name }} {{ sessions[0].status }} {{ sessions[0].agent_count }} {{ sessions[0].running_agent_count }} {{ sessions[0].connected_client_count }} {{ sessions[0].tab_count }} {{ sessions[0].pane_count }} {{ sessions[0].created_at_seconds }}".into(),
        )]))
        .unwrap();

        let frame = renderer
            .render(&ModeInfo::default(), &sample_model_with_sessions(), 1, 80)
            .unwrap();
        assert_eq!(frame.lines, ["z:10 z current 1 1 1 2 3 10"]);
    }

    #[test]
    fn inline_template_builds_switch_session_hitboxes() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::from([(
            "template".into(),
            "{% call Button(on_click=actions.switch_to_session(\"other\")) %}go{% endcall %}"
                .into(),
        )]))
        .unwrap();

        let frame = renderer
            .render(&ModeInfo::default(), &sample_model_with_sessions(), 1, 2)
            .unwrap();
        assert_eq!(frame.lines, ["go"]);
        assert_eq!(
            frame.hitboxes[0],
            [
                Some(ClickAction::SwitchToSession {
                    session: "other".into()
                }),
                Some(ClickAction::SwitchToSession {
                    session: "other".into()
                }),
            ]
        );
    }

    #[test]
    fn default_template_renders_session_list_with_current_session_not_clickable() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::new()).unwrap();
        let frame = renderer
            .render(&ModeInfo::default(), &sample_model_with_sessions(), 24, 100)
            .unwrap();
        let output = frame
            .lines
            .iter()
            .map(|line| plain_text(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(output.find("Sessions") < output.find("Agents"));
        assert!(output.contains("z current 1a 1r 1c 2t 3p"));
        assert!(output.contains("other active 0c 0t 0p"));
        assert!(!output.contains("other active 0a"));
        assert!(frame.hitboxes.iter().flatten().any(|action| {
            matches!(
                action,
                Some(ClickAction::SwitchToSession { session }) if session == "other"
            )
        }));
        assert!(!frame.hitboxes.iter().flatten().any(|action| {
            matches!(
                action,
                Some(ClickAction::SwitchToSession { session }) if session == "z"
            )
        }));
    }

    #[test]
    fn inline_template_builds_focus_pane_hitboxes() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::from([(
            "template".into(),
            "{% call Button(on_click=actions.focus_pane(\"9\")) %}go{% endcall %}".into(),
        )]))
        .unwrap();

        let frame = renderer
            .render(&ModeInfo::default(), &sample_model(), 1, 2)
            .unwrap();
        assert_eq!(frame.lines, ["go"]);
        assert_eq!(
            frame.hitboxes[0],
            [
                Some(ClickAction::FocusPane { pane: "9".into() }),
                Some(ClickAction::FocusPane { pane: "9".into() }),
            ]
        );
    }

    #[test]
    fn default_template_omits_tab_action_without_tab_id() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::new()).unwrap();
        let frame = renderer
            .render(&ModeInfo::default(), &sample_model_with_tab(None), 20, 80)
            .unwrap();

        assert!(!frame
            .hitboxes
            .iter()
            .flatten()
            .any(|action| { matches!(action, Some(ClickAction::SwitchTab { .. })) }));
    }

    #[test]
    fn external_template_reloads_changed_includes() {
        let dir = std::env::temp_dir().join(format!(
            "zellij-agent-threads-template-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.jinja"), "{% include 'part.jinja' %}").unwrap();
        fs::write(dir.join("part.jinja"), "{{ zellij_session | upper }}").unwrap();

        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::from([(
            "template_file".into(),
            dir.join("main.jinja").display().to_string(),
        )]))
        .unwrap();
        let frame = renderer
            .render(&ModeInfo::default(), &sample_model(), 1, 20)
            .unwrap();
        assert_eq!(frame.lines, ["Z"]);
        assert!(frame.refresh_after.is_some());

        fs::write(dir.join("part.jinja"), "reloaded").unwrap();
        let frame = renderer
            .render(&ModeInfo::default(), &sample_model(), 1, 20)
            .unwrap();
        assert_eq!(frame.lines, ["reloaded"]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_template_loader_configuration_is_rejected() {
        let error = AgentRenderer::from_configuration(&BTreeMap::from([(
            "template_dir".into(),
            "/tmp/templates".into(),
        )]))
        .err()
        .unwrap();

        assert!(error.to_string().contains("use template_file"));
    }

    #[test]
    fn inline_and_external_templates_are_mutually_exclusive() {
        let error = AgentRenderer::from_configuration(&BTreeMap::from([
            ("template".into(), "inline".into()),
            ("template_file".into(), "/tmp/main.jinja".into()),
        ]))
        .err()
        .unwrap();

        assert!(error
            .to_string()
            .contains("template and template_file cannot be configured together"));
    }

    #[test]
    fn malformed_action_arguments_are_template_errors() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::from([(
            "template".into(),
            "{% call Button(on_click=actions.switch_tab()) %}bad{% endcall %}".into(),
        )]))
        .unwrap();

        let error = renderer
            .render(&ModeInfo::default(), &sample_model(), 1, 10)
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("switch_tab expects exactly one argument"));
    }

    #[test]
    fn default_template_handles_tiny_viewports() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::new()).unwrap();
        let frame = renderer
            .render(&ModeInfo::default(), &sample_model(), 2, 8)
            .unwrap();

        assert_eq!(frame.lines.len(), 2);
        assert_eq!(frame.hitboxes.len(), 2);
        assert!(frame.hitboxes.iter().all(|line| line.len() == 8));
    }

    #[test]
    fn overflow_follows_the_focused_session() {
        let runtime = RuntimeState {
            agents: BTreeMap::from([
                ("a".into(), agent_session("a", "1", "First")),
                ("b".into(), agent_session("b", "2", "Second")),
                ("c".into(), agent_session("c", "3", "Third")),
            ]),
            focused_pane: Some("3".into()),
            active_tab: Some(7),
            tabs: BTreeMap::from([(7, "Agents".into())]),
            zellij_session: Some("z".into()),
            ..RuntimeState::default()
        };
        let model = RenderModel::from_runtime(&runtime, &RenderConfig::default());
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::new()).unwrap();
        let frame = renderer
            .render(&ModeInfo::default(), &model, 10, 80)
            .unwrap();
        let output = frame
            .lines
            .iter()
            .map(|line| plain_text(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(output.contains("Third"), "rendered output: {output:?}");
        assert!(frame.hitboxes.iter().flatten().any(|action| {
            matches!(
                action,
                Some(ClickAction::FocusPane { pane }) if pane == "3"
            )
        }));
    }

    fn plain_text(value: &str) -> String {
        let mut output = String::new();
        let mut chars = value.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch != '\u{1b}' {
                output.push(ch);
                continue;
            }
            match chars.next() {
                Some('[') => {
                    for ch in chars.by_ref() {
                        if ('@'..='~').contains(&ch) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(ch) = chars.next() {
                        if ch == '\u{7}' {
                            break;
                        }
                        if ch == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        output
    }

    fn sample_model() -> RenderModel {
        sample_model_with_tab(Some(7))
    }

    fn sample_model_with_sessions() -> RenderModel {
        let runtime = RuntimeState {
            agents: BTreeMap::from([(
                "s".into(),
                AgentReport {
                    tab_id: Some(7),
                    title: Some("First Message Title".into()),
                    ..agent_session("s", "1", "First Message Title")
                },
            )]),
            focused_pane: Some("1".into()),
            active_tab: Some(7),
            tabs: BTreeMap::from([(7, "Agents".into())]),
            zellij_session: Some("z".into()),
            zellij_sessions: BTreeMap::from([
                (
                    "z:10".into(),
                    ZellijSession {
                        generation_id: "z:10".into(),
                        name: "z".into(),
                        connected_client_count: 1,
                        tab_count: 2,
                        pane_count: 3,
                        created_at_seconds: 10,
                        current: true,
                    },
                ),
                (
                    "other:20".into(),
                    ZellijSession {
                        generation_id: "other:20".into(),
                        name: "other".into(),
                        connected_client_count: 0,
                        tab_count: 0,
                        pane_count: 0,
                        created_at_seconds: 20,
                        current: false,
                    },
                ),
            ]),
            ..RuntimeState::default()
        };
        RenderModel::from_runtime(&runtime, &RenderConfig::default())
    }

    fn sample_model_with_tab(tab_id: Option<usize>) -> RenderModel {
        let runtime = RuntimeState {
            agents: BTreeMap::from([(
                "s".into(),
                AgentReport {
                    tab_id,
                    title: Some("First Message Title".into()),
                    ..agent_session("s", "1", "First Message Title")
                },
            )]),
            focused_pane: Some("1".into()),
            active_tab: tab_id,
            tabs: tab_id
                .map(|id| BTreeMap::from([(id, "Agents".into())]))
                .unwrap_or_default(),
            zellij_session: Some("z".into()),
            ..RuntimeState::default()
        };
        RenderModel::from_runtime(&runtime, &RenderConfig::default())
    }

    fn agent_session(session: &str, pane: &str, title: &str) -> AgentReport {
        AgentReport {
            version: 2,
            harness: Some("pi".into()),
            agent_id: session.into(),
            session_name: Some(format!("{session}.jsonl")),
            cwd: "/tmp/project".into(),
            pane_id: Some(pane.into()),
            tab_id: Some(7),
            tab_name: Some("Agents".into()),
            zellij_session: Some("z".into()),
            state: AgentState::Running,
            model: Some("m".into()),
            title: Some(title.into()),
            current_tool: Some("bash".into()),
            updated_at: 0,
        }
    }
}
