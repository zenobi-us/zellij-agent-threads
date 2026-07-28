import { expect, test } from "bun:test";
import type { ExtensionContext } from "@earendil-works/pi-coding-agent";
import type { LogService } from "./log.js";
import type { StatusWidget } from "./status.js";
import { parsePaneTabInfo, pluginPipeArgs, ZellijPublisher } from "./zellij.js";

test("publisher targets the configured plugin alias", () => {
  expect(pluginPipeArgs("payload")).toEqual([
    "pipe",
    "--plugin",
    "agent-threads",
    "--name",
    "agenthreads:agent",
    "--",
    "payload",
  ]);
});

test("parsePaneTabInfo ignores empty zellij output", () => {
  expect(parsePaneTabInfo("", "1")).toBeUndefined();
});

test("parsePaneTabInfo ignores invalid zellij output", () => {
  expect(parsePaneTabInfo("{", "1")).toBeUndefined();
});

test("parsePaneTabInfo returns matching terminal pane", () => {
  expect(parsePaneTabInfo(JSON.stringify([
    { id: 1, is_plugin: true },
    { id: 1, is_plugin: false, tab_name: "tab", title: "pi" },
  ]), "1")).toEqual({ id: 1, is_plugin: false, tab_name: "tab", title: "pi" });
});

test("publisher sends active tools using the current_tool protocol field", async () => {
  let payload = "";
  const publisher = new ZellijPublisher(
    { update() {} } as unknown as StatusWidget,
    { trace: async () => {} } as unknown as LogService,
  );
  publisher.paneTabInfo = async () => undefined;
  publisher.pipeToPlugin = async (value) => { payload = value; };
  publisher.update({ currentTool: "bash" });

  await publisher.publish({
    cwd: "/tmp/project",
    model: { id: "test-model" },
    sessionManager: { getSessionFile: () => "/tmp/session.jsonl" },
  } as ExtensionContext);

  expect(JSON.parse(payload)).toMatchObject({ current_tool: "bash" });
  expect(JSON.parse(payload)).not.toHaveProperty("current_task");

  publisher.update({ currentTool: undefined });
  await publisher.publish({
    cwd: "/tmp/project",
    model: { id: "test-model" },
    sessionManager: { getSessionFile: () => "/tmp/session.jsonl" },
  } as ExtensionContext);

  expect(JSON.parse(payload)).not.toHaveProperty("current_tool");
});
