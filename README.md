# zellij-agent-threads

Moon + proto monorepo for the Zellij agent thread plugin and Pi extension.

## Projects

- `pkgs/plugins/zellij-plugin-agent-threads` — Rust/WASM Zellij plugin.
- `pkgs/plugins/pi-extension-zellij-threads` — Pi extension that publishes session state through Zellij pipes.
- `apps/docs` — docs app copied from `boxfiles/boxfiles` as the monorepo docs scaffold.

## Setup

```bash
proto install
bun install
moon query projects
```

## Common tasks

```bash
moon run zellij-plugin-agent-threads:build
moon run zellij-plugin-agent-threads:test
moon run pi-extension-zellij-threads:typecheck
moon run docs:dev
```
