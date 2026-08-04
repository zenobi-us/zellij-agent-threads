import { afterEach, expect, test } from "bun:test";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { cliAssetName, releaseAssetUrl, resolveChannelTag } from "./installer.js";
import { runAgentThreads } from "./index.js";
import type { AgentReportV2 } from "./store.js";

const dirs: string[] = [];
const cli = join(import.meta.dir, "index.ts");

afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

test("CLI seam runs commands with controlled argv, env, and output", async () => {
  const db = tempDb();
  const env = { AGENT_THREADS_DB: db };
  const stdout: string[] = [];
  const stderr: string[] = [];
  const io = { env, stdout: stdout.push.bind(stdout), stderr: stderr.push.bind(stderr), stdin: "" };

  expect(await runAgentThreads({ argv: ["upsert", "--json", JSON.stringify(report("a"))], ...io })).toBe(0);
  expect(await runAgentThreads({ argv: ["snapshot", "--json"], ...io })).toBe(0);
  expect(await runAgentThreads({ argv: ["delete", "--agent-id", "a"], ...io })).toBe(0);
  expect(await runAgentThreads({ argv: ["gc", "--json"], ...io })).toBe(0);
  expect(await runAgentThreads({ argv: ["snapshot", "--json"], ...io })).toBe(0);
  expect(stderr.join("")).toBe("");

  const lines = stdout.join("").trim().split("\n");
  expect((JSON.parse(lines[0]!) as { agents: AgentReportV2[] }).agents.map((agent) => agent.agent_id)).toEqual(["a"]);
  expect(JSON.parse(lines[1]!)).toEqual({ removed: 0 });
  expect((JSON.parse(lines[2]!) as { agents: AgentReportV2[] }).agents).toEqual([]);
});

test("CLI seam reports errors without exiting the test process", async () => {
  const stderr: string[] = [];

  expect(await runAgentThreads({ argv: ["nope"], stdout: () => {}, stderr: stderr.push.bind(stderr) })).toBe(1);
  expect(stderr.join("")).toContain("unknown command: nope");
  expect(stderr.join("")).toContain("agent-threads upsert");
});

test("install copies same-version plugin and detected Pi extension", () => {
  const { home, config, release } = fixture({ pi: true, version: "9.8.7" });

  const result = runCli(["install"], home, config, release, "9.8.7");

  expect(result.status).toBe(0);
  expect(result.stdout).toContain("release: agent-threads-v9.8.7");
  expect(result.stdout).toContain(`zellij config: ${join(config, "zellij", "config.kdl")}`);
  expect(result.stdout).toContain(`zellij config backup: ${join(config, "zellij", "config.kdl")}.bak`);
  expect(result.stdout).toContain(`pi extension: ${join(home, ".pi", "agent", "extensions", "pi-agenthread")}`);
  expect(readFileSync(join(config, "zellij", "plugins", "agent-threads.wasm"), "utf8")).toBe("wasm");
  expect(existsSync(join(home, ".pi", "agent", "extensions", "pi-agenthread", "package.json"))).toBe(true);
  expect(result.stdout).toContain("Add this Zellij plugin alias");
});

test("plain install completes plugin, supported harnesses, reload, and leaves CLI alone", () => {
  const { home, config, release } = fixture({ pi: true });
  const bin = join(home, "bin");
  mkdirSync(bin, { recursive: true });
  writeFileSync(join(bin, "zellij"), "#!/usr/bin/env sh\nexit 0\n");
  chmodSync(join(bin, "zellij"), 0o755);
  writeFileSync(join(bin, "agent-threads"), "old-cli");

  const result = runCli(["install"], home, config, release, "0.0.1", bin);

  expect(result.status).toBe(0);
  expect(readFileSync(join(config, "zellij", "plugins", "agent-threads.wasm"), "utf8")).toBe("wasm");
  expect(existsSync(join(home, ".pi", "agent", "extensions", "pi-agenthread", "package.json"))).toBe(true);
  expect(readFileSync(join(bin, "agent-threads"), "utf8")).toBe("old-cli");
  expect(result.stdout).toContain("Completed:");
  expect(result.stdout).toContain("installed Zellij plugin");
  expect(result.stdout).toContain("installed pi extension");
  expect(result.stdout).toContain("reloaded Zellij plugin");
  expect(result.stdout).toContain("Manual next steps:");
  expect(result.stdout).not.toContain("Warnings:");
});

