# AGENTS.md

## Project

`zellij-agent-threads` is a Moon + proto monorepo for:

- `pkgs/plugins/zellij-plugin`: Rust/WASM Zellij plugin that renders Pi agent session reports.
- `pkgs/plugins/pi-extension`: Pi extension that publishes session state to the Zellij plugin.
- `apps/docs`: docs scaffold copied from `boxfiles/boxfiles`.

Also read nested `AGENTS.md` files in apps/pkgs.

## Runtime and tooling

- Use Bun for JS package management.
- Use Moon for repo tasks.
- Use proto for tool versions.
- Rust plugin builds target `wasm32-wasip1`.
- Do not use root mise tasks; they were migrated to package `moon.yml` tasks.

## Common commands

```bash
bun install
moon query projects
moon run zellij-plugin:build
moon run zellij-plugin:test
moon run pi-extension:typecheck
```

## Coding workflow

- Keep plugin protocol JSON small and backwards-compatible.
- Validate Rust behavior with host-target unit tests plus WASM `cargo check`.
- Keep Pi extension best-effort: Zellij pipe failure must not break Pi startup.

## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues for `zenobi-us/zellij-agent-threads`. See `.memory/docs/agents/issue-tracker.md`.

### Triage labels

Triage labels use the default canonical vocabulary. See `.memory/docs/agents/triage-labels.md`.

### Domain docs

Domain docs use a multi-context layout. See `.memory/docs/agents/domain.md`.

### Zellij Development

Use `zellij-plugin-dev` skill 
