import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import { createInterface } from "node:readline/promises";
import { Document, Node, format } from "@bgotink/kdl";
import { parseCompat } from "@bgotink/kdl/v1-compat";

const REPO = "zenobi-us/zellij-agent-threads";
const CLI_NAME = "agent-threads";
const PLUGIN_ASSET = "agent-threads.wasm";
const PI_ASSET = "pi-agenthread.tar.gz";
const DOCS_URL = "https://github.com/zenobi-us/zellij-agent-threads/blob/main/docs/harness-integration.md";

type Fetch = (input: string | URL | Request, init?: RequestInit) => Promise<Response>;

type GithubReleaseAsset = {
  name?: string;
  browser_download_url?: string;
};

export type InstallOptions = {
  harness?: string;
  yes?: boolean;
  noReload?: boolean;
  env?: NodeJS.ProcessEnv;
};

export type SelfUpdateOptions = {
  channel?: string;
  env?: NodeJS.ProcessEnv;
};

type HarnessManifest = {
  name: string;
  assetName: string;
  installPath: string;
  detection: { homePath: string; command: string };
  docsUrl: string;
};

const HARNESSES: HarnessManifest[] = [
  {
    name: "pi",
    assetName: PI_ASSET,
    installPath: ".pi/agent/extensions/pi-agenthread",
    detection: { homePath: ".pi/agent", command: "pi" },
    docsUrl: DOCS_URL,
  },
];

export async function install(options: InstallOptions = {}): Promise<number> {
  const env = options.env ?? process.env;
  const version = cliVersion(env);
  const tag = `agent-threads-v${version}`;
  const completed: string[] = [];
  const warnings: string[] = [];
  const nextSteps: string[] = [];

  console.log(`Plan:`);
  console.log(`- release: ${tag}`);
  console.log(`- zellij plugin: ${zellijPluginPath(env)}`);
  console.log(`- harnesses: ${options.harness ?? "all supported"}`);

  const pluginAsset = await getReleaseAsset(PLUGIN_ASSET, tag, env);
  installFile(pluginAsset, zellijPluginPath(env));
  completed.push(`installed Zellij plugin to ${zellijPluginPath(env)}`);

  const wanted = options.harness ? [options.harness] : HARNESSES.map((harness) => harness.name);
  for (const name of wanted) {
    const harness = HARNESSES.find((item) => item.name === name);
    if (!harness) {
      warnings.push(`unsupported harness: ${name}`);
      nextSteps.push(integrationContract(name));
      continue;
    }
    if (!harnessDetected(harness, env)) {
      warnings.push(`${harness.name} not detected; skipped ${harness.name} extension`);
      nextSteps.push(`${harness.name} integration docs: ${harness.docsUrl}`);
      continue;
    }
    const asset = await getReleaseAsset(harness.assetName, tag, env);
    const dir = installDir(harness, env);
    installTarGz(asset, dir);
    completed.push(`installed ${harness.name} extension to ${dir}`);
  }

  await handleZellijConfig(env, options.yes === true, completed, warnings, nextSteps);
  if (!options.noReload) reloadZellijPlugin(env, warnings);

  printSummary(completed, warnings, nextSteps);
  return 0;
}

export async function selfUpdate(options: SelfUpdateOptions = {}): Promise<number> {
  const env = options.env ?? process.env;
  const channel = normalizeChannel(options.channel ?? "stable");
  const tag = await resolveChannelTag(channel, env);
  const assetName = cliAssetName(process.platform, process.arch);
  const asset = await getReleaseAsset(assetName, tag, env);
  const dest = join(home(env), ".local", "bin", CLI_NAME);

  console.log(`Plan:`);
  console.log(`- channel: ${channel}`);
  console.log(`- release: ${tag}`);
  console.log(`- CLI: ${dest}`);

  installFile(asset, dest, 0o755);
  printSummary([`installed CLI to ${dest}`], [], []);
  return 0;
}

export function cliAssetName(platform: NodeJS.Platform, arch: string): string {
  const extension = platform === "win32" ? ".exe" : "";
  const releasePlatform = platform === "win32" ? "windows" : platform;
  return `${CLI_NAME}-${releasePlatform}-${arch}${extension}`;
}

function cliVersion(env: NodeJS.ProcessEnv): string {
  if (env.AGENT_THREADS_VERSION) return env.AGENT_THREADS_VERSION;
  const packagePath = new URL("../package.json", import.meta.url);
  try {
    return JSON.parse(readFileSync(packagePath, "utf8")).version ?? "0.0.0";
  } catch {
    return "0.0.0";
  }
}

