---
title: How it works
description: How Pi reports, the agent-threads store, and the Zellij plugin keep the agent list current.
---

# How it works

Zellij Agent Threads has three small parts:

- the Pi extension
- the `agent-threads` CLI and SQLite store
- the Zellij plugin

The store is the source of truth. The pipe only wakes the plugin.

```mermaid
flowchart LR
  Pi[Pi extension] --> Store[agent-threads store]
  Plugin[Zellij plugin] --> Store
  Pi -. wake .-> Plugin
```

## 1. Pi writes reports

The Pi extension listens to Pi lifecycle events. It publishes an Agent Report when the session starts, runs, becomes idle, uses a tool, or shuts down.

Each report contains the agent state, pane ID, tab data, working directory, model, title, and update time.

```mermaid
flowchart LR
  Event[Pi event] --> Report[Agent Report v2]
  Report --> CLI[agent-threads upsert]
  CLI --> DB[(SQLite store)]
```

On shutdown, the extension deletes the row instead of writing a closed row.

```mermaid
flowchart LR
  Shutdown[session_shutdown] --> Delete[agent-threads delete]
  Delete --> DB[(SQLite store)]
```

## 2. Pi wakes the plugin

After a store write, the Pi extension sends a Zellij pipe message.

This message does not carry the agent list. It only tells the plugin to read the store.

```mermaid
sequenceDiagram
  participant Pi as Pi extension
  participant Zellij as zellij pipe
  participant Plugin as Zellij plugin
  Pi->>Zellij: agenthreads:refresh
  Zellij->>Plugin: refresh
  Plugin->>Plugin: schedule snapshot
```

The extension also calls `zellij action list-panes --json`. It uses this data to add pane and tab metadata to the report.

```mermaid
flowchart LR
  Pi[Pi extension] --> Panes[zellij action list-panes]
  Panes --> Report[Report title and tab data]
```

## 3. The store keeps one row per agent location

The CLI stores reports in SQLite. The row key uses the Zellij pane when it exists.

This means a restarted agent in the same pane replaces the old row.

```mermaid
flowchart LR
  ReportA[pane 42 old agent] --> Key[pane key work:42]
  ReportB[pane 42 new agent] --> Key
  Key --> Row[one store row]
```

Each row has a lease. The default lease is 10 seconds. Each new report extends the lease.

```mermaid
flowchart LR
  Upsert[upsert report] --> Lease[lease_until = now + 10s]
  Lease --> Row[(agent row)]
```

## 4. The plugin reads snapshots

The Zellij plugin does not receive all report data through the pipe. It runs the CLI and reads a snapshot.

```mermaid
flowchart LR
  Timer[plugin timer or refresh pipe] --> CLI[agent-threads snapshot --json]
  CLI --> DB[(SQLite store)]
  DB --> Snapshot[AgentSnapshot]
  Snapshot --> Render[render panel]
```

The plugin keeps a small in-memory model. A valid snapshot replaces that model.

```mermaid
flowchart LR
  Snapshot[store snapshot] --> Filter[drop closed and stale panes]
  Filter --> Model[plugin model]
  Model --> UI[Zellij panel]
```

## 5. Stale entries are removed

Stale entries can come from a crash, a killed pane, or a missed shutdown event.

The system removes stale entries in three places.

### Store garbage collection

`agent-threads` removes expired rows when it writes or reads the store.

```mermaid
flowchart LR
  ReadOrWrite[snapshot or upsert] --> GC[delete rows where lease expired]
  GC --> DB[(SQLite store)]
```

### Plugin lease expiry

The plugin also expires agents from memory after 10 seconds without a report.

```mermaid
flowchart LR
  NoReport[no report for 10s] --> Expire[expire_silent_agents]
  Expire --> UI[remove from panel]
```

### Closed pane filtering

The plugin compares snapshot rows with live Zellij panes. If a row has a pane ID that no longer exists, the plugin ignores that row.

```mermaid
flowchart LR
  SnapshotRow[row with pane_id] --> LivePanes[Zellij session panes]
  LivePanes -->|pane exists| Keep[keep row]
  LivePanes -->|pane missing| Drop[drop row]
```

## Result

A normal update uses this path:

```mermaid
sequenceDiagram
  participant Pi as Pi extension
  participant CLI as agent-threads CLI
  participant DB as SQLite store
  participant Plugin as Zellij plugin
  Pi->>CLI: upsert report
  CLI->>DB: write row and refresh lease
  Pi-->>Plugin: agenthreads:refresh
  Plugin->>CLI: snapshot --json
  CLI->>DB: delete expired rows, then read rows
  DB-->>Plugin: live reports
  Plugin->>Plugin: filter closed panes
```

The store cleans old rows. The plugin also protects the panel from stale rows. No background sweeper is required.