test("install detects Pi from the pi command on PATH", () => {
  const { home, config, release } = fixture({ pi: false });
  const bin = join(home, "bin");
  mkdirSync(bin, { recursive: true });
  writeFileSync(join(bin, "pi"), "#!/usr/bin/env sh\nexit 0\n");
  chmodSync(join(bin, "pi"), 0o755);

  const result = runCli(["install"], home, config, release, "0.0.1", bin);

  expect(result.status).toBe(0);
  expect(existsSync(join(home, ".pi", "agent", "extensions", "pi-agenthread", "package.json"))).toBe(true);
});

test("install replaces an existing Zellij plugin file", () => {
  const { home, config, release } = fixture({ pi: false });
  const pluginPath = join(config, "zellij", "plugins", "agent-threads.wasm");
  mkdirSync(join(config, "zellij", "plugins"), { recursive: true });
  writeFileSync(pluginPath, "old");

  const result = runCli(["install"], home, config, release);

  expect(result.status).toBe(0);
  expect(readFileSync(pluginPath, "utf8")).toBe("wasm");
});

test("non-interactive install prints the Zellij snippet without editing config", () => {
  const { home, config, release } = fixture({ pi: false });
  const configPath = join(config, "zellij", "config.kdl");
  const oldConfig = "plugins {\n    compact-bar location=\"zellij:compact-bar\"\n}\n";
  mkdirSync(join(config, "zellij"), { recursive: true });
  writeFileSync(configPath, oldConfig);

  const result = runCli(["install"], home, config, release);

  expect(result.status).toBe(0);
  expect(readFileSync(configPath, "utf8")).toBe(oldConfig);
  expect(result.stdout).toContain("Add this Zellij plugin alias");
  expect(result.stdout).toContain(`agent-threads location=\"file:${join(config, "zellij", "plugins", "agent-threads.wasm")}\"`);
  expect(result.stdout).not.toContain("Edit Zellij config");
});

test("install skips Pi extension when Pi is not detected", () => {
  const { home, config, release } = fixture({ pi: false });

  const result = runCli(["install"], home, config, release);

  expect(result.status).toBe(0);
  expect(existsSync(join(config, "zellij", "plugins", "agent-threads.wasm"))).toBe(true);
  expect(existsSync(join(home, ".pi", "agent", "extensions", "pi-agenthread"))).toBe(false);
  expect(result.stdout).toContain("pi not detected; skipped pi extension");
  expect(result.stdout).toContain("pi integration docs: https://github.com/zenobi-us/zellij-agent-threads/blob/main/docs/harness-integration.md");
});

test("install prints generic contract for unsupported harness", () => {
  const { home, config, release } = fixture({ pi: false });

  const result = runCli(["install", "--harness", "ghost"], home, config, release);

  expect(result.status).toBe(0);
  expect(result.stdout).toContain("unsupported harness: ghost");
  expect(result.stdout).toContain("agent-threads upsert --json '<AgentReportV2>'");
  expect(result.stdout).toContain("agent-threads delete --agent-id '<id>'");
  expect(result.stdout).toContain("agent-threads snapshot --json");
  expect(result.stdout).toContain("docs/harness-integration.md");
});

test("interactive install edits Zellij config when accepted", () => {
  const { home, config, release } = fixture({ pi: false });

  const result = runCliInteractive(["install"], home, config, release, "y\n");

  expect(result.status).toBe(0);
  expect(result.stdout).toContain("Edit Zellij config");
  expect(readFileSync(join(config, "zellij", "config.kdl"), "utf8")).toContain("agent-threads");
});

