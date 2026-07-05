import type { ExtensionContext } from "@earendil-works/pi-coding-agent";
import { spawn } from "node:child_process";
import { appendFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import type { StatusWidget, StatusValues } from "./status.js";

export const PIPE_NAME = "zellij-agent-threads";
export const LOG_FILE = `${tmpdir()}/pi-zellij-agent-${process.getuid?.() ?? "user"}.log`;
export const REFRESH_MS = 2_000;

export type AgentState = "idle" | "running" | "shutdown";

type PaneTabInfo = {
  id?: number;
  is_plugin?: boolean;
  tab_id?: number;
  tab_name?: string;
  title?: string;
  name?: string;
};

type PublisherState = {
  state: AgentState;
  title?: string;
  currentTool?: string;
};

/**
 * Owns every Zellij-facing side effect: pipe payloads, pane metadata lookup,
 * heartbeat refreshes, and debug logging.
 *
 * Keeping this isolated makes Pi event hooks pure orchestration: they update
 * lifecycle/tool state, then ask this class to publish the current snapshot.
 */
export class ZellijPublisher {
  lastError: string | undefined;
  publishCount = 0;
  private refreshTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(
    private statusWidget: StatusWidget,
    private state: PublisherState = { state: "idle" },
  ) {}

  /**
   * Session config can change on reload/resume, so the publisher keeps the same
   * transport state while swapping only the footer renderer.
   */
  updateStatusWidget(statusWidget: StatusWidget): void {
    this.statusWidget = statusWidget;
  }

  /**
   * Gives lifecycle hooks one place to mutate publishable state before any pipe
   * write. This avoids passing tool/lifecycle data through every method call.
   */
  update(values: Partial<PublisherState>): void {
    this.state = { ...this.state, ...values };
  }

  /**
   * Maps publisher state to footer interpolation keys. Title is populated from
   * Zellij pane metadata during publish, not from conversation text.
   */
  statusValues(): StatusValues {
    return {
      state: this.state.state,
      title: this.state.title,
      tool: this.state.currentTool,
    };
  }

  /**
   * Sends the current Pi session snapshot to the Zellij plugin. Status updates
   * bracket the pipe write so users can see whether transport is stuck or failed.
   */
  async publish(ctx: ExtensionContext, nextState = this.state.state, updateStatus = true): Promise<void> {
    try {
      this.state.state = nextState;
      this.publishCount += 1;
      if (updateStatus) this.statusWidget.update(ctx, "publishing", this.statusValues());
      const tab = await this.paneTabInfo();
      const paneTitle = tab?.title ?? tab?.name ?? tab?.tab_name;
      this.state.title = paneTitle;
      const payload = JSON.stringify({
        version: 1,
        harness: "pi",
        session: this.sessionKey(ctx),
        cwd: ctx.cwd,
        zellij_session: process.env.ZELLIJ_SESSION_NAME,
        pane_id: process.env.ZELLIJ_PANE_ID,
        tab_id: tab?.tab_id,
        tab_name: tab?.tab_name,
        state: this.state.state,
        model: ctx.model?.id,
        title: paneTitle,
        current_tool: this.state.currentTool,
        updated_at: Date.now(),
      });

      await this.trace(`publish state=${this.state.state} bytes=${payload.length}`);
      await this.pipeToPlugin(payload);
      this.lastError = undefined;
      if (updateStatus) this.statusWidget.update(ctx, "ok", this.statusValues());
      await this.trace(`pipe ok state=${this.state.state}`);
    } catch (error) {
      this.lastError = error instanceof Error ? error.message : String(error);
      if (updateStatus) this.statusWidget.update(ctx, "error", this.statusValues());
      await this.trace(`pipe error state=${this.state.state} error=${this.lastError}`);
    }
  }

  /**
   * Stops heartbeat refreshes before shutdown/session replacement so the old Pi
   * context cannot keep publishing after its runtime is torn down.
   */
  stopRefresh(): void {
    if (!this.refreshTimer) return;
    clearTimeout(this.refreshTimer);
    this.refreshTimer = undefined;
  }

  /**
   * Keeps the Zellij plugin fresh even when no Pi events fire for a while. This
   * is intentionally a refresh loop, not a permanent interval, so each publish
   * finishes before the next one is scheduled.
   */
  scheduleRefresh(ctx: ExtensionContext): void {
    this.stopRefresh();
    this.refreshTimer = setTimeout(() => {
      void this.publish(ctx, this.state.state, false).finally(() => {
        this.statusWidget.update(ctx, "refreshing", this.statusValues());
        if (this.state.state !== "shutdown") this.scheduleRefresh(ctx);
      });
    }, REFRESH_MS);
  }

  /**
   * Uses the Zellij pane as the stable identity when available because resumed Pi
   * sessions in the same pane should replace, not duplicate, the displayed row.
   */
  sessionKey(ctx: ExtensionContext): string {
    const paneId = process.env.ZELLIJ_PANE_ID;
    if (paneId) return `${process.env.ZELLIJ_SESSION_NAME ?? "zellij"}:${paneId}`;
    return ctx.sessionManager.getSessionFile() ?? `${ctx.cwd}:${process.pid}`;
  }

  /**
   * Reads pane metadata from Zellij so the plugin receives the terminal pane
   * title instead of inventing a conversation summary locally.
   */
  paneTabInfo(paneId = process.env.ZELLIJ_PANE_ID): Promise<PaneTabInfo | undefined> {
    return new Promise<PaneTabInfo | undefined>((resolve) => {
      const child = spawn("zellij", ["action", "list-panes", "--json"], {
        stdio: ["ignore", "pipe", "ignore"],
      });
      let stdout = "";
      child.stdout.setEncoding("utf8");
      child.stdout.on("data", (chunk) => { stdout += chunk; });
      child.on("error", () => resolve(undefined));
      child.on("exit", (code) => {
        if (code !== 0) return resolve(undefined);
        resolve(parsePaneTabInfo(stdout, paneId));
      });
    });
  }

  /**
   * Uses `zellij pipe` instead of direct plugin IPC because it is the stable,
   * user-facing boundary Zellij exposes to external processes.
   */
  pipeToPlugin(payload: string): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      const child = spawn("zellij", ["pipe", "--name", PIPE_NAME, "--", payload], {
        stdio: "ignore",
      });

      child.on("error", reject);
      child.on("exit", (code, signal) => {
        if (code === 0) resolve();
        else reject(new Error(`zellij pipe failed code=${code} signal=${signal}`));
      });
    });
  }

  /**
   * Writes a flat debug trail outside the session file so publish failures remain
   * inspectable even when the Pi UI cannot render them.
   */
  private async trace(message: string): Promise<void> {
    await appendFile(LOG_FILE, `${new Date().toISOString()} ${message}\n`);
  }
}


export function parsePaneTabInfo(stdout: string, paneId: string | undefined): PaneTabInfo | undefined {
  if (!paneId || !stdout.trim()) return undefined;
  try {
    const panes = JSON.parse(stdout) as PaneTabInfo[];
    return panes.find((pane) => !pane.is_plugin && String(pane.id) === paneId);
  } catch {
    return undefined;
  }
}
