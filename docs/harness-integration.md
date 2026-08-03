# Harness integration contract

A harness can report agents to `agent-threads` without a first-class installer.

## Report an agent

Call `agent-threads upsert` when an agent starts or changes state.

```bash
agent-threads upsert --json '<AgentReportV2>'
```

The JSON object must use Agent Report v2.

## Delete an agent

Call `agent-threads delete` when the agent exits.

```bash
agent-threads delete --agent-id '<id>'
```

## Read the snapshot

Call `agent-threads snapshot --json` to read the current store.

```bash
agent-threads snapshot --json
```

The command prints the current Agent Snapshot JSON.
