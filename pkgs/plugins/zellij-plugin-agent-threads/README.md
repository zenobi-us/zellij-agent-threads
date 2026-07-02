# zellij-plugin-agent-threads

Rust/WASM Zellij plugin that lists Pi agent sessions reported through Zellij pipes.

## Build

```bash
moon run zellij-plugin-agent-threads:build
```

Output:

```text
pkgs/plugins/zellij-plugin-agent-threads/target/wasm32-wasip1/release/zellij-plugin-agent-threads.wasm
```

## Test

```bash
moon run zellij-plugin-agent-threads:test
moon run zellij-plugin-agent-threads:check
```

Host-target tests are used because raw `.wasm` test binaries do not execute directly on Linux without a WASI runner.

## Pipe name

```text
pi-agent-session
```