test("interactive install leaves Zellij config unchanged when rejected", () => {
  const { home, config, release } = fixture({ pi: false });
  const configPath = join(config, "zellij", "config.kdl");
  mkdirSync(join(config, "zellij"), { recursive: true });
  writeFileSync(configPath, "plugins {\n}\n");

  const result = runCliInteractive(["install"], home, config, release, "n\n");

  expect(result.status).toBe(0);
  expect(readFileSync(configPath, "utf8")).toBe("plugins {\n}\n");
  expect(result.stdout).toContain("Skipped Zellij config edit");
});

test("config edit creates a backup before write", () => {
  const { home, config, release } = fixture({ pi: false });
  const configPath = join(config, "zellij", "config.kdl");
  const oldConfig = "plugins {\n    compact-bar location=\"zellij:compact-bar\"\n}\n";
  mkdirSync(join(config, "zellij"), { recursive: true });
  writeFileSync(configPath, oldConfig);

  const result = runCliInteractive(["install"], home, config, release, "y\n");

  expect(result.status).toBe(0);
  expect(readFileSync(`${configPath}.bak`, "utf8")).toBe(oldConfig);
  expect(readFileSync(configPath, "utf8")).toContain("agent-threads");
});

test("config edit backs up an existing empty config before write", () => {
  const { home, config, release } = fixture({ pi: false });
  const configPath = join(config, "zellij", "config.kdl");
  mkdirSync(join(config, "zellij"), { recursive: true });
  writeFileSync(configPath, "");

  const result = runCliInteractive(["install"], home, config, release, "y\n");

  expect(result.status).toBe(0);
  expect(readFileSync(`${configPath}.bak`, "utf8")).toBe("");
  expect(readFileSync(configPath, "utf8")).toContain("agent-threads");
});

test("config edit mutates an existing plugins KDL node", () => {
  const { home, config, release } = fixture({ pi: false });
  const configPath = join(config, "zellij", "config.kdl");
  mkdirSync(join(config, "zellij"), { recursive: true });
  writeFileSync(configPath, "plugins {\n    compact-bar location=\"zellij:compact-bar\"\n}\n");

  const result = runCliInteractive(["install"], home, config, release, "y\n");
  const updated = readFileSync(configPath, "utf8");

  expect(result.status).toBe(0);
  expect(updated).toContain("compact-bar");
  expect(updated).toContain("agent-threads");
  expect(updated).toContain(`file:${join(config, "zellij", "plugins", "agent-threads.wasm")}`);
  expect(result.stdout).not.toContain("conservative append");
});

test("plain install warning still reports successful installed files and manual steps", () => {
  const { home, config, release } = fixture({ pi: true });

  const result = runCli(["install"], home, config, release);

  expect(result.status).toBe(0);
  expect(readFileSync(join(config, "zellij", "plugins", "agent-threads.wasm"), "utf8")).toBe("wasm");
  expect(existsSync(join(home, ".pi", "agent", "extensions", "pi-agenthread", "package.json"))).toBe(true);
  expect(result.stdout).toContain("Completed:");
  expect(result.stdout).toContain("installed Zellij plugin");
  expect(result.stdout).toContain("installed pi extension");
  expect(result.stdout).toContain("Warnings:");
  expect(result.stdout).toContain("Zellij reload failed; files still installed");
  expect(result.stdout).toContain("Manual next steps:");
});

test("install reports successful Zellij reload as completed work", () => {
  const { home, config, release } = fixture({ pi: false });
  const bin = join(home, "bin");
  mkdirSync(bin, { recursive: true });
  writeFileSync(join(bin, "zellij"), "#!/usr/bin/env sh\nexit 0\n");
  chmodSync(join(bin, "zellij"), 0o755);

  const result = runCli(["install"], home, config, release, "0.0.1", bin);

  expect(result.status).toBe(0);
  expect(result.stdout).toContain("reloaded Zellij plugin");
  expect(result.stdout).not.toContain("Zellij reload failed");
});

