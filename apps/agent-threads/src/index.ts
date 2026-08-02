#!/usr/bin/env bun
import { AgentStore, defaultStorePath, parseReportJson } from "./store.js";

const args = process.argv.slice(2);

try {
  const { command, flags } = parseArgs(args);
  const store = new AgentStore(flags.db ?? process.env.AGENT_THREADS_DB ?? defaultStorePath());
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
          console.log(JSON.stringify(snapshot));
        } else {
          for (const agent of snapshot.agents) {
            console.log(`${agent.state}\t${agent.zellij_session ?? "?"}\t${agent.pane_id ?? "?"}\t${agent.title ?? agent.cwd}`);
          }
        }
        break;
      }
      case "gc": {
        const removed = store.gc();
        if (flags.json === "true") console.log(JSON.stringify({ removed }));
        break;
      }
      case "help":
      case undefined:
        usage(0);
        break;
      default:
        throw new Error(`unknown command: ${command}`);
    }
  } finally {
    store.close();
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  usage(1);
}

type ParsedArgs = {
  command: string | undefined;
  flags: Record<string, string>;
};

function parseArgs(argv: string[]): ParsedArgs {
  const [command, ...rest] = argv;
  const flags: Record<string, string> = {};
  for (let index = 0; index < rest.length; index += 1) {
    const arg = rest[index]!;
    if (!arg.startsWith("--")) throw new Error(`unexpected argument: ${arg}`);
    const name = arg.slice(2);
    if (name === "json" && (rest[index + 1] === undefined || rest[index + 1]!.startsWith("--"))) {
      flags.json = "true";
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

function usage(exitCode: number): never {
  const out = exitCode === 0 ? console.log : console.error;
  out(`usage:
  agent-threads upsert --json '<AgentReportV2>' [--db path]
  agent-threads delete --agent-id '<id>' [--db path]
  agent-threads snapshot --json [--db path]
  agent-threads gc [--json] [--db path]`);
  process.exit(exitCode);
}
