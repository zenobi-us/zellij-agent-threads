# Troubleshooting (/troubleshooting)



# Troubleshooting [#troubleshooting]

Use this page when the plugin pane is empty, the installer prints a snippet, or a template fails.

## The panel is empty [#the-panel-is-empty]

First, make sure that Pi runs inside Zellij.

Then read the store snapshot:

```sh
agent-threads snapshot --json
```

If the snapshot contains Agents, the plugin is not reading or rendering the store.

Check that your Zellij layout contains the plugin pane:

```kdl
layout {
    pane {
        plugin location="agent-threads"
    }
}
```

Check that `~/.config/zellij/config.kdl` contains the plugin alias:

```kdl
plugins {
    agent-threads location="file:/home/you/.config/zellij/plugins/agent-threads.wasm"
}
```

If the snapshot is empty, the Pi extension did not run `agent-threads upsert` successfully.

Make sure that `agent-threads` is in the `PATH` that Pi uses.

Then run the installer again:

```sh
agent-threads install
```

Then start a new Pi process inside Zellij so the wrapper can see pane metadata.

## The installer prints a KDL snippet [#the-installer-prints-a-kdl-snippet]

This happens in a non-interactive terminal, or when the installer cannot edit the Zellij configuration.

Copy the printed `plugins` block into `~/.config/zellij/config.kdl`.

Then add `plugin location="agent-threads"` to the layout that you use.

## The Pi wrapper is missing [#the-pi-wrapper-is-missing]

The installer installs the Pi extension wrapper only when it detects Pi, or when you select Pi explicitly.

Run this command if Pi exists but detection fails:

```sh
agent-threads install --harness pi
```

The wrapper installs to this path:

```text
~/.pi/agent/extensions/pi-agenthread
```

## Agents disappear after 10 seconds [#agents-disappear-after-10-seconds]

This is normal when an Agent stops sending reports.

Each report renews a 10-second lease. The store and plugin remove stale rows after the lease expires.

If a running Agent disappears, restart Pi in that pane. Then check that the wrapper runs the CLI again:

```sh
agent-threads snapshot --json
```

## Closed panes still appear [#closed-panes-still-appear]

The plugin filters rows for panes that no longer exist.

If a stale row remains in the store, run garbage collection:

```sh
agent-threads gc --json
```

Then refresh the plugin pane or restart the Zellij session.

## A template file does not load [#a-template-file-does-not-load]

`template_file` requires Zellij `FullHdAccess` permission.

When Zellij asks for permission, grant it.

Make sure that `template` and `template_file` are not both set:

```kdl
plugin location="agent-threads" {
    template_file "/home/you/.config/zellij-agent-threads/templates/main.jinja"
}
```

Relative template paths use `${ZELLIJ_CONFIG_DIR:-$HOME/.config/zellij}`.

Includes and imports resolve from the directory that contains the entry template file.

## The template shows an error [#the-template-shows-an-error]

Start with the smallest template:

```kdl
plugin location="agent-threads" {
    template "{{ zellij_session }}: {{ agents | length }} agents"
}
```

If this works, the plugin and store are healthy. The error is in the template file.

Check field names against the [Templates reference](/templates).