test("self-update installs selected channel CLI to local bin", () => {
  const { home, config, release } = fixture({ pi: false });
  const asset = cliAssetName(process.platform, process.arch);
  writeFileSync(join(release, asset), "binary");
  chmodSync(join(release, asset), 0o755);

  const result = runCli(["self-update", "--channel", "prerelease"], home, config, release);

  expect(result.status).toBe(0);
  expect(result.stdout).toContain("channel: prerelease");
  expect(readFileSync(join(home, ".local", "bin", "agent-threads"), "utf8")).toBe("binary");
});

test("self-update defaults to the stable channel", () => {
  const { home, config, release } = fixture({ pi: false });
  const asset = cliAssetName(process.platform, process.arch);
  writeFileSync(join(release, asset), "stable-binary");
  chmodSync(join(release, asset), 0o755);

  const result = runCli(["self-update"], home, config, release);

  expect(result.status).toBe(0);
  expect(result.stdout).toContain("channel: stable");
  expect(readFileSync(join(home, ".local", "bin", "agent-threads"), "utf8")).toBe("stable-binary");
});

test("CLI asset names match release platform assets", () => {
  expect(cliAssetName("linux", "x64")).toBe("agent-threads-linux-x64");
  expect(cliAssetName("linux", "arm64")).toBe("agent-threads-linux-arm64");
  expect(cliAssetName("darwin", "arm64")).toBe("agent-threads-darwin-arm64");
  expect(cliAssetName("win32", "x64")).toBe("agent-threads-windows-x64.exe");
});

test("stable channel selects the newest non-prerelease CLI release", async () => {
  const fetch = async () =>
    Response.json([
      { tag_name: "agent-threads-v2.0.0-beta.1", prerelease: true },
      { tag_name: "agent-threads-v1.9.0", prerelease: false },
      { tag_name: "other-v9.9.9", prerelease: false },
    ]);

  await expect(resolveChannelTag("stable", {}, fetch)).resolves.toBe("agent-threads-v1.9.0");
});

test("prerelease channel selects the newest prerelease CLI release", async () => {
  const fetch = async () =>
    Response.json([
      { tag_name: "agent-threads-v2.0.0-beta.1", prerelease: true },
      { tag_name: "agent-threads-v1.9.0", prerelease: false },
    ]);

  await expect(resolveChannelTag("prerelease", {}, fetch)).resolves.toBe("agent-threads-v2.0.0-beta.1");
});

test("release asset resolver selects an asset from the requested release", async () => {
  const fetch = async () =>
    Response.json({
      assets: [
        { name: "agent-threads-linux-x64", browser_download_url: "https://example.invalid/linux" },
        { name: "agent-threads.wasm", browser_download_url: "https://example.invalid/wasm" },
      ],
    });

  await expect(releaseAssetUrl("agent-threads.wasm", "agent-threads-v9.8.7", fetch)).resolves.toBe("https://example.invalid/wasm");
});

test("release asset resolver reports a missing release", async () => {
  const fetch = async () => new Response("not found", { status: 404 });

  await expect(releaseAssetUrl("agent-threads.wasm", "agent-threads-v9.8.7", fetch)).rejects.toThrow(
    "missing release agent-threads-v9.8.7",
  );
});

test("release asset resolver reports a missing asset", async () => {
  const fetch = async () => Response.json({ assets: [{ name: "pi-agenthread.tar.gz", browser_download_url: "https://example.invalid/pi" }] });

  await expect(releaseAssetUrl("agent-threads.wasm", "agent-threads-v9.8.7", fetch)).rejects.toThrow(
    "missing release asset agent-threads.wasm in agent-threads-v9.8.7",
  );
});

test("install reports a missing local release", () => {
  const { home, config, release } = fixture({ pi: false });
  rmSync(release, { recursive: true, force: true });

  const result = runCli(["install"], home, config, release);

  expect(result.status).toBe(1);
  expect(result.stderr).toContain("missing release agent-threads-v0.0.1");
});

test("install reports a missing local asset", () => {
  const { home, config, release } = fixture({ pi: false });
  rmSync(join(release, "agent-threads.wasm"), { force: true });

  const result = runCli(["install"], home, config, release);

  expect(result.status).toBe(1);
  expect(result.stderr).toContain("missing release asset agent-threads.wasm in agent-threads-v0.0.1");
});

