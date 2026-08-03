# Harness integration contract

A harness can report agents to `agent-threads` without a first-class installer.
This contract is unsupported and best-effort.
Only the documented Agent Report version is part of this contract.

## Requirements

- Call the `agent-threads` CLI from the harness process.
- Send Agent Report v2 JSON to `agent-threads upsert`.
- Send a heartbeat upsert before the 10-second lease expires.
- Delete the agent row when the harness exits.
- Use `agent-threads snapshot --json` only to inspect the current store.

The store path defaults to `$XDG_RUNTIME_DIR/zellij-agent-threads/state.sqlite`.
Set `AGENT_THREADS_DB` or pass `--db path` to use another store.

## Report an agent

Call `agent-threads upsert` when an agent starts, changes state, or sends a heartbeat.

```bash
agent-threads upsert --json '<AgentReportV2>'
```

The JSON object must use Agent Report v2:

```json
{
  "version": 2,
  "harness": "example-harness",
  "agent_id": "agent-123",
  "session_name": "work",
  "cwd": "/home/you/project",
  "zellij_session": "work",
  "pane_id": "42",
  "tab_id": 1,
  "tab_name": "agents",
  "state": "running",
  "activity": "tool_running",
  "model": "model-name",
  "title": "agent title",
  "current_tool": "bash",
  "current_tool_kind": "tool",
  "sequence": 12,
  "updated_at": 1730000000000
}
```

Required fields are `version`, `agent_id`, `cwd`, `state`, and `updated_at`.
The `state` value must be `idle`, `running`, or `shutdown`.
Use `sequence` if the harness can send events out of order.

If `pane_id` exists, the store key is `zellij_session:pane_id`.
If `pane_id` does not exist, the store key is `agent_id`.
Each upsert renews a 10-second lease.
Rows expire 10 seconds after the last upsert.
Send a heartbeat upsert before that lease expires.
A 5-second interval leaves room for scheduler delay.
If the lease expires, the agent disappears from snapshots until the next upsert.

## Delete an agent

Call `agent-threads delete` when the agent exits.

```bash
agent-threads delete --agent-id '<id>'
```

You can also send a report with `state` set to `shutdown`.

## Read the snapshot

Call `agent-threads snapshot --json` to read the current store.

```bash
agent-threads snapshot --json
```

The command prints the current Agent Snapshot JSON.
