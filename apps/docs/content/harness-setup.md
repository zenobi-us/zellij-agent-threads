---
title: Generic harness setup
description: Connect any agent harness that can run hook commands to Zellij Agent Threads.
status: draft
category: reference
tags:
  - reference
  - harness
  - hooks
---

# Generic harness setup

Use this page when a harness can run hook commands and pass session data to those commands.

The hook command writes an Agent Report to the `agent-threads` CLI. The CLI writes to the SQLite store. The Zellij plugin reads the store.

## Event contract

Map harness events to these CLI commands.

| Harness event | When to run it | CLI command | Best payload |
| --- | --- | --- | --- |
| Session start or resume | The harness starts or resumes a session. | `agent-threads upsert --json '<AgentReportV2>'` | `state: "idle"`, `activity: "settled"`, session fields, pane fields, `updated_at` |
| User prompt submit | The user submits a prompt. | `agent-threads upsert --json '<AgentReportV2>'` | `state: "running"`, `activity: "thinking"`, prompt session fields, `updated_at` |
| Agent turn start | The model starts work. | `agent-threads upsert --json '<AgentReportV2>'` | `state: "running"`, `activity: "thinking"`, `sequence`, `updated_at` |
| Tool start | The model starts a tool call. | `agent-threads upsert --json '<AgentReportV2>'` | `state: "running"`, `activity: "tool_running"`, `current_tool`, `current_tool_kind: "tool"`, `last_tool`, `last_tool_at` |
| User question start | The model asks the user for input. | `agent-threads upsert --json '<AgentReportV2>'` | `state: "running"`, `activity: "waiting_for_user"`, `current_tool`, `current_tool_kind: "user_question"` |
| Tool end | A tool call exits. | `agent-threads upsert --json '<AgentReportV2>'` | `state: "running"`, `activity: "thinking"`, clear `current_tool`, keep `last_tool` and `last_tool_at` |
| Agent turn end | The model finishes, fails, or is aborted. | `agent-threads upsert --json '<AgentReportV2>'` | `state: "idle"`, `activity: "settled"`, `settled_reason`, `settled_message` if available |
| Heartbeat | No event fires for 5 seconds while the session is alive. | `agent-threads upsert --json '<AgentReportV2>'` | Repeat the last known payload with a new `sequence` and `updated_at` |
| Session end | The harness exits or replaces the session. | `agent-threads delete --agent-id '<id>'` | No JSON payload |

Each upsert renews a 10-second lease. Send a heartbeat every 5 seconds while the Agent can still be shown.

If the harness can send events out of order, include `sequence`. Increase it by 1 for each report from the same Agent.

## `agent-threads upsert`

Writes or replaces one active Agent row.

```sh
agent-threads upsert --json '<AgentReportV2>'
```

The command reads one JSON object.

```json
{
  "version": 2,
  "harness": "example-harness",
  "agent_id": "example:work:42",
  "session_name": "session-123",
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
  "last_tool": "bash",
  "last_tool_at": 1730000000000,
  "settled_reason": "finished",
  "settled_message": "done",
  "sequence": 12,
  "updated_at": 1730000000000
}
```

### Agent Report v2 fields

| Field | Type | Required | Use |
| --- | --- | --- | --- |
| `version` | `2` | yes | Protocol version. |
| `harness` | string | no | Harness name, for example `claude-code` or `codex`. |
| `agent_id` | string | yes | Stable Agent identity. Use the Zellij pane when possible. |
| `session_name` | string | no | Harness session name or transcript name. |
| `cwd` | string | yes | Current working directory. |
| `zellij_session` | string | no | `$ZELLIJ_SESSION_NAME` when the hook runs inside Zellij. |
| `pane_id` | string | no | `$ZELLIJ_PANE_ID` when the hook runs inside Zellij. |
| `tab_id` | number | no | Native Zellij tab ID when the harness can read it. |
| `tab_name` | string | no | Native Zellij tab name when the harness can read it. |
| `state` | `idle`, `running`, `shutdown` | yes | Agent lifecycle state. |
| `activity` | `thinking`, `tool_running`, `waiting_for_user`, `settled` | no | More precise activity for the row. |
| `model` | string | no | Active model name when the harness exposes it. |
| `title` | string | no | Short title for the row. Prefer the pane title. |
| `current_tool` | string | no | Tool that is running now. |
| `current_tool_kind` | `tool`, `user_question` | no | Type of the running tool. |
| `last_tool` | string | no | Last tool that started or ended. |
| `last_tool_at` | number | no | Unix time in milliseconds for `last_tool`. |
| `settled_reason` | `finished`, `failed`, `aborted` | no | Result of the last turn. |
| `settled_message` | string | no | Short failure or abort text. |
| `sequence` | number | no | Monotonic order value for this Agent. |
| `updated_at` | number | yes | Unix time in milliseconds for this report. |

If `pane_id` exists, the store key is `zellij_session:pane_id`. This makes a restarted Agent in the same pane replace the old row.

If `pane_id` does not exist, the store key is `agent_id`.

## `agent-threads delete`

Deletes one Agent row.

```sh
agent-threads delete --agent-id '<id>'
```

Run this command when the harness exits.

You can also publish `state: "shutdown"`. The CLI treats that report as a delete.

## `agent-threads snapshot`

Reads the current store.

