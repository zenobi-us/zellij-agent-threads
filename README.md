# zellij-agent-threads

Moon + proto monorepo for the Zellij agent thread plugin and Pi extension.

## Projects

- `pkgs/plugins/zellij-plugin` — Rust/WASM Zellij plugin.
- `pkgs/plugins/pi-extension` — Pi extension that publishes session state through Zellij pipes.
- `apps/docs` — docs app copied from `boxfiles/boxfiles` as the monorepo docs scaffold.

## Setup

```bash
proto install
bun install
moon query projects
```

## Common tasks

```bash
moon run zellij-plugin:build
moon run zellij-plugin:test
moon run pi-extension:typecheck
moon run docs:dev
```
