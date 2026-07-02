# Context map

```text
Pi extension lifecycle event
  -> JSON payload
  -> zellij pipe --name zellij-agent-threads
  -> Rust Zellij plugin pipe handler
  -> in-memory session map
  -> Zellij render
```

## Project map

- `pkgs/plugins/pi-extension/pi-zellij-agent.ts`: Pi event publisher.
- `pkgs/plugins/zellij-plugin/src/main.rs`: Zellij plugin state, pipe parsing, rendering, pane cleanup.
- `pkgs/plugins/zellij-plugin/moon.yml`: build/dev task that starts or reloads one plugin instance.
- `apps/docs`: copied docs scaffold.
- `.moon/*`, `.prototools`, `proto/plugins/*`: boxfiles-style Moon/proto workspace.
