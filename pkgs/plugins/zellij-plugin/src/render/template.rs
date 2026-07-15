use std::collections::BTreeMap;

use zellij_template_render::{
    error_frame as shared_error_frame, ActionRegistry, ButtonPresentation, ButtonView, Environment,
    Error, ErrorKind, Frame, Renderer, TemplateContext, TemplateEnvironment, TemplateHost,
    TemplateSource, TemplateTheme, Value, Viewport,
};

use super::model::RenderModel;

const DEFAULT_TEMPLATE_NAME: &str = "main.jinja";

pub(crate) const DEFAULT_TEMPLATE: &str = r#"{%- call Flex(direction="column", grow=1) -%}
{%- call Flex(shrink=0) -%}
{{- " %s " | format(zellij_session) | bg("index:3") | fg("index:0") -}}
{%- endcall -%}
{%- call Flex(basis=2, shrink=0) -%}{{- padding_rows -}}{%- endcall -%}
{%- call Flex(direction="column", grow=1, shrink=1, overflow="scroll") -%}
{%- if sessions | length > 0 -%}
{%- call Flex(shrink=0) -%}
{{- " Agents " | bold | bg("index:6") | fg("index:0") -}}
{%- endcall -%}
{%- macro indicator(focused) -%}{{ " " | bg("index:4") | fg("index:0") if focused else " " }}{%- endmacro -%}
{%- macro t(text, focused) -%}{{ text | bold if focused else text | dim }}{%- endmacro -%}
{%- for group in groups -%}
{%- set tab_label = " %s " | format(group.tab_name) -%}
{%- if group.tab_id is not none -%}
{%- call Button(on_click=actions.switch_tab(group.tab_id), focused=false) -%}
{{- tab_label | bg("index:6") | fg("index:0") if group.active else tab_label | bg("index:8") | dim -}}
{%- endcall -%}
{%- else -%}
{%- call Flex(shrink=0) -%}
{{- tab_label | bg("index:6") | fg("index:0") if group.active else tab_label | bg("index:8") | dim -}}
{%- endcall -%}
{%- endif -%}
{%- for session in group.sessions -%}
{%- call Button(on_click=actions.focus_pane(session.pane), focused=session.focused) -%}
{%- set icon = "󱉺" if session.state == "running" else "󰏧" -%}
{%- set agent = "%s %s %s@%s" | format(session.pane, icon, session.harness, session.model) -%}
{{ indicator(session.focused) }} {{ t(agent | bold, session.focused) }}
{{ indicator(session.focused) }}     󰆈 {{ session.title | bold if session.focused else session.title | dim }}
{{ indicator(session.focused) }}      {{ session.cwd | bold if session.focused else session.cwd | dim }}
{%- if session.state == "running" %}
{{ indicator(session.focused) }}  {{ session.current_task | bold if session.focused else session.current_task | dim }}
{%- endif -%}
{%- endcall -%}
{%- endfor -%}
{%- endfor -%}
{%- endif -%}
{{- layout_fill -}}
{%- endcall -%}
{%- if sessions | length > 0 -%}
{%- call Flex(direction="column", shrink=0) -%}
{%- if has_error -%}
{{ " pipe error " | bg("index:1") | fg("index:7") }} {{ last_error | dim }}
{%- endif -%}
{{ " Events " | bg("index:3") | fg("index:0") }}
{%- for event in events %}
{{ " " | bg("index:3") }}  {{ " %s " | format(event) | fg("index:3") }}
{%- endfor -%}
{%- endcall -%}
{%- endif -%}
{%- call Flex(basis=2, shrink=0) -%}{{- padding_rows -}}{%- endcall -%}
{%- endcall -%}"#;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClickAction {
    SwitchTab { tab: u32 },
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
        embedded.add_template(DEFAULT_TEMPLATE_NAME, DEFAULT_TEMPLATE)?;
        let source =
            TemplateSource::from_configuration(configuration, embedded, DEFAULT_TEMPLATE_NAME)?;

        Ok(Self {
            host: TemplateHost::new(
                Renderer::new(
                    ActionRegistry::new()
                        .with("switch_tab", decode_switch_tab)
                        .with("focus_pane", decode_focus_pane),
                ),
                source,
                TemplateEnvironment::from_configuration(configuration),
            ),
        })
    }

    pub(crate) fn render(
        &mut self,
        model: &RenderModel,
        rows: usize,
        cols: usize,
    ) -> Result<RenderedFrame, Error> {
        let active_tab = model.active_tab();
        let focused_pane = model.focused_pane().map(str::to_owned);
        self.host.render(
            template_context(model, rows),
            static_theme(),
            Viewport { rows, cols },
            move |button| present_button(button, active_tab, focused_pane.as_deref()),
        )
    }
}

pub(crate) fn error_frame(error: &Error, rows: usize, cols: usize) -> RenderedFrame {
    shared_error_frame(error, Viewport { rows, cols })
}

