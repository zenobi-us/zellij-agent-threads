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

## Language

**Agent**:
An active Pi runtime represented by one Zellij terminal pane. Resuming or replacing Pi work in the same pane updates the same agent.
_Avoid_: Session, agent session

**Zellij session**:
A named running Zellij environment containing tabs, panes, clients, and plugins.
_Avoid_: Agent session, workspace

**Session summary**:
The compact metadata shown for one Zellij session, including native Zellij counts and aggregate agent activity.
_Avoid_: Global agent registry, remote agent list

**Template button**:
Rendered text that maps click hitboxes to a Zellij action such as focusing a pane, switching a tab, or switching a Zellij session.

**Harness**:
An AI agent runtime that can report agent activity to `agent-threads`.
_Avoid_: Client, integration

**Harness manifest**:
A declarative record that names a supported harness and its install assets, paths, and documentation links.
_Avoid_: Harness plugin, adapter script

**Released installer**:
The `agent-threads install` path that installs release assets for the same version as the running CLI.
_Avoid_: Source install, dev install

**Self-update**:
The `agent-threads self-update` path that replaces the CLI binary in `~/.local/bin` from a selected release channel.
_Avoid_: Install, upgrade everything
