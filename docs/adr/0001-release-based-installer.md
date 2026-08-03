# Use release assets for installs

`agent-threads install` installs released artifacts, not the current checkout. The running CLI version selects the Zellij plugin and harness extension assets, so the CLI, store protocol, plugin, and extension stay aligned.

## Consequences

- The CLI is a standalone Bun binary built with Moon, following the Boxfiles release pattern.
- Release assets are separate files: platform CLI binaries, `agent-threads.wasm`, and `pi-agenthread.tar.gz`.
- The embedded harness manifest is declarative. TypeScript code owns detection and installation behavior.
- The first supported harness is Pi. Unsupported harnesses get the generic report contract and a docs URL.
- `agent-threads self-update` updates only the CLI in `~/.local/bin` and supports release channels.
