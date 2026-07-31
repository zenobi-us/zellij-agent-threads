# Zellij Agent Threads

<img width="285" height="309" alt="image" src="https://github.com/user-attachments/assets/1cfe2b1d-23b1-491a-8fbe-ce973aefe676" />


> ![WARNING]
> This project is in early development. It is not yet stable and may change

An LLM agent dashboard for Zellij.

Provides a Zellij pane for LLM agents that phone home through [Zellij plugin pipes](https://zellij.dev/documentation/plugin-pipes.html).

It shows agents across tabs and panes, their state, current tool, and worktree so you can find
running work without tab hunting.

Display format is configurable with MiniJinja templates. See [Templates](#templates) below.


## Install

Requires [Zellij](https://zellij.dev/),
[Pi](https://github.com/badlogic/pi-mono),
[proto](https://moonrepo.dev/proto), and [Bun](https://bun.sh/).

Clone repository, install toolchains and dependencies, then build and install
both integrations:

```sh
git clone https://github.com/zenobi-us/zellij-agent-threads.git
cd zellij-agent-threads
proto install
bun install
moon run zellij-plugin:install
```

This builds plugin for `wasm32-wasip1`, copies it to
`~/.config/zellij/plugins/agent-threads.wasm`, and links Pi extension at
`~/.pi/agent/extensions/zellij-agent`.

Register plugin alias in `~/.config/zellij/config.kdl`. Pi extension uses this alias to direct
session reports to one plugin instead of broadcasting them:

```kdl
plugins {
    agent-threads location="file:~/.config/zellij/plugins/agent-threads.wasm"
}
```

Add alias to Zellij layout:

```kdl
layout {
    pane {
        plugin location="agent-threads"
    }
}
```

Start Zellij using layout, then start Pi in any pane. Agent reports appear in plugin panel
automatically.

For development, rebuild and reload whenever Rust source changes:

```sh
moon run zellij-plugin:dev-watch
```

## Usage

Default panel groups current Zellij session agents by tab. It shows running or idle state,
pane, model, title, worktree, current tool, and recent plugin events. Click a
tab or pane entry to switch to it. Tabs with no agents are hidden. Silent agents
disappear after ten seconds without a heartbeat report.

## Templates

Plugin accepts an inline [MiniJinja](https://docs.rs/minijinja/latest/minijinja/)
template in layout configuration. This small panel displays the Zellij session name and
agent count:

```kdl
plugin location="agent-threads" {
    template "{{ zellij_session }}: {{ agents | length }} agents"
}
```

For multi-file templates, set `template_file` to the entry file. Disk templates
require Zellij `FullHdAccess`. Includes/imports load lazily and remain cached
until plugin reload. External templates are trusted and can read files exposed
to the plugin through `/host`.

```kdl
plugin location="agent-threads" {
    template_file "/home/you/.config/zellij-agent-threads/templates/main.jinja"
}
```

`main.jinja` can include sibling templates:

```jinja
{{ zellij_session }}
{% for tab in tabs %}
{% if tab.agents | length > 0 %}
{{ tab.tab_name }} [{{ tab.agents | length }}]
{% for agent in tab.agents %}
- {{ agent.harness }}: {{ agent.state }} — {{ agent.cwd }}
{% endfor %}
{% endif %}
{% endfor %}
```

Template model exposes `zellij_session`, `agents`, `sessions`, `tabs`, `events`,
`has_error`, and `last_error`. Each agent exposes `agent_id`, `session_name`,
`state`, `pane`, `cwd`, `model`, `title`, `harness`, `current_tool`, and `focused`.
`tabs` contains all current Zellij session tabs. `tab.agents` contains matching agents only.

Each `sessions[]` item comes from native Zellij session data. It exposes `generation_id`,
`name`, `status`, `agent_count`, `running_agent_count`, `connected_client_count`,
`tab_count`, `pane_count`, `created_at_seconds`, and `current`.

Remote Agent counts use leased in-memory Session summaries. Each sidebar polls other active
Zellij sessions every ten seconds with `zellij --session <name> pipe --name agenthreads:summary`.
The poll does not specify a plugin destination, so it cannot launch a missing sidebar.
Valid replies renew a thirty-second lease. Missing, expired, or unavailable summaries show zero
remote Agent counts while polling continues. The plugin does not store summaries on disk and does
not copy full remote Agent records.

The old `groups`, `group.sessions`, and `current_task` template names are removed.

Templates use `zellij-template-render` components and typed actions:

```jinja
{% call Flex(direction="column", grow=1) %}
{% for agent in agents %}
{% call Button(on_click=actions.focus_pane(agent.pane), focused=agent.focused) %}
{{ " %s " | format(agent.title) | fg("index:6") }}
{% endcall %}
{% endfor %}
{% endcall %}
```

Available actions are `actions.switch_to_session(name)`, `actions.switch_tab(index)`, and
`actions.focus_pane(pane)`. Use `switch_to_session` only for non-current active sessions.
Colors use `index:N` or `rgb:R,G,B`. `format` performs normal MiniJinja string
formatting. `format_time` formats Unix timestamps.

## Protocol v2

Pi publishes version-two Agent Reports only. The payload uses `agent_id` as the stable agent key
and `session_name` as diagnostic metadata. If a report has a `pane_id`, the plugin uses that pane
as the stable row identity. Each accepted report renews a ten-second lease for that agent.
Reports with any other `version` are rejected and recorded as pipe errors.

Version one is removed. The old `session` field and `current_task` field are not accepted.
External publishers must send `current_tool` instead.

Legacy `template_dir`/`template_name` configuration and the local `Grid`,
`Stack`, `PaneButton`, `TabButton`, `remap`, and `italic` helpers were removed.

## More information

[Documentation website](https://zenobi-us.github.io/zellij-agent-threads/)
