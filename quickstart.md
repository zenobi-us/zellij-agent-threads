# Quickstart (/quickstart)



# Quickstart [#quickstart]

Use this tutorial to install Zellij Agent Threads from a release.

You will install the `agent-threads` CLI, the Zellij plugin, and the Pi extension. Then you will open the plugin in a Zellij layout.

## Prerequisites [#prerequisites]

Install these tools first:

* [Zellij](https://zellij.dev/)
* [Pi](https://github.com/badlogic/pi-mono)

Make sure that `~/.local/bin` is in your `PATH`.

## 1. Download the CLI [#1-download-the-cli]

Download the release asset for your platform:

```sh
mkdir -p ~/.local/bin
curl -L https://github.com/zenobi-us/zellij-agent-threads/releases/latest/download/agent-threads-linux-x64 \
  -o ~/.local/bin/agent-threads
chmod +x ~/.local/bin/agent-threads
```

Use one of these assets:

* `agent-threads-linux-x64`
* `agent-threads-linux-arm64`
* `agent-threads-darwin-x64`
* `agent-threads-darwin-arm64`
* `agent-threads-windows-x64.exe`

## 2. Install the plugin and Pi extension [#2-install-the-plugin-and-pi-extension]

Run the released installer:

```sh
agent-threads install
```

The installer downloads assets for the same version as the CLI.

It installs these files:

* Zellij plugin: `~/.config/zellij/plugins/agent-threads.wasm`
* Pi extension: `~/.pi/agent/extensions/pi-agenthread`

The installer can edit `~/.config/zellij/config.kdl` in an interactive terminal. It creates `~/.config/zellij/config.kdl.bak` before it writes changes.

If you want the installer to edit without a prompt, run this command:

```sh
agent-threads install --yes
```

If the terminal is not interactive, the installer prints the Zellij snippet instead.

## 3. Add the Zellij plugin alias [#3-add-the-zellij-plugin-alias]

If the installer did not edit your Zellij configuration, add this alias to `~/.config/zellij/config.kdl`:

```kdl
plugins {
    agent-threads location="file:/home/you/.config/zellij/plugins/agent-threads.wasm"
}
```

Replace `/home/you` with your home directory.

## 4. Add the plugin pane to a layout [#4-add-the-plugin-pane-to-a-layout]

Create a layout file, for example `~/.config/zellij/layouts/agent-threads.kdl`:

```kdl
layout {
    pane {
        plugin location="agent-threads"
    }
}
```

Start Zellij with that layout:

```sh
zellij --layout ~/.config/zellij/layouts/agent-threads.kdl
```

## 5. Start Pi [#5-start-pi]

Start Pi in any Zellij pane.

The Pi extension listens to Pi events. It runs `agent-threads upsert` and `agent-threads delete` for you.

The `agent-threads` CLI owns the store. The extension is only the Pi wrapper.

The plugin panel shows each active Agent with its state, pane, model, title, worktree, and current tool.

## 6. Check the result [#6-check-the-result]

Look for the Agent row in the plugin panel.

If the panel is empty, read the store snapshot:

```sh
agent-threads snapshot --json
```

If the snapshot is empty, make sure that Pi runs inside Zellij and that the Pi wrapper is installed.

## Contributor install [#contributor-install]

Source installs are for contributors.

Use the source install flow in the [repository README](https://github.com/zenobi-us/zellij-agent-threads#contributor-source-install).
