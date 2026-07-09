# zellij-agent-threads

<img width="2750" height="1159" alt="image" src="https://github.com/user-attachments/assets/3c2ceb3f-d714-4a8b-9f3f-5b86fc4aef88" />


Moon + proto monorepo for the Zellij agent thread plugin and Pi extension.

## Projects

- `pkgs/plugins/zellij-plugin` — Rust/WASM Zellij plugin.
- `pkgs/plugins/pi-extension` — Pi extension that publishes session state through Zellij pipes.
- `apps/docs` — docs app copied from `boxfiles/boxfiles` as the monorepo docs scaffold.

## Setup

```bash
proto install
bun install
moon query projects
```

## Common tasks

```bash
moon run zellij-plugin:build
moon run zellij-plugin:test
moon run pi-extension:typecheck
moon run docs:dev
```

## Zellij plugin data flow

`pkgs/plugins/zellij-plugin` is a small Zellij lifecycle state machine. Pi publishes JSON session reports to the `zellij-agent-threads` pipe; the plugin stores the latest report per pane/session, folds Zellij pane/tab events into that runtime state, then renders a MiniJinja template into the plugin pane.

```text
                 Pi extension / zellij pipe
                           |
                           v
+------------------+   pipe:zellij-agent-threads   +------------------+
| Zellij lifecycle | ----------------------------> | RuntimeState     |
| load/pipe/update |                               | sessions/events  |
+--------+---------+                               +---+----------+---+
         |                                             |          ^
         | load: parse config, subscribe, get id        |          |
         v                                             |          |
+------------------+                                   |          |
| PluginConfig     |                                   |          |
| render + widths  |                                   |          |
+--------+---------+                                   |          |
         |                                             |          |
         | render(rows, cols)                           |          |
         v                                             |          |
+------------------+        MiniJinja data        +-----v---------+---+
| RenderModel      | <--------------------------- | RuntimeState     |
| groups/sessions  |                              | focused/active   |
+--------+---------+                              +------------------+
         |
         | template + filters/buttons
         v
+------------------+        hitboxes/action map    +------------------+
| Renderer         | ----------------------------> | Mouse click      |
| clear + print UI |                               | dispatch         |
+------------------+                               +---+----------+---+
                                                        |          |
                                tab button: switch_tab -+          |
                               pane button: focus_pane ------------+
```

Render-impacting session changes request repaint; hidden-field-only updates still refresh stored state without repaint. Shutdown reports and `PaneClosed` events remove stale sessions.
