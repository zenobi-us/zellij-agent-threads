import { afterEach, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { AgentStore, type AgentReportV2 } from "./store.js";

const dirs: string[] = [];

afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function store(): AgentStore {
  const dir = mkdtempSync(join(tmpdir(), "agent-threads-test-"));
  dirs.push(dir);
  return new AgentStore(join(dir, "state.sqlite"));
}

function report(agentId: string, paneId = "1"): AgentReportV2 {
  return {
    version: 2,
    harness: "pi",
    agent_id: agentId,
    session_name: `/tmp/${agentId}.jsonl`,
    cwd: "/tmp/project",
    zellij_session: "work",
    pane_id: paneId,
    state: "running",
    model: "test-model",
    title: agentId,
    updated_at: 1_000,
  };
}

test("SQLite upsert replaces same pane", () => {
  const db = store();
  try {
    db.upsert(report("old", "42"), 1_000);
    db.upsert({ ...report("new", "42"), title: "new title", updated_at: 2_000 }, 2_000);

    const snapshot = db.snapshot(2_001);
    expect(snapshot.agents).toHaveLength(1);
    expect(snapshot.agents[0]).toMatchObject({
      agent_id: "new",
      pane_id: "42",
      title: "new title",
      updated_at: 2_000,
    });
  } finally {
    db.close();
  }
});

test("shutdown delete removes row", () => {
  const db = store();
  try {
    db.upsert(report("a", "7"), 1_000);
    expect(db.snapshot(1_001).agents).toHaveLength(1);

    db.upsert({ ...report("a", "7"), state: "shutdown" }, 2_000);
    expect(db.snapshot(2_001).agents).toHaveLength(0);
  } finally {
    db.close();
  }
});

test("delete command target removes matching agent id", () => {
  const db = store();
  try {
    db.upsert(report("a", "7"), 1_000);
    expect(db.delete("a")).toBe(1);
    expect(db.snapshot(1_001).agents).toHaveLength(0);
  } finally {
    db.close();
  }
});

test("GC removes expired rows", () => {
  const db = store();
  try {
    db.upsert(report("old", "1"), 1_000, 500);
    db.upsert(report("fresh", "2"), 1_000, 5_000);

    expect(db.gc(1_501)).toBe(1);
    expect(db.snapshot(1_501).agents.map((agent) => agent.agent_id)).toEqual(["fresh"]);
  } finally {
    db.close();
  }
});

test("snapshot deletes expired rows", () => {
  const db = store();
  try {
    db.upsert(report("old", "1"), 1_000, 500);
    db.upsert(report("fresh", "2"), 1_000, 5_000);

    expect(db.snapshot(1_501).agents.map((agent) => agent.agent_id)).toEqual(["fresh"]);
    expect(db.gc(1_501)).toBe(0);
  } finally {
    db.close();
  }
});

test("snapshot keeps lifecycle metadata", () => {
  const db = store();
  try {
    db.upsert({
      ...report("a", "7"),
      activity: "waiting_for_user",
      current_tool: "question",
      current_tool_kind: "user_question",
      last_tool: "question",
      last_tool_at: 1_500,
      settled_reason: "failed",
      settled_message: "boom",
      sequence: 3,
    }, 1_000);

    expect(db.snapshot(1_001).agents[0]).toMatchObject({
      activity: "waiting_for_user",
      current_tool: "question",
      current_tool_kind: "user_question",
      last_tool: "question",
      last_tool_at: 1_500,
      settled_reason: "failed",
      settled_message: "boom",
      sequence: 3,
    });
  } finally {
    db.close();
  }
});

test("older sequence cannot overwrite newer state", () => {
  const db = store();
  try {
    db.upsert({ ...report("a", "7"), state: "idle", sequence: 2, updated_at: 2_000 }, 2_000);
    db.upsert({ ...report("a", "7"), state: "running", sequence: 1, updated_at: 1_000 }, 1_000);

    expect(db.snapshot(2_001).agents[0]).toMatchObject({
      state: "idle",
      sequence: 2,
      updated_at: 2_000,
    });
  } finally {
    db.close();
  }
});
