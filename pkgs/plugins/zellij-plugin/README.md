# zellij-plugin-agent-threads

Rust/WASM Zellij plugin that lists Pi agent sessions reported through Zellij pipes.

## Build

```bash
moon run zellij-plugin:build
```

Output:

```text
pkgs/plugins/zellij-plugin/target/wasm32-wasip1/release/zellij-plugin-agent-threads.wasm
```

## Test

```bash
moon run zellij-plugin:test
moon run zellij-plugin:check
```

Host-target tests are used because raw `.wasm` test binaries do not execute directly on Linux without a WASI runner.

## Pipe name

```text
zellij-agent-threads
```

## Template files

Inline `template` config still works. For `{% include %}` / `{% import %}`, load templates from disk:

```kdl
plugin location="file:/path/to/zellij-plugin-agent-threads.wasm" {
    template_dir "/home/q/.config/zellij-agent-threads/templates"
    template_name "main.j2"
}
```

`template_name` defaults to `main.j2`. Disk templates request Zellij `FullHdAccess`.