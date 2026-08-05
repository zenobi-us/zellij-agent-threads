# Templates (/templates)



# Templates [#templates]

The Zellij plugin renders its panel with MiniJinja templates.

Use `template` for a small inline template. Use `template_file` for a file that can include other template files.

## Inline template [#inline-template]

Add `template` to the plugin pane in your Zellij layout:

```kdl
plugin location="agent-threads" {
    template "{{ zellij_session }}: {{ agents | length }} agents"
}
```

Use escaped newlines for a multi-line inline template. Zellij layouts use KDL v1, so triple-quoted strings do not work.

```kdl
plugin location="agent-threads" {
    template "{{ zellij_session }}\n{% for tab in tabs %}{% if tab.agents | length > 0 %}{{ tab.tab_name }} [{{ tab.agents | length }}]\n{% for agent in tab.agents %}- {{ agent.title }} {{ agent.state }}\n  {{ agent.cwd }}\n{% endfor %}{% endif %}{% endfor %}"
}
```

## Template file [#template-file]

Add `template_file` when the template must live on disk:

```kdl
plugin location="agent-threads" {
    template_file "/home/you/.config/zellij-agent-threads/templates/main.jinja"
}
```

`template` and `template_file` are mutually exclusive.

`template_file` accepts absolute paths, `~/...` paths, and relative paths. Relative paths use `${ZELLIJ_CONFIG_DIR:-$HOME/.config/zellij}`.

External templates require Zellij `FullHdAccess` permission.

## Include another file [#include-another-file]

`main.jinja` can include files from the same template directory:

```jinja
{{ zellij_session }}
{% include "agents.jinja" %}
```

The plugin mounts the template directory only. It does not expose the full host filesystem.

## Model fields [#model-fields]

The template model exposes these top-level values:

| Field            | Meaning                                   |
| ---------------- | ----------------------------------------- |
| `zellij_session` | Current Zellij session name.              |
| `agents`         | Flat list of known Agents.                |
| `sessions`       | Native Zellij sessions with Agent counts. |
| `tabs`           | Tabs in the current Zellij session.       |
| `events`         | Recent plugin events.                     |
| `has_error`      | `true` when the plugin has an error.      |
| `last_error`     | Last plugin error text.                   |

Each `agent` exposes these fields:

| Field            | Meaning                                          |
| ---------------- | ------------------------------------------------ |
| `id`             | Template identity for the Agent row.             |
| `agent_id`       | Stable Agent ID from the report.                 |
| `pane_id`        | Stable pane identity when Zellij reports one.    |
| `pane`           | Pane value for `actions.focus_pane(agent.pane)`. |
| `session_name`   | Diagnostic session name from the report.         |
| `state`          | Agent state, for example `running` or `idle`.    |
| `cwd`            | Current working directory.                       |
| `model`          | Model name when the harness reports it.          |
| `title`          | Agent title.                                     |
| `zellij_session` | Zellij session for this Agent.                   |
| `harness`        | Harness name, for example `pi`.                  |
| `current_tool`   | Current tool name when the harness reports it.   |
| `focused`        | `true` when the pane is focused.                 |
| `active_tab`     | `true` when the Agent is in the active tab.      |

Each `tab` exposes these fields:

| Field      | Meaning                                                      |
| ---------- | ------------------------------------------------------------ |
| `id`       | Template identity for the tab.                               |
| `tab_id`   | Native tab ID. Use it with `actions.switch_tab(tab.tab_id)`. |
| `tab_name` | Native tab name.                                             |
| `active`   | `true` when this is the active tab.                          |
| `agents`   | Agents that belong to this tab.                              |

Each `session` exposes these fields:

| Field                    | Meaning                                                                |
| ------------------------ | ---------------------------------------------------------------------- |
| `id`                     | Runtime-owned stable identity for one native session generation.       |
| `generation_id`          | Same value as `id`, kept for template compatibility.                   |
| `name`                   | Native Zellij session name.                                            |
| `status`                 | `current` for the current session. Other active sessions use `active`. |
| `agent_count`            | Number of known Agents for this session.                               |
| `running_agent_count`    | Number of running Agents for this session.                             |
| `connected_client_count` | Number of connected Zellij clients. Web clients are included.          |
| `tab_count`              | Number of native tabs in the session.                                  |
| `pane_count`             | Number of native panes in the session.                                 |
| `created_at_seconds`     | Native Zellij session-age value, in seconds.                           |
| `current`                | `true` when this is the current Zellij session.                        |

## Actions [#actions]

Interactive template entries use typed actions:

```jinja
{% call Button(on_click=actions.focus_pane(agent.pane), focused=agent.focused) %}
{{ agent.title }}
{% endcall %}
```

Available actions are:

| Action                                    | Use                                      |
| ----------------------------------------- | ---------------------------------------- |
| `actions.focus_pane(agent.pane)`          | Focus an Agent pane.                     |
| `actions.switch_tab(tab.tab_id)`          | Switch to a tab.                         |
| `actions.switch_to_session(session.name)` | Switch to another active Zellij session. |

Do not attach `actions.switch_to_session(session.name)` to the current session.

## Colors and formatting [#colors-and-formatting]

Templates use `zellij-template-render` components and filters.

Colors use `index:N` or `rgb:R,G,B` values:

```jinja
{{ agent.title | fg("index:6") }}
{{ agent.state | bg("rgb:40,44,52") }}
{{ agent.cwd | fg(theme.active_text) | bg(theme.active_background) }}
```

Use the [zellij-template-render colour contract](https://github.com/zenobi-us/zellij-plugins/tree/main/pkgs/zellij-template-render#colour-contract) for the colour and renderer reference.

MiniJinja `format` works as usual:

```jinja
{{ " %s " | format(agent.title) }}
```

Use `format_time` for Unix timestamps.

## Animation [#animation]

`AnimationFrame(fps=...)` returns a wall-clock frame number and schedules another render.

```jinja
{% set frames = ["⠋", "⠙", "⠹", "⠸"] %}
{% if agent.state == "running" %}
  {{ frames[AnimationFrame(fps=8) % (frames | length)] }}
{% endif %}
```

Use a value from 1 to 20 for `fps`.

Every animation frame reruns the full template. Keep animated templates small.
