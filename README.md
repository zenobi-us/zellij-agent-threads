# zellij-plugin-agent-worktree-list

Zellij plugin that lists pi agent sessions reported through Zellij pipes.

Pi extension included: `pi-zellij-agent.ts`. It sends JSON to the plugin with:

```bash
zellij pipe --name pi-agent-session -- '<json payload>'
```

## Flow

```text
pi session event
  -> pi-zellij-agent.ts runs `zellij pipe --name pi-agent-session`
  -> Zellij delivers PipeMessage to running plugin
  -> plugin upserts/removes session state
  -> plugin renders reported sessions
```

## Build

```bash
cd zellij-plugin-agent-worktree-list
mise trust
mise run build
```

Output:

```text
target/wasm32-wasip1/release/zellij_plugin_agent_worktree_list.wasm
```

## Run in Zellij

```bash
zellij action start-or-reload-plugin \
  file:target/wasm32-wasip1/release/zellij_plugin_agent_worktree_list.wasm
```

## Load Pi extension

Quick test:

```bash
ZELLIJ_AGENT_PLUGIN_URL=file:target/wasm32-wasip1/release/zellij_plugin_agent_worktree_list.wasm \
  pi -e ./pi-zellij-agent.ts
```

Auto-load:

```bash
mkdir -p ~/.pi/agent/extensions
cp pi-zellij-agent.ts ~/.pi/agent/extensions/zellij-agent.ts
export ZELLIJ_AGENT_PLUGIN_URL=file:/home/zenobius/Projects/zellij-plugin-agent-worktree-list/target/wasm32-wasip1/release/zellij_plugin_agent_worktree_list.wasm
# inside pi: /reload
```

Manual publish:

```text
/zellij-agent-publish
```

## Debug trail

Pi extension now writes best-effort trace lines to:

```text
/tmp/pi-zellij-agent-<uid>.log
```

Pi footer shows `zellij publishing|ok|error`; `/zellij-agent-publish` notification includes the log path.

Zellij plugin renders a non-blank initial status:

```text
zellij-agent pipes=0 sessions=0
waiting for pi extension reports
plugin loaded
```

After traffic, it shows pipe count, session count/error, and last pipe events.

## Notes

- Plugin uses Zellij pipes, not terminal titles.
- Plugin asks for no Zellij application-state permission.
- Pi extension sends targeted pipes with `--plugin`; this launches the plugin if missing.
- Source docs: Zellij User Guide says pipes send arbitrary text payloads, can be named, can target a plugin URL and launch it if missing, and Rust plugins receive them via `fn pipe(&mut self, PipeMessage) -> bool`.
