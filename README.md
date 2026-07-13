# Zellij Agent Threads

> ![WARNING]
> This project is in early development. It is not yet stable and may change

An LLM agent dashboard for Zellij.

Provides a Zellij pane for llm harness sessions that phone home via [Zellij call plugin pipes](https://zellij.dev/documentation/plugin-pipes.html).

It shows agents across tabs and panes, their state, current task, and worktree so you can find
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
`~/.config/zellij/plugins/zellij-agent-threads.wasm`, and links Pi extension at
`~/.pi/agent/extensions/zellij-agent`.

Add plugin to Zellij layout:

```kdl
layout {
    pane {
        plugin location="file:/home/you/.config/zellij/plugins/zellij-agent-threads.wasm"
    }
}
```

Replace `/home/you` with your home directory. Start Zellij using layout, then
start Pi in any pane. Agent reports appear in plugin panel automatically.

For development, rebuild and reload whenever Rust source changes:

```sh
moon run zellij-plugin:dev-watch
```

## Usage

Default panel groups Pi agents by Zellij tab. It shows running or idle state,
pane, model, title, worktree, current task, and recent plugin events. Click a
tab or pane entry to switch to it.

## Templates

Plugin accepts an inline [MiniJinja](https://docs.rs/minijinja/latest/minijinja/)
template in layout configuration. This small panel displays session name and
agent count:

```kdl
plugin location="file:/home/you/.config/zellij/plugins/zellij-agent-threads.wasm" {
    template "{{ zellij_session }}: {{ sessions | length }} agents"
}
```

For multi-file templates, set `template_dir` and `template_name`. `main.j2` is
the default name. Disk templates require Zellij `FullHdAccess` permission.

```kdl
plugin location="file:/home/you/.config/zellij/plugins/zellij-agent-threads.wasm" {
    template_dir "/home/you/.config/zellij-agent-threads/templates"
    template_name "main.j2"
}
```

`main.j2` can include sibling templates:

```jinja
{{ zellij_session }}
{% for group in groups %}
{{ group.tab_name }} [{{ group.sessions | length }}]
{% for session in group.sessions %}
- {{ session.harness }}: {{ session.state }} — {{ session.cwd }}
{% endfor %}
{% endfor %}
```

Template model exposes `zellij_session`, `sessions`, `groups`, `events`,
`has_error`, and `last_error`. Each session exposes `state`, `pane`, `cwd`,
`model`, `title`, `harness`, `current_task`, and `focused`.

## More information

[Documentation website](https://zenobi-us.github.io/zellij-agent-threads/)
