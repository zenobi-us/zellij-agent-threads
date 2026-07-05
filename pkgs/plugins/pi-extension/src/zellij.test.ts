import { expect, test } from "bun:test";
import { parsePaneTabInfo } from "./zellij.js";

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
