#!/usr/bin/env bun
import { install, selfUpdate } from "./installer.js";
import { AgentStore, defaultStorePath, parseReportJson } from "./store.js";

export type CliIo = {
  argv: string[];
  env?: NodeJS.ProcessEnv;
  stdin?: unknown;
  stdout?: (chunk: string) => void;
  stderr?: (chunk: string) => void;
};

export async function runAgentThreads({
  argv,
  env = process.env,
  stdout = (chunk) => process.stdout.write(chunk),
  stderr = (chunk) => process.stderr.write(chunk),
}: CliIo): Promise<number> {
  try {
    const { command, flags } = parseArgs(argv);
    switch (command) {
      case "install":
        return await install({ harness: flags.harness, env });
      case "self-update":
        return await selfUpdate({ channel: flags.channel, env });
      case "help":
      case undefined:
        writeLine(stdout, usageText());
        return 0;
    }

    const store = new AgentStore(flags.db ?? env.AGENT_THREADS_DB ?? defaultStorePath(env));
    try {
      switch (command) {
        case "upsert": {
          const json = requireFlag(flags, "json");
          store.upsert(parseReportJson(json));
          break;
        }
        case "delete": {
          const agentId = requireFlag(flags, "agent-id");
          store.delete(agentId);
          break;
        }
        case "snapshot": {
          const snapshot = store.snapshot();
          if (flags.json === "true") {
            writeLine(stdout, JSON.stringify(snapshot));
          } else {
            for (const agent of snapshot.agents) {
              writeLine(stdout, `${agent.state}\t${agent.zellij_session ?? "?"}\t${agent.pane_id ?? "?"}\t${agent.title ?? agent.cwd}`);
            }
          }
          break;
        }
        case "gc": {
          const removed = store.gc();
          if (flags.json === "true") writeLine(stdout, JSON.stringify({ removed }));
          break;
        }
        default:
          throw new Error(`unknown command: ${command}`);
      }
    } finally {
      store.close();
    }
    return 0;
  } catch (error) {
    writeLine(stderr, error instanceof Error ? error.message : String(error));
    writeLine(stderr, usageText());
    return 1;
  }
}

type ParsedArgs = {
  command: string | undefined;
  flags: Record<string, string>;
};

function parseArgs(argv: string[]): ParsedArgs {
  const [command, ...rest] = argv;
  const flags: Record<string, string> = {};
  const booleanFlags = new Set(["json"]);
  for (let index = 0; index < rest.length; index += 1) {
    const arg = rest[index]!;
    if (!arg.startsWith("--")) throw new Error(`unexpected argument: ${arg}`);
    const name = arg.slice(2);
    if (booleanFlags.has(name) && (rest[index + 1] === undefined || rest[index + 1]!.startsWith("--"))) {
      flags[name] = "true";
      continue;
    }
    const value = rest[index + 1];
    if (value === undefined) throw new Error(`missing value for --${name}`);
    flags[name] = value;
    index += 1;
  }
  return { command, flags };
}

function requireFlag(flags: Record<string, string>, name: string): string {
  const value = flags[name];
  if (!value || value === "true") throw new Error(`missing --${name}`);
  return value;
}

function writeLine(write: (chunk: string) => void, text: string): void {
  write(`${text}\n`);
}

function usageText(): string {
  return `usage:
  agent-threads upsert --json '<AgentReportV2>' [--db path]
  agent-threads delete --agent-id '<id>' [--db path]
  agent-threads snapshot --json [--db path]
  agent-threads gc [--json] [--db path]
  agent-threads install [--harness pi]
  agent-threads self-update [--channel stable|prerelease]`;
}

if (import.meta.main) {
  process.exit(await runAgentThreads({ argv: process.argv.slice(2) }));
}