async function getReleaseAsset(assetName: string, tag: string, env: NodeJS.ProcessEnv): Promise<string> {
  const releaseDir = env.AGENT_THREADS_RELEASE_DIR;
  if (releaseDir) {
    if (!existsSync(releaseDir)) throw new Error(`missing release ${tag}: ${releaseDir}`);
    const path = join(releaseDir, assetName);
    if (!existsSync(path)) throw new Error(`missing release asset ${assetName} in ${tag}: ${path}`);
    return path;
  }

  const url = await releaseAssetUrl(assetName, tag, fetch);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`failed to download release asset ${assetName} from ${tag}: ${response.status}`);
  const dir = mkdtempSync(join(tmpdir(), "agent-threads-release-"));
  const tempPath = join(dir, `${assetName}.tmp`);
  const path = join(dir, assetName);
  writeFileSync(tempPath, Buffer.from(await response.arrayBuffer()));
  renameSync(tempPath, path);
  return path;
}

export async function releaseAssetUrl(assetName: string, tag: string, fetchImpl: Fetch = fetch): Promise<string> {
  const response = await fetchImpl(`https://api.github.com/repos/${REPO}/releases/tags/${tag}`, {
    headers: { accept: "application/vnd.github+json" },
  });
  if (response.status === 404) throw new Error(`missing release ${tag} for ${REPO}`);
  if (!response.ok) throw new Error(`failed to resolve release ${tag}: ${response.status}`);

  const body = (await response.json()) as { assets?: GithubReleaseAsset[] };
  const asset = body.assets?.find((item) => item.name === assetName);
  if (!asset) throw new Error(`missing release asset ${assetName} in ${tag}`);
  if (!asset.browser_download_url) throw new Error(`release asset ${assetName} in ${tag} has no download URL`);
  return asset.browser_download_url;
}

