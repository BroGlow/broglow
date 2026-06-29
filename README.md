# clawlight

A lightweight TUI dashboard and macOS menu bar indicator for monitoring
[Claude Code](https://claude.ai/code) sessions in real time.

## Features

- **TUI dashboard** — terminal UI showing all active Claude Code sessions
  with live status updates (active, inactive, needs help)
- **macOS menu bar icon** — native menu bar daemon showing a pixel art
  Clawd icon that changes color based on aggregate session health;
  auto-starts at login via launchd
- **Auto-naming** — sessions are automatically named based on the first
  prompt using the Claude CLI
- **File watching** — state updates in real time as sessions start, stop,
  or request help

## Install

### Homebrew

```bash
brew install clawlight/tap/clawlight
```

### From GitHub Releases

Download a binary from [Releases](https://github.com/clawlight/clawlight-cli/releases)
(replace `VERSION`, e.g. `0.6.0`):

```bash
# Apple Silicon
curl -L https://github.com/clawlight/clawlight-cli/releases/download/vVERSION/clawlight-vVERSION-aarch64-apple-darwin.tar.gz | tar xz
sudo mv clawlight /usr/local/bin/

# Intel Mac
curl -L https://github.com/clawlight/clawlight-cli/releases/download/vVERSION/clawlight-vVERSION-x86_64-apple-darwin.tar.gz | tar xz
sudo mv clawlight /usr/local/bin/
```

### From Source

```bash
git clone https://github.com/clawlight/clawlight-cli.git
cd clawlight-cli
cargo install --path .
```

## Quick Start

```bash
# Install hooks into Claude Code
clawlight install

# Launch the TUI dashboard
clawlight
```

`clawlight install` writes a hook script to `~/.claude/clawlight/hook.sh`
and registers it in `~/.claude/settings.json` for the `SessionStart`,
`Stop`, `Notification`, and `SessionEnd` events. After that, every Claude
Code session automatically reports its status to
`~/.claude/clawlight/state.json`, which the TUI watches in real time.

## macOS Menu Bar

`clawlight install` sets up a native macOS menu bar daemon as a launchd
LaunchAgent. It starts automatically at login and stays in your menu
bar with a color-coded Clawd icon.

| Icon   | Meaning                           |
|--------|-----------------------------------|
| Green  | All sessions actively working     |
| Yellow | At least one session is inactive  |
| Red    | At least one session needs help   |
| Gray   | No live sessions                  |

Clicking the icon shows session counts, a list of live sessions, and
an "Open clawlight" entry that launches the TUI in a new Terminal window.

Logs are at `~/.claude/clawlight/menubar.{log,err}`. The LaunchAgent
plist lives at `~/Library/LaunchAgents/io.roush.clawlight.menubar.plist`.

## Usage

```
clawlight              Launch the TUI dashboard
clawlight install      Install hooks and start the menu bar daemon
clawlight uninstall    Remove hooks, unload the menu bar daemon, clean up
clawlight menubar      Run the menu bar daemon in the foreground (debugging)
clawlight led          Mirror session state to an ESP32 in the foreground (debugging)
```

Inside the dashboard:

```
q:quit   j/k:nav   r:reload   x:clear   l:toggle ESP32 LEDs
```

## ESP32 status LEDs (optional)

If you have an ESP32 status board, clawlight can mirror the aggregate
session state to it over USB serial — red/yellow/green LEDs that match
the menu bar icon. The board firmware lives in a separate repository,
[clawlight-firmware](https://github.com/clawlight/clawlight-firmware).

**Setup:** plug in the board, open `clawlight`, and press **`l`**. That's
it — the menu bar daemon drives the LEDs from then on, automatically
reconnecting when you replug the board and surviving reboots. Press `l`
again to turn it off.

LEDs are **off by default**; until you enable them, clawlight never opens
a serial port, so there's nothing to configure if you don't have the
board. The setting lives in `~/.claude/clawlight/config.json`.

For debugging you can still run the driver in the foreground with
`clawlight led` (use `--port /dev/cu.usbmodemXXXX` to pin a device).

## Uninstall

```bash
clawlight uninstall
```

This unloads the LaunchAgent, removes its plist, removes the hook
script, and clears entries from `~/.claude/settings.json`.

## License

MIT