test("invalid Pi archive fails before replacing existing extension", () => {
  const { home, config, release } = fixture({ pi: true });
  const extension = join(home, ".pi", "agent", "extensions", "pi-agenthread");
  mkdirSync(extension, { recursive: true });
  writeFileSync(join(extension, "package.json"), "old");
  writeArchiveWithoutPackage(release);

  const result = runCli(["install"], home, config, release);

  expect(result.status).toBe(1);
  expect(result.stderr).toContain("invalid pi extension archive: missing package.json");
  expect(readFileSync(join(extension, "package.json"), "utf8")).toBe("old");
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

function fixture(options: { pi: boolean; version?: string }) {
  const root = mkdtempSync(join(tmpdir(), "agent-threads-cli-test-"));
  dirs.push(root);
  const home = join(root, "home");
  const config = join(root, "config");
  const release = join(root, "release");
  mkdirSync(release, { recursive: true });
  mkdirSync(config, { recursive: true });
  mkdirSync(home, { recursive: true });
  writeFileSync(join(release, "agent-threads.wasm"), "wasm");
  writePiArchive(release);
  if (options.pi) mkdirSync(join(home, ".pi", "agent"), { recursive: true });
  return { home, config, release, version: options.version ?? "0.0.1" };
}

function writePiArchive(release: string): void {
  const payload = join(release, "pi-payload");
  mkdirSync(join(payload, "src"), { recursive: true });
  writeFileSync(join(payload, "package.json"), '{"name":"pi-agenthread"}\n');
  writeFileSync(join(payload, "src", "index.ts"), "export {};\n");
  const result = spawnSync("tar", ["-czf", join(release, "pi-agenthread.tar.gz"), "-C", payload, "."], { encoding: "utf8" });
  if (result.status !== 0) throw new Error(result.stderr || result.stdout);
}

function writeArchiveWithoutPackage(release: string): void {
  const payload = join(release, "bad-pi-payload");
  mkdirSync(join(payload, "src"), { recursive: true });
  writeFileSync(join(payload, "src", "index.ts"), "export {};\n");
  const result = spawnSync("tar", ["-czf", join(release, "pi-agenthread.tar.gz"), "-C", payload, "."], { encoding: "utf8" });
  if (result.status !== 0) throw new Error(result.stderr || result.stdout);
}

function runCli(args: string[], home: string, config: string, release: string, version = "0.0.1", pathPrefix?: string) {
  const path = testPath(home, pathPrefix);
  return spawnSync(process.execPath, [cli, ...args], {
    encoding: "utf8",
    env: {
      ...process.env,
      HOME: home,
      PATH: path,
      XDG_CONFIG_HOME: config,
      AGENT_THREADS_RELEASE_DIR: release,
      AGENT_THREADS_VERSION: version,
      AGENT_THREADS_NONINTERACTIVE: "1",
    },
  });
}

function runCliInteractive(args: string[], home: string, config: string, release: string, input: string, version = "0.0.1") {
  const path = testPath(home);
  return spawnSync(process.execPath, [cli, ...args], {
    encoding: "utf8",
    input,
    env: {
      ...process.env,
      HOME: home,
      PATH: path,
      XDG_CONFIG_HOME: config,
      AGENT_THREADS_RELEASE_DIR: release,
      AGENT_THREADS_VERSION: version,
      AGENT_THREADS_FORCE_INTERACTIVE: "1",
    },
  });
}

function testPath(home: string, pathPrefix?: string): string {
  const bin = join(home, ".agent-threads-test-bin");
  mkdirSync(bin, { recursive: true });
  const zellij = join(bin, "zellij");
  if (!existsSync(zellij)) {
    writeFileSync(zellij, "#!/usr/bin/env sh\nexit 1\n");
    chmodSync(zellij, 0o755);
  }
  return pathPrefix ? `${pathPrefix}:${bin}:/usr/bin:/bin` : `${bin}:/usr/bin:/bin`;
}
