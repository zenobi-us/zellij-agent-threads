import { Database } from "bun:sqlite";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";

export const STORE_VERSION = 1;
export const DEFAULT_LEASE_MS = 10_000;

export type AgentState = "idle" | "running" | "shutdown";

export type AgentReportV2 = {
  version: 2;
  harness?: string;
  agent_id: string;
  session_name?: string;
  cwd: string;
  zellij_session?: string;
  pane_id?: string;
  tab_id?: number;
  tab_name?: string;
  state: AgentState;
  model?: string;
  title?: string;
  current_tool?: string;
  updated_at: number;
};

export type AgentSnapshot = {
  version: 1;
  agents: AgentReportV2[];
};

type AgentRow = {
  agent_id: string;
  session_name: string | null;
  cwd: string;
  zellij_session: string | null;
  pane_id: string | null;
  tab_id: number | null;
  tab_name: string | null;
  state: AgentState;
  model: string | null;
  title: string | null;
  current_tool: string | null;
  updated_at: number;
};

export function defaultStorePath(env: NodeJS.ProcessEnv = process.env): string {
  const runtimeDir = env.XDG_RUNTIME_DIR || tmpdir();
  return join(runtimeDir, "zellij-agent-threads", "state.sqlite");
}

export function agentKey(report: Pick<AgentReportV2, "agent_id" | "zellij_session" | "pane_id">): string {
  if (report.pane_id) return `${report.zellij_session ?? ""}:${report.pane_id}`;
  return report.agent_id;
}

export class AgentStore {
  readonly db: Database;

  constructor(readonly path = defaultStorePath()) {
    mkdirSync(dirname(path), { recursive: true });
    this.db = new Database(path);
    this.db.exec("PRAGMA journal_mode = WAL");
    this.db.exec("PRAGMA busy_timeout = 1000");
    this.migrate();
  }

  close(): void {
    this.db.close();
  }

  upsert(report: AgentReportV2, now = Date.now(), leaseMs = DEFAULT_LEASE_MS): void {
    validateReport(report);
    this.gc(now);
    const key = agentKey(report);
    if (report.state === "shutdown") {
      this.delete(report.agent_id, key);
      return;
    }
    const leaseUntil = now + leaseMs;
    this.db.query(`
      insert into agents (
        key, agent_id, session_name, zellij_session, pane_id, tab_id, tab_name,
        state, model, title, cwd, current_tool, updated_at, lease_until
      ) values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      on conflict(key) do update set
        agent_id = excluded.agent_id,
        session_name = excluded.session_name,
        zellij_session = excluded.zellij_session,
        pane_id = excluded.pane_id,
        tab_id = excluded.tab_id,
        tab_name = excluded.tab_name,
        state = excluded.state,
        model = excluded.model,
        title = excluded.title,
        cwd = excluded.cwd,
        current_tool = excluded.current_tool,
        updated_at = excluded.updated_at,
        lease_until = excluded.lease_until
    `).run(
      key,
      report.agent_id,
      report.session_name ?? null,
      report.zellij_session ?? null,
      report.pane_id ?? null,
      report.tab_id ?? null,
      report.tab_name ?? null,
      report.state,
      report.model ?? null,
      report.title ?? null,
      report.cwd,
      report.current_tool ?? null,
      report.updated_at,
      leaseUntil,
    );
  }

  delete(agentId: string, key = agentId): number {
    const result = this.db.query("delete from agents where key = ? or agent_id = ?").run(key, agentId);
    return result.changes;
  }

  gc(now = Date.now()): number {
    const result = this.db.query("delete from agents where lease_until <= ?").run(now);
    return result.changes;
  }

  snapshot(now = Date.now()): AgentSnapshot {
    this.gc(now);
    const rows = this.db.query<AgentRow, [number]>(`
      select agent_id, session_name, cwd, zellij_session, pane_id, tab_id, tab_name,
             state, model, title, current_tool, updated_at
      from agents
      where lease_until > ?
      order by coalesce(zellij_session, ''), coalesce(pane_id, ''), agent_id
    `).all(now);
    return { version: STORE_VERSION, agents: rows.map(rowToReport) };
  }

  private migrate(): void {
    this.db.exec(`
      create table if not exists agents(
        key text primary key,
        agent_id text not null,
        zellij_session text,
        pane_id text,
        tab_id integer,
        tab_name text,
        state text not null,
        model text,
        title text,
        cwd text not null,
        current_tool text,
        updated_at integer not null,
        lease_until integer not null
      )
    `);
    const columns = this.db.query<{ name: string }, []>("pragma table_info(agents)").all().map((row) => row.name);
    if (!columns.includes("session_name")) {
      this.db.exec("alter table agents add column session_name text");
    }
    this.db.exec("create index if not exists agents_lease_until_idx on agents(lease_until)");
  }
}

function rowToReport(row: AgentRow): AgentReportV2 {
  return withoutUndefined({
    version: 2 as const,
    harness: "pi",
    agent_id: row.agent_id,
    session_name: row.session_name ?? undefined,
    cwd: row.cwd,
    zellij_session: row.zellij_session ?? undefined,
    pane_id: row.pane_id ?? undefined,
    tab_id: row.tab_id ?? undefined,
    tab_name: row.tab_name ?? undefined,
    state: row.state,
    model: row.model ?? undefined,
    title: row.title ?? undefined,
    current_tool: row.current_tool ?? undefined,
    updated_at: row.updated_at,
  });
}

function withoutUndefined<T extends Record<string, unknown>>(value: T): T {
  for (const key of Object.keys(value)) {
    if (value[key] === undefined) delete value[key];
  }
  return value;
}

export function parseReportJson(json: string): AgentReportV2 {
  const value = JSON.parse(json) as AgentReportV2;
  validateReport(value);
  return value;
}

function validateReport(value: AgentReportV2): void {
  if (!value || typeof value !== "object") throw new Error("agent report must be an object");
  if (value.version !== 2) throw new Error("agent report version must be 2");
  if (!value.agent_id) throw new Error("agent report missing agent_id");
  if (!value.cwd) throw new Error("agent report missing cwd");
  if (!["idle", "running", "shutdown"].includes(value.state)) throw new Error("agent report has invalid state");
  if (!Number.isFinite(value.updated_at)) throw new Error("agent report missing updated_at");
}
