import classNames from "classnames";
import type { PropsWithChildren } from "react";

const agents = [
  {
    name: "coder-01",
    status: "done",
    path: "pkgs/plugin",
    detail: "3m ago",
  },
  {
    name: "docs-bot",
    status: "active",
    path: "apps/docs",
    detail: "writing hero copy",
  },
  {
    name: "reviewer",
    status: "idle",
    path: "waiting for diff",
    detail: "",
  },
];

const terminalLines = [
  "$ bun test",
  "✓ packages built",
  "✓ extension typecheck",
  "",
  "$ moon run zellij-plugin-agent-threads:test",
  "running 12 tests...",
  "test render_sidebar_items ... ok",
  "test parse_session_report ... ok",
];

function AppChrome(
  props: PropsWithChildren<{
    className?: string;
    title?: string;
    buttons?: {
      close?: boolean;
      minimize?: boolean;
      maximize?: boolean;
    };
  }>,
) {
  return (
    <div className="w-full overflow-hidden rounded-md border border-rp-muted/30 bg-rp-base text-rp-text shadow-2xl shadow-rp-base/30">
      <div className="flex items-center gap-2 border-b border-rp-muted/30 bg-rp-surface px-4 py-3">
        <span className="size-3 rounded-full bg-rp-love" />
        <span className="size-3 rounded-full bg-rp-gold" />
        <span className="size-3 rounded-full bg-rp-foam" />
        <span className="ml-3 font-mono text-xs text-rp-subtle">
          {props.title}
        </span>
      </div>
      {props.children}
    </div>
  );
}

export function ZellijTerminalPreview() {
  return (
    <AppChrome title="zellij-agent-threads">
      <div className="grid min-h-80 grid-cols-[minmax(12rem,16rem)_1fr] font-mono text-sm">
        <aside className="border-r border-rp-muted/30 bg-rp-surface/70 p-4">
          <div className="space-y-3">
            {agents.map((agent) => (
              <div key={agent.name} className="rounded-lg ">
                <div className="flex items-center justify-between gap-3">
                  <span className="flex items-center gap-2 text-rp-text">
                    <span
                      className={classNames(
                        "size-2 ",
                        agent.status === "active" && "bg-rp-foam",
                        agent.status === "done" && "bg-rp-iris",
                        agent.status === "idle" && "bg-rp-muted",
                      )}
                    />
                    {agent.name}
                  </span>
                  <span className="text-xs text-rp-subtle">{agent.status}</span>
                </div>
                <div className="mt-2 ml-4 text-xs leading-5 text-rp-subtle">
                  <div>{agent.path}</div>
                  {agent.detail && <div>{agent.detail}</div>}
                </div>
              </div>
            ))}
          </div>
        </aside>

        <main className="grid grid-rows-[1fr_auto] bg-rp-base">
          <section className="h-full rounded-br-sm border border-rp-muted/20 bg-[#191724] p-4">
            <div className="space-y-2 text-rp-text">
              {terminalLines.map((line, index) => (
                <div
                  key={`${line}-${index}`}
                  className={line.startsWith("✓") ? "text-rp-foam" : ""}
                >
                  {line || "\u00a0"}
                </div>
              ))}
            </div>
          </section>
        </main>
      </div>
    </AppChrome>
  );
}
