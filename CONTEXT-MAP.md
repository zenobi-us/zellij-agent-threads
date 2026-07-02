# Context map

```text
Pi extension lifecycle event
  -> JSON payload
  -> zellij pipe --name pi-agent-session
  -> Rust Zellij plugin pipe handler
  -> in-memory session map
  -> Zellij render
```

## Project map

- `pkgs/plugins/pi-extension-zellij-threads/pi-zellij-agent.ts`: Pi event publisher.
- `pkgs/plugins/zellij-plugin-agent-threads/src/main.rs`: Zellij plugin state, pipe parsing, rendering, pane cleanup.
- `pkgs/plugins/zellij-plugin-agent-threads/layouts/dev.kdl`: local Zellij dev layout.
- `apps/docs`: copied docs scaffold.
- `.moon/*`, `.prototools`, `proto/plugins/*`: boxfiles-style Moon/proto workspace.