```sh
agent-threads snapshot --json
```

Use this command to debug an integration. Do not use it as the source of truth inside a hook.

## Claude Code example

Claude Code command hooks receive JSON on `stdin`. See the [Claude Code hooks reference](https://code.claude.com/docs/en/hooks.md). Put a small publisher script on disk, then call it from hook events.

Example `.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          { "type": "command", "command": "node .claude/hooks/agent-threads.mjs" }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          { "type": "command", "command": "node .claude/hooks/agent-threads.mjs" }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          { "type": "command", "command": "node .claude/hooks/agent-threads.mjs" }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          { "type": "command", "command": "node .claude/hooks/agent-threads.mjs" }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          { "type": "command", "command": "node .claude/hooks/agent-threads.mjs" }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          { "type": "command", "command": "node .claude/hooks/agent-threads.mjs", "timeout": 3 }
        ]
      }
    ]
  }
}
```

Example `.claude/hooks/agent-threads.mjs`:

```js
#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const input = JSON.parse(readFileSync(0, "utf8") || "{}");
const now = Date.now();
const zellijSession = process.env.ZELLIJ_SESSION_NAME;
const paneId = process.env.ZELLIJ_PANE_ID;
const agentId = paneId
  ? `claude-code:${zellijSession ?? "zellij"}:${paneId}`
  : `claude-code:${input.session_id}`;

if (input.hook_event_name === "SessionEnd") {
  spawnSync("agent-threads", ["delete", "--agent-id", agentId], { stdio: "ignore" });
  process.exit(0);
}

const tool = input.tool_name;
const event = input.hook_event_name;
const payload = {
  version: 2,
  harness: "claude-code",
  agent_id: agentId,
  session_name: input.session_id,
  cwd: input.cwd ?? process.cwd(),
  zellij_session: zellijSession,
  pane_id: paneId,
  state: event === "Stop" || event === "SessionStart" ? "idle" : "running",
  activity: event === "PreToolUse" ? "tool_running" : event === "Stop" || event === "SessionStart" ? "settled" : "thinking",
  title: process.env.ZELLIJ_PANE_TITLE,
  current_tool: event === "PreToolUse" ? tool : undefined,
  current_tool_kind: event === "PreToolUse" ? "tool" : undefined,
  last_tool: tool,
  last_tool_at: tool ? now : undefined,
  settled_reason: event === "Stop" ? "finished" : undefined,
  updated_at: now
};

spawnSync("agent-threads", ["upsert", "--json", JSON.stringify(payload)], { stdio: "ignore" });
```

## Codex example

Codex command hooks also receive JSON on `stdin`. See the [Codex hooks reference](https://developers.openai.com/codex/hooks). Put the same publisher shape behind Codex hook events.

Example `.codex/hooks.json`:

```json
{
  "description": "Publish Codex sessions to Zellij Agent Threads.",
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          { "type": "command", "command": "node .codex/hooks/agent-threads.mjs" }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          { "type": "command", "command": "node .codex/hooks/agent-threads.mjs" }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          { "type": "command", "command": "node .codex/hooks/agent-threads.mjs" }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          { "type": "command", "command": "node .codex/hooks/agent-threads.mjs" }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          { "type": "command", "command": "node .codex/hooks/agent-threads.mjs" }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          { "type": "command", "command": "node .codex/hooks/agent-threads.mjs", "timeout": 3 }
        ]
      }
    ]
  }
}
```

Example `.codex/hooks/agent-threads.mjs`:

```js
#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const input = JSON.parse(readFileSync(0, "utf8") || "{}");
const now = Date.now();
const zellijSession = process.env.ZELLIJ_SESSION_NAME;
const paneId = process.env.ZELLIJ_PANE_ID;
const agentId = paneId
  ? `codex:${zellijSession ?? "zellij"}:${paneId}`
  : `codex:${input.session_id}`;

if (input.hook_event_name === "SessionEnd") {
  spawnSync("agent-threads", ["delete", "--agent-id", agentId], { stdio: "ignore" });
  process.exit(0);
}

const tool = input.tool_name;
const event = input.hook_event_name;
const payload = {
  version: 2,
  harness: "codex",
  agent_id: agentId,
  session_name: input.session_id,
  cwd: input.cwd ?? process.cwd(),
  zellij_session: zellijSession,
  pane_id: paneId,
  state: event === "Stop" || event === "SessionStart" ? "idle" : "running",
  activity: event === "PreToolUse" ? "tool_running" : event === "Stop" || event === "SessionStart" ? "settled" : "thinking",
  model: input.model,
  title: process.env.ZELLIJ_PANE_TITLE,
  current_tool: event === "PreToolUse" ? tool : undefined,
  current_tool_kind: event === "PreToolUse" ? "tool" : undefined,
  last_tool: tool,
  last_tool_at: tool ? now : undefined,
  settled_reason: event === "Stop" ? "finished" : undefined,
  updated_at: now
};

spawnSync("agent-threads", ["upsert", "--json", JSON.stringify(payload)], { stdio: "ignore" });
```

## Limits of hook-only setup

A hook-only setup is best effort.

If no hook fires for more than 10 seconds, the row expires. Add a harness-native heartbeat when the harness supports long quiet turns.

If the hook cannot read Zellij pane data, the plugin cannot focus the Agent pane. The row can still appear by `agent_id`.
