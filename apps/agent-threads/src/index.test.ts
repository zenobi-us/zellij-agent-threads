import { afterEach, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { runAgentThreads } from "./index.js";
import type { AgentReportV2 } from "./store.js";

const dirs: string[] = [];

afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function tempDb(): string {
  const dir = mkdtempSync(join(tmpdir(), "agent-threads-cli-test-"));
  dirs.push(dir);
  return join(dir, "state.sqlite");
}

function report(agentId: string): AgentReportV2 {
  return {
    version: 2,
    harness: "pi",
    agent_id: agentId,
    cwd: "/tmp/project",
    zellij_session: "work",
    pane_id: "1",
    state: "running",
    updated_at: 1_000,
  };
}

test("CLI seam runs commands with controlled argv, env, and output", () => {
  const db = tempDb();
  const env = { AGENT_THREADS_DB: db };
  const stdout: string[] = [];
  const stderr: string[] = [];
  const io = { env, stdout: stdout.push.bind(stdout), stderr: stderr.push.bind(stderr), stdin: "" };

  expect(runAgentThreads({ argv: ["upsert", "--json", JSON.stringify(report("a"))], ...io })).toBe(0);
  expect(runAgentThreads({ argv: ["snapshot", "--json"], ...io })).toBe(0);
  expect(runAgentThreads({ argv: ["delete", "--agent-id", "a"], ...io })).toBe(0);
  expect(runAgentThreads({ argv: ["gc", "--json"], ...io })).toBe(0);
  expect(runAgentThreads({ argv: ["snapshot", "--json"], ...io })).toBe(0);
  expect(stderr.join("")).toBe("");

  const lines = stdout.join("").trim().split("\n");
  expect((JSON.parse(lines[0]!) as { agents: AgentReportV2[] }).agents.map((agent) => agent.agent_id)).toEqual(["a"]);
  expect(JSON.parse(lines[1]!)).toEqual({ removed: 0 });
  expect((JSON.parse(lines[2]!) as { agents: AgentReportV2[] }).agents).toEqual([]);
});

test("CLI seam reports errors without exiting the test process", () => {
  const stderr: string[] = [];

  expect(runAgentThreads({ argv: ["nope"], stdout: () => {}, stderr: stderr.push.bind(stderr) })).toBe(1);
  expect(stderr.join("")).toContain("unknown command: nope");
  expect(stderr.join("")).toContain("agent-threads upsert");
});