function installFile(source: string, dest: string, mode?: number): void {
  mkdirSync(dirname(dest), { recursive: true });
  const dir = mkdtempSync(join(dirname(dest), `.${basename(dest)}-`));
  const temp = join(dir, basename(dest));
  try {
    copyFileSync(source, temp);
    if (mode !== undefined) chmodSync(temp, mode);
    renameSync(temp, dest);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function installTarGz(archive: string, dest: string): void {
  mkdirSync(dirname(dest), { recursive: true });
  const temp = mkdtempSync(join(dirname(dest), `.${basename(dest)}-`));
  try {
    extractTarGz(archive, temp);
    replacePath(temp, dest);
  } catch (error) {
    rmSync(temp, { recursive: true, force: true });
    throw error;
  }
}

function replacePath(source: string, dest: string): void {
  const backup = existsSync(dest) ? mkdtempSync(join(dirname(dest), `.${basename(dest)}-old-`)) : undefined;
  if (backup) {
    rmSync(backup, { recursive: true, force: true });
    renameSync(dest, backup);
  }
  try {
    renameSync(source, dest);
  } catch (error) {
    if (backup && !existsSync(dest)) renameSync(backup, dest);
    throw error;
  }
  if (backup) rmSync(backup, { recursive: true, force: true });
}

function extractTarGz(archive: string, dest: string): void {
  const result = spawnSync("tar", ["-xzf", archive, "-C", dest], { encoding: "utf8" });
  if (result.status !== 0) throw new Error(`failed to extract ${archive}: ${result.stderr || result.stdout}`);
}

function harnessDetected(harness: HarnessManifest, env: NodeJS.ProcessEnv): boolean {
  return existsSync(join(home(env), harness.detection.homePath)) || commandExists(harness.detection.command, env);
}

function installDir(harness: HarnessManifest, env: NodeJS.ProcessEnv): string {
  return join(home(env), harness.installPath);
}

async function handleZellijConfig(
  env: NodeJS.ProcessEnv,
  yes: boolean,
  completed: string[],
  warnings: string[],
  nextSteps: string[],
): Promise<void> {
  const snippet = zellijPluginSnippet(env);
  if (!yes && !isInteractive(env)) {
    nextSteps.push(`Add this Zellij plugin alias to ${zellijConfigPath(env)}:\n${snippet}`);
    return;
  }

  if (!yes && !(await confirmConfigEdit(env))) {
    nextSteps.push(`Skipped Zellij config edit. Add this alias to ${zellijConfigPath(env)}:\n${snippet}`);
    return;
  }

  const configPath = zellijConfigPath(env);
  mkdirSync(dirname(configPath), { recursive: true });
  const oldConfig = existsSync(configPath) ? readFileSync(configPath, "utf8") : "";

  let newConfig: string;
  try {
    newConfig = upsertZellijPluginAlias(oldConfig, zellijPluginPath(env));
  } catch (error) {
    warnings.push(`Zellij config was not edited because KDL parsing failed: ${error instanceof Error ? error.message : String(error)}`);
    nextSteps.push(`Add this Zellij plugin alias to ${configPath}:\n${snippet}`);
    return;
  }

  if (newConfig === oldConfig) {
    completed.push(`Zellij config already references agent-threads`);
    return;
  }

  if (oldConfig) copyFileSync(configPath, `${configPath}.bak`);
  writeFileSync(configPath, newConfig);
  completed.push(`updated Zellij config at ${configPath}`);
  if (oldConfig) completed.push(`backed up Zellij config to ${configPath}.bak`);
}

function upsertZellijPluginAlias(config: string, pluginPath: string): string {
  const document = parseCompat(config);
  let plugins = document.findNodeByName("plugins");
  if (!plugins) {
    plugins = Node.create("plugins");
    plugins.children = new Document();
    document.appendNode(plugins);
  }

  plugins.children ??= new Document();
  let alias = plugins.children.findNodeByName("agent-threads");
  if (!alias) {
    alias = Node.create("agent-threads");
    plugins.appendNode(alias);
  }
  alias.setProperty("location", `file:${pluginPath}`);

  const updated = format(document);
  return updated.endsWith("\n") ? updated : `${updated}\n`;
}

function isInteractive(env: NodeJS.ProcessEnv): boolean {
  return env.AGENT_THREADS_NONINTERACTIVE !== "1" && (process.stdin.isTTY === true || env.AGENT_THREADS_FORCE_INTERACTIVE === "1");
}

async function confirmConfigEdit(env: NodeJS.ProcessEnv): Promise<boolean> {
  const rl = createInterface({ input: process.stdin, output: process.stdout });
  try {
    const answer = await rl.question(`Edit Zellij config at ${zellijConfigPath(env)}? [y/N] `);
    return /^(y|yes)$/i.test(answer.trim());
  } finally {
    rl.close();
  }
}

function reloadZellijPlugin(env: NodeJS.ProcessEnv, warnings: string[]): void {
  const result = spawnSync("zellij", ["action", "start-or-reload-plugin", `file:${zellijPluginPath(env)}`], { encoding: "utf8", env });
  if (result.status !== 0) warnings.push(`Zellij reload failed; files still installed${result.stderr ? `: ${result.stderr.trim()}` : ""}`);
}

function printSummary(completed: string[], warnings: string[], nextSteps: string[]): void {
  if (completed.length) console.log(`\nCompleted:\n${completed.map((item) => `- ${item}`).join("\n")}`);
  if (warnings.length) console.log(`\nWarnings:\n${warnings.map((item) => `- ${item}`).join("\n")}`);
  if (nextSteps.length) console.log(`\nManual next steps:\n${nextSteps.map((item) => `- ${item}`).join("\n")}`);
}

function normalizeChannel(channel: string): "stable" | "prerelease" {
  if (channel === "latest") return "stable";
  if (channel === "next") return "prerelease";
  if (channel === "stable" || channel === "prerelease") return channel;
  throw new Error(`unsupported channel: ${channel}`);
}

async function resolveChannelTag(channel: "stable" | "prerelease", env: NodeJS.ProcessEnv): Promise<string> {
  if (env.AGENT_THREADS_RELEASE_TAG) return env.AGENT_THREADS_RELEASE_TAG;
  if (env.AGENT_THREADS_RELEASE_DIR) return `agent-threads-v${cliVersion(env)}`;

  const response = await fetch(`https://api.github.com/repos/${REPO}/releases`, { headers: { accept: "application/vnd.github+json" } });
  if (!response.ok) throw new Error(`failed to resolve ${channel} release: ${response.status}`);
  const body = await response.json();
  const release = body.find((item: { prerelease?: boolean; tag_name?: string }) => {
    if (!item.tag_name?.startsWith("agent-threads-v")) return false;
    return channel === "prerelease" ? item.prerelease : !item.prerelease;
  });
  if (!release?.tag_name) throw new Error(`no ${channel} release found for agent-threads`);
  return release.tag_name;
}

function zellijPluginSnippet(env: NodeJS.ProcessEnv): string {
  return `plugins {\n    agent-threads location=\"file:${zellijPluginPath(env)}\"\n}`;
}

function zellijPluginPath(env: NodeJS.ProcessEnv): string {
  return join(configHome(env), "zellij", "plugins", "agent-threads.wasm");
}

function zellijConfigPath(env: NodeJS.ProcessEnv): string {
  return join(configHome(env), "zellij", "config.kdl");
}

function configHome(env: NodeJS.ProcessEnv): string {
  return env.XDG_CONFIG_HOME || join(home(env), ".config");
}

function home(env: NodeJS.ProcessEnv): string {
  return env.HOME || homedir();
}

function integrationContract(name: string): string {
  return `${name} can integrate by calling:\n  ${CLI_NAME} upsert --json '<AgentReportV2>'\n  ${CLI_NAME} delete --agent-id '<id>'\n  ${CLI_NAME} snapshot --json\nDocs: ${DOCS_URL}`;
}

function commandExists(command: string, env: NodeJS.ProcessEnv): boolean {
  return spawnSync("sh", ["-c", `command -v ${command}`], { stdio: "ignore", env }).status === 0;
}
