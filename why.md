# Why Zellij Agent Threads (/why)



# Why Zellij Agent Threads [#why-zellij-agent-threads]

Zellij Agent Threads shows active AI Agents inside Zellij.

It gives you one terminal-native panel for Agents that run across tabs, panes, and worktrees.

## The problem [#the-problem]

Agent work spreads quickly.

One Agent can run in a feature worktree. Another Agent can run tests in a different tab. A third Agent can sit idle after a review.

After a few prompts, tab names are not enough. You need to know where each Agent lives and what each Agent is doing.

Zellij Agent Threads answers these questions:

* Which Agents are running?
* Which Agents are idle?
* Which tab contains an Agent?
* Which pane contains an Agent?
* Which worktree does the Agent use?
* Which Agents belong to this Zellij session?

## The model [#the-model]

A harness gives Agent state to `agent-threads`.

Pi is the first supported harness. Its extension is a wrapper that turns Pi lifecycle events into `agent-threads` CLI calls.

The CLI writes Agent Reports to SQLite. The Zellij plugin reads snapshots from the CLI and renders a panel.

```text
Pi event
  -> Pi extension wrapper
  -> agent-threads upsert/delete
  -> SQLite store
  -> Zellij plugin snapshot
  -> Zellij plugin panel
```

## The default view [#the-default-view]

The default panel groups Agents by Zellij tab:

```text
frontend [2]
  - pi running
    ~/src/app/.worktrees/nav-redesign
  - pi idle
    ~/src/app

infra [1]
  - pi running
    ~/src/ops/.worktrees/tf-cleanup
```

The panel also shows pane data, model data, titles, current tools, and recent plugin events.

## What this project optimizes for [#what-this-project-optimizes-for]

Zellij Agent Threads optimizes for a small live overview.

It is not an Agent runner. It does not start Agents. It does not replace Pi, Zellij, or your task tracker.

It only answers one question: what is active in this Zellij session?

## Why Zellij [#why-zellij]

Zellij already knows the terminal layout.

It knows tabs, panes, clients, and sessions. It can host a plugin pane next to your terminals. It can focus a pane when you click an entry.

This makes Zellij a good place for the Agent overview.

## Current limits [#current-limits]

This project is in early development.

The first supported harness is Pi. Other harnesses can integrate by writing Agent Reports to the `agent-threads` CLI.

The template API can change while the project is young. Pin release versions when you share templates across machines.
