import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { spawn } from "node:child_process";
import { appendFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { pipeline } from '@huggingface/transformers';


const PIPE_NAME = "zellij-agent-threads";
const STATUS_KEY = "zellij-agent";
const LOG_FILE = `${tmpdir()}/pi-zellij-agent-${process.getuid?.() ?? "user"}.log`;
const REFRESH_MS = 2_000;

type AgentState = "idle" | "running" | "shutdown";
type PaneTabInfo = {
  id?: number;
  is_plugin?: boolean;
  tab_id?: number;
  tab_name?: string;
};

let titleGenerator: Promise<any> | undefined;


// Local title generation is best-effort; never block Pi's agent loop on it.
async function generateSummaryTitle(sentence: string): Promise<string> {
  titleGenerator ??= pipeline('text2text-generation', 'Xenova/flan-t5-small');
  const generator = await titleGenerator;
  const input = sentence.replace(/\s+/g, " ").trim().slice(0, 1_000);
  const prompt = `Write a 4 to 8 word sentence title: ${input}`;

  const response = await generator(prompt, {
    max_new_tokens: 16,
    temperature: 0.2,
    do_sample: false
  });

  return cleanTitle(response[0]?.generated_text || input || "Untitled Group");
}

// Helper to clean up the output formatting
function cleanTitle(text: string): string {
  return text
    .trim()
    .replace(/^[.,\/#!$%\^&\*;:{}=\-_`~()]+|[.,\/#!$%\^&\*;:{}=\-_`~()]+$/g, "")
    .split(/\s+/)
    .slice(0, 8)
    .join(" ")
    .replace(/\b\w/g, char => char.toUpperCase());
}

function textFromMessage(message: unknown): string | undefined {
  const content = (message as { content?: unknown })?.content;
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return undefined;
  return content
    .map((block) => {
      if (typeof block !== "object" || block === null) return "";
      if ((block as { type?: unknown }).type !== "text") return "";
      return String((block as { text?: unknown }).text ?? "");
    })
    .filter(Boolean)
    .join("\n");
}

function lastUserOrAssistantText(messages: unknown[]): string | undefined {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    const role = (message as { role?: unknown })?.role;
    if (role !== "user" && role !== "assistant") continue;
    const text = textFromMessage(message);
    if (text) return text;
  }
}

export default function (pi: ExtensionAPI) {
  let state: AgentState = "idle";
  let lastStatus = "init";
  let lastError: string | undefined;
  let publishCount = 0;
  let refreshTimer: ReturnType<typeof setTimeout> | undefined;
  let title: string | undefined;
  let currentTask: string | undefined;

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


  function sessionKey(ctx: ExtensionContext) {
    const paneId = process.env.ZELLIJ_PANE_ID;
    if (paneId) return `${process.env.ZELLIJ_SESSION_NAME ?? "zellij"}:${paneId}`;
    return ctx.sessionManager.getSessionFile() ?? `${ctx.cwd}:${process.pid}`;
  }

  async function publish(ctx: ExtensionContext, nextState = state, updateStatus = true) {
    try {
      state = nextState;
      publishCount += 1;
      if (updateStatus) updateUi(ctx, "publishing");
      const tab = await paneTabInfo();

      const payload = JSON.stringify({
        version: 1,
        session: sessionKey(ctx),
        cwd: ctx.cwd,
        zellij_session: process.env.ZELLIJ_SESSION_NAME,
        pane_id: process.env.ZELLIJ_PANE_ID,
        tab_id: tab?.tab_id,
        tab_name: tab?.tab_name,
        state,
        model: ctx.model?.id,
        title,
        current_task: currentTask,
        updated_at: Date.now(),
      });

      await trace(`publish state=${state} bytes=${payload.length}`);
      await pipeToPlugin(payload);
      lastError = undefined;
      if (updateStatus) updateUi(ctx, "ok");
      await trace(`pipe ok state=${state}`);
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error);
      if (updateStatus) updateUi(ctx, "error");
      await trace(`pipe error state=${state} error=${lastError}`);
    }
  }

  function stopRefresh() {
    if (!refreshTimer) return;
    clearTimeout(refreshTimer);
    refreshTimer = undefined;
  }

  function scheduleRefresh(ctx: ExtensionContext) {
    stopRefresh();
    refreshTimer = setTimeout(() => {
      void publish(ctx, state, false).finally(() => {
        updateUi(ctx, "refreshing");
        if (state !== "shutdown") scheduleRefresh(ctx);
      });
    }, REFRESH_MS);
  }

  pi.on("session_start", (_event, ctx) => {
    title = undefined;
    currentTask = undefined;
    scheduleRefresh(ctx);
    void publish(ctx, "idle");
  });
  pi.on("before_agent_start", (event, ctx) => {
    const prompt = event.prompt;
    const isFirstPrompt = title === undefined;
    title ??= cleanTitle(prompt || ctx.cwd);
    currentTask = cleanTitle(prompt || title);
    void publish(ctx, "running");
    void generateSummaryTitle(prompt).then((summary) => {
      if (isFirstPrompt) title = summary;
      currentTask = summary;
      void publish(ctx, "running");
    }).catch((error) => { void trace(`title error ${error instanceof Error ? error.message : String(error)}`); });
  });
  pi.on("agent_start", (_event, ctx) => { void publish(ctx, "running"); });
  pi.on("agent_end", (event, ctx) => {
    const text = lastUserOrAssistantText(event.messages) ?? currentTask ?? title ?? ctx.cwd;
    currentTask = cleanTitle(text);
    void publish(ctx, "idle");
    void generateSummaryTitle(text).then((summary) => {
      currentTask = summary;
      void publish(ctx, "idle");
    }).catch((error) => { void trace(`title error ${error instanceof Error ? error.message : String(error)}`); });
  });
  pi.on("model_select", (_event, ctx) => { void publish(ctx); });
  pi.on("session_shutdown", (_event, ctx) => {
    stopRefresh();
    void publish(ctx, "shutdown");
  });

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