fn template_context(model: &RenderModel, rows: usize) -> TemplateContext {
    TemplateContext::new()
        .with("empty_message", model.empty_message.clone())
        .with("sessions", Value::from_serialize(&model.sessions))
        .with("zellij_session", model.zellij_session.clone())
        .with("harness", model.harness.clone())
        .with("groups", Value::from_serialize(&model.groups))
        .with("events", Value::from_serialize(&model.events))
        .with("has_error", model.has_error)
        .with("last_error", model.last_error.clone())
        .with("padding_rows", " \n ")
        .with("layout_fill", model.layout_fill(rows))
}

fn static_theme() -> TemplateTheme {
    TemplateTheme {
        text: "index:7".into(),
        background: "index:0".into(),
        active_text: "index:0".into(),
        active_background: "index:6".into(),
        muted_text: "index:8".into(),
        muted_background: "index:0".into(),
        alert: "index:1".into(),
    }
}

fn present_button(
    button: ButtonView<'_, ClickAction>,
    active_tab: Option<u32>,
    focused_pane: Option<&str>,
) -> Result<ButtonPresentation, Error> {
    let focused = button.focused.unwrap_or_else(|| match button.action {
        ClickAction::SwitchTab { tab } => active_tab == Some(*tab),
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
    use crate::runtime::{AgentSession, AgentState, RuntimeState};

    use super::*;

    #[test]
    fn default_template_renders_typed_actions() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::new()).unwrap();
        let frame = renderer.render(&sample_model(), 20, 80).unwrap();
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
        assert_eq!(plain_text(&frame.lines[17]), " Events ");
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
    fn inline_template_keeps_top_level_data_and_builtin_format() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::from([(
            "template".into(),
            "{{ \" %s \" | format(zellij_session) }} {{ 1700000000 | format_time(\"%s\") }}".into(),
        )]))
        .unwrap();

        let frame = renderer.render(&sample_model(), 1, 30).unwrap();
        assert_eq!(frame.lines, [" z  1700000000"]);
    }

    #[test]
    fn inline_template_builds_focus_pane_hitboxes() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::from([(
            "template".into(),
            "{% call Button(on_click=actions.focus_pane(\"9\")) %}go{% endcall %}".into(),
        )]))
        .unwrap();

        let frame = renderer.render(&sample_model(), 1, 2).unwrap();
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
            .render(&sample_model_with_tab(None), 20, 80)
            .unwrap();

        assert!(!frame
            .hitboxes
            .iter()
            .flatten()
            .any(|action| { matches!(action, Some(ClickAction::SwitchTab { .. })) }));
    }

    #[test]
    fn external_template_supports_includes() {
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
        let frame = renderer.render(&sample_model(), 1, 20).unwrap();

        assert_eq!(frame.lines, ["Z"]);
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

        let error = renderer.render(&sample_model(), 1, 10).unwrap_err();
        assert!(error
            .to_string()
            .contains("switch_tab expects exactly one argument"));
    }

    #[test]
    fn default_template_handles_tiny_viewports() {
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::new()).unwrap();
        let frame = renderer.render(&sample_model(), 2, 8).unwrap();

        assert_eq!(frame.lines.len(), 2);
        assert_eq!(frame.hitboxes.len(), 2);
        assert!(frame.hitboxes.iter().all(|line| line.len() == 8));
    }

    #[test]
    fn overflow_follows_the_focused_session() {
        let runtime = RuntimeState {
            sessions: BTreeMap::from([
                ("a".into(), agent_session("a", "1", "First")),
                ("b".into(), agent_session("b", "2", "Second")),
                ("c".into(), agent_session("c", "3", "Third")),
            ]),
            focused_pane: Some("3".into()),
            active_tab: Some(7),
            zellij_session: Some("z".into()),
            ..RuntimeState::default()
        };
        let model = RenderModel::from_runtime(&runtime, &RenderConfig::default());
        let mut renderer = AgentRenderer::from_configuration(&BTreeMap::new()).unwrap();
        let frame = renderer.render(&model, 10, 80).unwrap();
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

    fn sample_model_with_tab(tab_id: Option<usize>) -> RenderModel {
        let runtime = RuntimeState {
            sessions: BTreeMap::from([(
                "s".into(),
                AgentSession {
                    tab_id,
                    title: Some("First Message Title".into()),
                    ..agent_session("s", "1", "First Message Title")
                },
            )]),
            focused_pane: Some("1".into()),
            active_tab: tab_id,
            zellij_session: Some("z".into()),
            ..RuntimeState::default()
        };
        RenderModel::from_runtime(&runtime, &RenderConfig::default())
    }

    fn agent_session(session: &str, pane: &str, title: &str) -> AgentSession {
        AgentSession {
            version: 1,
            harness: Some("pi".into()),
            session: session.into(),
            cwd: "/tmp/project".into(),
            pane_id: Some(pane.into()),
            tab_id: Some(7),
            tab_name: Some("Agents".into()),
            zellij_session: Some("z".into()),
            state: AgentState::Running,
            model: Some("m".into()),
            title: Some(title.into()),
            current_task: Some("Latest Task".into()),
            updated_at: 0,
        }
    }
}
