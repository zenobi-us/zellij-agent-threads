# zellij-plugin-agent-threads

Rust/WASM Zellij plugin that lists Pi agent sessions reported through Zellij pipes.

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

## Pipe name

```text
zellij-agent-threads
```

## Templates

Inline MiniJinja templates use the shared `zellij-template-render` renderer:

```kdl
plugin location="file:/path/to/zellij-plugin-agent-threads.wasm" {
    template "{{ zellij_session }}: {{ sessions | length }} agents"
}
```

For `{% include %}` / `{% import %}`, point `template_file` at the entry file:

```kdl
plugin location="file:/path/to/zellij-plugin-agent-threads.wasm" {
    template_file "/home/q/.config/zellij-agent-threads/templates/main.jinja"
}
```

`template` and `template_file` are mutually exclusive. External templates request Zellij
`FullHdAccess`. Includes/imports load lazily and remain cached until plugin reload. External
templates are trusted and can read files exposed to the plugin through `/host`.

Use a resolved absolute path for symlinked configuration trees:

```bash
readlink -f ~/.config/zellij/plugins/agent-threads/main.jinja
```

See `demo-external.kdl`. After building, verify an agent-threads template that renders
`session.title`:

```bash
python3 scripts/check-external-template.py --template-file "$(readlink -f /path/to/main.jinja)"
```

For another template shape, pass `--expect TEXT` and make the template render that injected
session title.

Interactive entries use typed actions:

```jinja
{% call Button(on_click=actions.switch_tab(group.tab_id)) %}{{ group.tab_name }}{% endcall %}
{% call Button(on_click=actions.focus_pane(session.pane), focused=session.focused) %}
{{ session.title }}
{% endcall %}
```

Layout uses nested `Flex` components. Colors passed to `fg`/`bg` use `index:N` or
`rgb:R,G,B`. MiniJinja's normal `format` filter remains available; timestamp formatting uses
`format_time`.

Breaking change: `template_dir`, `template_name`, `Grid`, `Stack`, `PaneButton`, `TabButton`,
`remap`, `italic`, and the old Flex `weights`/padding props are removed.
