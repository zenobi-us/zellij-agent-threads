# Context

This repository is a Moon + proto monorepo for Zellij/Pi agent thread integration.

## Components

- Rust Zellij plugin receives `agenthreads:agent` JSON pipe messages and namespaced `agenthreads:refresh`/`agenthreads:toggle` control messages.
- Pi extension publishes lifecycle events (`session_start`, `agent_start`, `agent_end`, `model_select`, `session_shutdown`) directly to the configured `agent-threads` plugin alias.
- Docs app is copied from `boxfiles/boxfiles` as a ready Moon/Bun/Waku documentation scaffold.

## Important constraints

- Rust plugin must build for `wasm32-wasip1`.
- Unit tests run on host target because `.wasm` tests cannot execute directly on Linux without a WASI runner.
- Pi extension must treat Zellij pipe failures as non-fatal.

## Glossary

- Template button: rendered text that maps click hitboxes to Zellij actions such as focusing panes or switching tabs.
