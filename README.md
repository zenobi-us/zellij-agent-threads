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

Requires [Zellij](https://zellij.dev/) and [Pi](https://github.com/badlogic/pi-mono).

### Released install

Download the `agent-threads` CLI from the latest GitHub release:

```sh
mkdir -p ~/.local/bin
curl -L https://github.com/zenobi-us/zellij-agent-threads/releases/latest/download/agent-threads-linux-x64 \
  -o ~/.local/bin/agent-threads
chmod +x ~/.local/bin/agent-threads
```

Use the asset that matches your platform:

- `agent-threads-linux-x64`
- `agent-threads-linux-arm64`
- `agent-threads-darwin-x64`
- `agent-threads-darwin-arm64`
- `agent-threads-windows-x64.exe`

Then install the released plugin and Pi extension:

```sh
agent-threads install
```

The released installer downloads assets for the same version as the running CLI.
It installs these files:

- Zellij plugin: `~/.config/zellij/plugins/agent-threads.wasm`
- Pi extension: `~/.pi/agent/extensions/pi-agenthread`

The `--harness pi` flag does not skip the plugin. It selects the Pi extension after the plugin install.

The installer prompts before it edits `~/.config/zellij/config.kdl` in an interactive terminal.
Use `agent-threads install --yes` to edit the file without a prompt.
In a non-interactive terminal, the installer prints the KDL snippet instead.
It backs up an existing config file to `~/.config/zellij/config.kdl.bak` before it writes changes.

The Zellij plugin alias uses this shape:

```kdl
plugins {
    agent-threads location="file:/home/you/.config/zellij/plugins/agent-threads.wasm"
}
```

Add the alias to a Zellij layout:

```kdl
layout {
    pane {
        plugin location="agent-threads"
    }
}
```

Start Zellij with that layout. Then start Pi in any pane.
Agent reports appear in the plugin panel automatically.

### Self-update

Update the CLI from the stable channel:

```sh
agent-threads self-update
```

Use the prerelease channel when you need the next release candidate:

```sh
agent-threads self-update --channel prerelease
```

The `stable` channel selects the latest non-prerelease tag named `agent-threads-v*`.
The `prerelease` channel selects the latest prerelease tag named `agent-threads-v*`.
The aliases `latest` and `next` map to `stable` and `prerelease`.
Self-update replaces only `~/.local/bin/agent-threads`.
Run `agent-threads install` after self-update to align the plugin and Pi extension.

### Contributor source install

Source installs require [proto](https://moonrepo.dev/proto) and [Bun](https://bun.sh/).

Clone the repository, install toolchains and dependencies, then build and install both integrations:

```sh
git clone https://github.com/zenobi-us/zellij-agent-threads.git
cd zellij-agent-threads
proto install
bun install
moon run zellij-plugin:install
```

This builds the plugin for `wasm32-wasip1`, copies it to
`~/.config/zellij/plugins/agent-threads.wasm`, and installs the Pi extension at
`~/.pi/agent/extensions/pi-agenthread`.

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

Each `sessions[]` item comes from native Zellij session data. It exposes `id`, `generation_id`,
`name`, `status`, `agent_count`, `running_agent_count`, `connected_client_count`, `tab_count`,
`pane_count`, `created_at_seconds`, and `current`. The plugin owns `id` because Zellij reports
session age, not a stable native session key.

Agent presence comes from the `agent-threads` CLI singleton store. Pi writes reports with
`agent-threads upsert --json '<AgentReportV2>'` and deletes closed sessions with
`agent-threads delete --agent-id '<id>'`. The CLI stores rows in SQLite at
`$XDG_RUNTIME_DIR/zellij-agent-threads/state.sqlite`. The plugin polls
`agent-threads snapshot --json` and replaces its in-memory agent list with that snapshot.
Zellij pipes are wake/control signals only.
`agenthreads:summary` remote polling is deprecated.

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
and `session_name` as diagnostic metadata. If a report has a `pane_id`, the store key is
`zellij_session:pane_id`.
Otherwise, it is `agent_id`.
Each upsert renews a ten-second SQLite lease.
Reports with any other `version` are rejected by the store or recorded as pipe errors.

Version one is removed. The old `session` field and `current_task` field are not accepted.
External publishers must send `current_tool` instead.

Legacy `template_dir`/`template_name` configuration and the local `Grid`,
`Stack`, `PaneButton`, `TabButton`, `remap`, and `italic` helpers were removed.

## More information

[Documentation website](https://zenobi-us.github.io/zellij-agent-threads/)
