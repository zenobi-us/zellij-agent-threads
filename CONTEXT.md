# Context

This repository is a Moon + proto monorepo for Zellij/Pi agent thread integration.

## Components

- Rust Zellij plugin receives `zellij pipe` JSON messages named `zellij-agent-threads` and renders active sessions.
- Pi extension publishes lifecycle events (`session_start`, `agent_start`, `agent_end`, `model_select`, `session_shutdown`) to the plugin.
- Docs app is copied from `boxfiles/boxfiles` as a ready Moon/Bun/Waku documentation scaffold.

## Important constraints

- Rust plugin must build for `wasm32-wasip1`.
- Unit tests run on host target because `.wasm` tests cannot execute directly on Linux without a WASI runner.
- Pi extension must treat Zellij pipe failures as non-fatal.

## Glossary

- Pane size sync: Rust plugin behavior that keeps multiple plugin panes at the same collapsed or expanded width after one plugin pane is toggled.
- Zellij layout adapter: concrete Rust code that performs Zellij layout side effects for pane size sync, including resizing plugin panes and sending layout pipe messages to peer plugin instances.
