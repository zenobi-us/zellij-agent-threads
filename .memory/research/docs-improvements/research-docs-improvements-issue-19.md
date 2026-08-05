# Research: docs improvements for issue 19

Date: 2026-08-05

## Question

What must the documentation pass change for `zellij-agent-threads` after issue 19?

## Findings

- `apps/docs/content/quickstart.md` was copied from Boxfiles. It named Boxfiles commands and manifest files. This contradicted `README.md` and the current project glossary.
- `README.md` defines the released install path. Users download an `agent-threads-*` binary, then run `agent-threads install`.
- `README.md` says that the released installer installs `~/.config/zellij/plugins/agent-threads.wasm` and `~/.pi/agent/extensions/pi-agenthread`.
- `apps/agent-threads/src/installer.ts` builds the Zellij plugin alias as `agent-threads location="file:<path>"`.
- `apps/agent-threads/src/index.test.ts` covers install behavior, non-interactive snippet output, Pi extension install, and platform asset names.
- Official Zellij docs say plugin aliases live in the `plugins` block. A layout can load a plugin with `pane { plugin location="..." }`.
- Official Zellij docs say `zellij --layout /path/to/layout_file.kdl` applies a layout at startup or inside a running session.
- `apps/docs/src/components/ReleaseVersion.tsx` still looked for `@boxfiles/cli`. The current `apps/docs/public/releases.json` does not contain an `agent-threads` CLI package.
- `apps/docs/src/components/useReleaseVersions.tsx` still had a Boxfiles comment.
- `CONTEXT.md` defines `Agent`, `Zellij session`, `Harness`, `Released installer`, and `Self-update`. Docs must use these terms.

## Sources

- `apps/docs/content/quickstart.md`
- `README.md`
- `CONTEXT.md`
- `apps/agent-threads/src/installer.ts`
- `apps/agent-threads/src/index.test.ts`
- `apps/docs/public/releases.json`
- `apps/docs/src/components/ReleaseVersion.tsx`
- `apps/docs/src/components/useReleaseVersions.tsx`
- Zellij User Guide: Plugin Aliases, https://zellij.dev/documentation/plugin-aliases
- Zellij User Guide: Creating a Layout, https://zellij.dev/documentation/creating-a-layout.html
- Zellij User Guide: Layouts, https://zellij.dev/documentation/layouts
