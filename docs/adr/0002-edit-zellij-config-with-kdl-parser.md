# Edit Zellij config with a KDL parser

When `agent-threads install` runs in an interactive terminal, it prompts before it edits `~/.config/zellij/config.kdl`. If the user accepts, the CLI uses a maintained KDL parser instead of appending text or using a small custom parser.

## Consequences

- The installer backs up `config.kdl` before it writes changes.
- Non-interactive installs print the required KDL snippet instead of editing the file.
- The Zellij plugin reload is best-effort. A reload failure does not fail the file install.
