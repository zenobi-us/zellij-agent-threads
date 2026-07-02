import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { spawn } from "node:child_process";
import { appendFile } from "node:fs/promises";
import { tmpdir } from "node:os";

const PIPE_NAME = "zellij-agent-threads";
const STATUS_KEY = "zellij-agent";
const LOG_FILE = `${tmpdir()}/pi-zellij-agent-${process.getuid?.() ?? "user"}.log`;

type AgentState = "idle" | "running" | "shutdown";
type PaneTabInfo = {
  id?: number;
  is_plugin?: boolean;
  tab_id?: number;
  tab_name?: string;
};

export default function (pi: ExtensionAPI) {
  let state: AgentState = "idle";
  let lastStatus = "init";
  let lastError: string | undefined;
  let publishCount = 0;

  async function trace(message: string) {
    const line = `${new Date().toISOString()} ${message}\n`;
    try {
      await appendFile(LOG_FILE, line);
    } catch {
      // ponytail: debug log is best-effort; footer status still carries state.
    }
  }

  function updateUi(ctx: ExtensionContext, status: string) {
    lastStatus = status;
    try {
      if (!ctx.hasUI) return;
      ctx.ui.setStatus(STATUS_KEY, `zellij ${status}`);
      ctx.ui.setWidget(STATUS_KEY, undefined);
    } catch {
      // ponytail: ctx can go stale after session replacement; Zellij publish must not crash Pi.
    }
  }

  function pipeToPlugin(payload: string) {
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

  function paneTabInfo(paneId = process.env.ZELLIJ_PANE_ID) {
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
        try {
          const panes = JSON.parse(stdout) as PaneTabInfo[];
          const ownPane = panes.find((pane) => !pane.is_plugin && String(pane.id) === paneId);
          resolve(ownPane);
        } catch {
          resolve(undefined);
        }
      });
    });
  }

  async function publish(ctx: ExtensionContext, nextState = state) {
    try {
      state = nextState;
      publishCount += 1;
      updateUi(ctx, "publishing");
      const tab = await paneTabInfo();

      const payload = JSON.stringify({
        version: 1,
        session: ctx.sessionManager.getSessionFile() ?? `${ctx.cwd}:${process.pid}`,
        cwd: ctx.cwd,
        zellij_session: process.env.ZELLIJ_SESSION_NAME,
        pane_id: process.env.ZELLIJ_PANE_ID,
        tab_id: tab?.tab_id,
        tab_name: tab?.tab_name,
        state,
        model: ctx.model?.id,
        updated_at: Date.now(),
      });

      await trace(`publish state=${state} bytes=${payload.length}`);
      await pipeToPlugin(payload);
      lastError = undefined;
      updateUi(ctx, "ok");
      await trace(`pipe ok state=${state}`);
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error);
      updateUi(ctx, "error");
      await trace(`pipe error state=${state} error=${lastError}`);
    }
  }

  pi.on("session_start", (_event, ctx) => { void publish(ctx, "idle"); });
  pi.on("agent_start", (_event, ctx) => { void publish(ctx, "running"); });
  pi.on("agent_end", (_event, ctx) => { void publish(ctx, "idle"); });
  pi.on("model_select", (_event, ctx) => { void publish(ctx); });
  pi.on("session_shutdown", (_event, ctx) => { void publish(ctx, "shutdown"); });

  pi.registerCommand("zellij-agent-publish", {
    description: "Publish this pi session to the Zellij agent plugin",
    handler: async (_args, ctx) => {
      await publish(ctx);
      try {
        if (ctx.hasUI) ctx.ui.notify(`zellij-agent ${lastStatus}; log ${LOG_FILE}`, lastError ? "warning" : "info");
      } catch {
        // UI should never break command execution.
      }
    },
  });
}
