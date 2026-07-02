---
title: Why Zellij Agent Threads
description: |
  Why Zellij Agent Threads exists: a Zellij panel for tracking agent sessions across panes and tabs.
---

# Why Zellij Agent Threads

Zellij Agent Threads shows agent sessions inside Zellij.

It gives you a sidebar or floating panel that lists agents running anywhere in the current Zellij session, across panes and tabs.

## The problem

Agent work spreads across tabs, panes, and worktrees. After a few prompts, it gets hard to tell what is still running and where it lives.

You need one terminal-native view that answers:

- which agents are running?
- which agents are idle?
- which tab are they in?
- which worktree are they using?
- what belongs to this Zellij session?

```text
agent session in any pane or tab
  -> status report
  -> Zellij panel
```

## The default view

The default list groups agent sessions by tab:

```text
{TabName} [count]
  - {agentname} {status}
    {worktreepath}
  - {agentname} {status}
    {worktreepath}
```

Example:

```text
frontend [2]
  - claude running
    ~/src/app/.worktrees/nav-redesign
  - qwen idle
    ~/src/app

infra [1]
  - codex running
    ~/src/ops/.worktrees/tf-cleanup
```

## What it optimizes for

### Current-session focus

By default, the panel only shows agent sessions from the current Zellij session. The goal is not a global dashboard. It is local awareness for the workspace you are already using.

### Pane and tab coverage

A session can start in any pane on any tab. The panel should collect those reports and group them into one readable list.

### Worktree clarity

The worktree path matters more than process trivia. When several agents are active, the first question is usually “what code is this touching?”

### Harness-neutral reporting

The first supported harness is only the first integration. Any harness should be able to publish session status if it can report agent name, status, tab, pane, and worktree path.

## What this is not

- not an orchestrator
- not a scheduler
- not durable history
- not telemetry
- not a web dashboard

It is a Zellij-native status list for agent sessions.

## Good fit

Use it when you want:

- a sidebar list of agent sessions
- a floating “what is running?” panel
- status from panes across all tabs
- grouping by tab
- worktree paths at a glance
- current-session filtering by default

## Bad fit

Use something else when you need:

- cross-machine tracking
- permanent audit logs
- metrics
- remote control
- workflow automation

Zellij Agent Threads should stay small: receive session reports, group them, render a useful list.
