# zellij-plugin-agent-threads

Rust/WASM Zellij plugin that lists Pi agents reported through Zellij pipes.

## Build

```bash
moon run zellij-plugin:build
```

Output:

```text
pkgs/plugins/zellij-plugin/target/wasm32-wasip1/release/zellij-plugin-agent-threads.wasm
```

## Test

```bash
moon run zellij-plugin:test
moon run zellij-plugin:check
```

Host-target tests are used because raw `.wasm` test binaries do not execute directly on Linux without a WASI runner.

## Plugin alias

Register the installed plugin in `~/.config/zellij/config.kdl`. Pi extension targets this alias
with `zellij pipe --plugin agent-threads`.

```kdl
plugins {
    agent-threads location="file:~/.config/zellij/plugins/agent-threads.wasm"
}
```

## Pipe commands

| Name | Payload | Behaviour |
|---|---|---|
| `agenthreads:agent` | Agent Report v2 JSON | Update rendered agent state |
| `agenthreads:refresh` | None | Reload the plugin instance |
| `agenthreads:toggle` | None | Hide or show the plugin pane |

Example keybindings:

```kdl
bind "Alt r" {
    MessagePlugin "agent-threads" {
        name "agenthreads:refresh"
    }
}
bind "Alt a" {
    MessagePlugin "agent-threads" {
        name "agenthreads:toggle"
    }
}
```

## Templates

Inline MiniJinja templates use the shared `zellij-template-render` renderer:

```kdl
plugin location="agent-threads" {
    template "{{ zellij_session }}: {{ agents | length }} agents"
}
```

For `{% include %}` / `{% import %}`, point `template_file` at the entry file:

```kdl
plugin location="agent-threads" {
    template_file "/home/q/.config/zellij-agent-threads/templates/main.jinja"
}
```

`template` and `template_file` are mutually exclusive. `template_file` accepts absolute paths,
`~/...`, and relative paths such as `./templates/main.jinja`. Relative paths use
`${ZELLIJ_CONFIG_DIR:-$HOME/.config/zellij}`.

External templates request Zellij `FullHdAccess`. After permission is granted, the plugin mounts
the template's parent directory as `/host` and loads the entry from the guest root. This permits
host-side symlink resolution without exposing the complete host filesystem. Includes and imports
resolve relative to the entry file. The plugin polls loaded template files once per second and
reloads them when their contents change.

See `demo-external.kdl`. After building, verify an agent-threads template that renders
`agent.title`:

```bash
python3 scripts/check-external-template.py --template-file ~/.config/zellij/templates/main.jinja
```

For another template shape, pass `--expect TEXT` and make the template render that injected
agent title.

Interactive entries use typed actions:

```jinja
{% for tab in tabs %}
{% if tab.agents | length > 0 %}
{% call Button(on_click=actions.switch_tab(tab.tab_id)) %}{{ tab.tab_name }}{% endcall %}
{% for agent in tab.agents %}
{% call Button(on_click=actions.focus_pane(agent.pane), focused=agent.focused) %}
{{ agent.title }}
{% endcall %}
{% endfor %}
{% endif %}
{% endfor %}
```

Layout uses nested `Flex` components. Colors passed to `fg`/`bg` use `index:N` or
`rgb:R,G,B`. MiniJinja's normal `format` filter remains available; timestamp formatting uses
`format_time`.

`AnimationFrame(fps=...)` returns a wall-clock frame number and schedules the next render. The
built-in template uses it at 8 FPS for running-agent icons. External templates can define the same
pattern:

```jinja
{% set frames = ["⠋", "⠙", "⠹", "⠸"] %}
{% if agent.state == "running" %}
  {{ frames[AnimationFrame(fps=8) % (frames | length)] }}
{% endif %}
```

Only executed calls request another render. Frequencies must be between 1 and 20 FPS, and frames
should have equal terminal width. Every animation frame reruns the complete template and layout.

## Protocol v2 and template migration

The plugin accepts only version-two Agent Reports. The payload must use `agent_id` and can include `session_name` for diagnostics. If `pane_id` is present, the plugin uses the pane as the stable row key. Reports with another `version` are rejected explicitly.

Template model v2 exposes `agents`, all current-session `tabs`, and `tabs[].agents`.
Agents without matching tab metadata stay in the flat `agents` list. The built-in template renders
only tabs that have agents.

Breaking change: `session`, `current_task`, `sessions`, `groups`, and `group.sessions` are removed.
Also removed: `template_dir`, `template_name`, `Grid`, `Stack`, `PaneButton`, `TabButton`,
`remap`, `italic`, and the old Flex `weights`/padding props.
