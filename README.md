# clawlight

A lightweight TUI dashboard and menu bar / system tray indicator for
monitoring [Claude Code](https://claude.ai/code) sessions in real time.
Runs on macOS, Windows, and Linux.

## Features

- **TUI dashboard** — terminal UI showing all active Claude Code sessions
  with live status updates (active, inactive, needs help)
- **Menu bar / system tray icon** — native tray daemon showing a pixel art
  Clawd icon that changes color based on aggregate session health;
  auto-starts at login (launchd on macOS, a registry Run key on Windows, an
  XDG autostart entry on Linux)
- **Auto-naming** — sessions are automatically named based on the first
  prompt using the Claude CLI
- **File watching** — state updates in real time as sessions start, stop,
  or request help
- **Cross-platform** — a single native binary; the hook backend is built
  in (no `bash`/`jq` dependency)

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

### Windows

Download the `x86_64-pc-windows-msvc` archive from
[Releases](https://github.com/clawlight/clawlight-cli/releases), extract
`clawlight.exe`, and put it somewhere on your `PATH` (PowerShell):

```powershell
# From the folder containing the downloaded archive (replace VERSION):
tar -xf clawlight-vVERSION-x86_64-pc-windows-msvc.tar.gz
$dest = "$env:LOCALAPPDATA\Programs\clawlight"
New-Item -ItemType Directory -Force $dest | Out-Null
Move-Item clawlight.exe $dest -Force
# Add $dest to your user PATH (one-time):
[Environment]::SetEnvironmentVariable("Path", "$([Environment]::GetEnvironmentVariable('Path','User'));$dest", "User")
```

Open a new terminal afterward so the updated `PATH` takes effect.

### From Source

Requires the [Rust toolchain](https://rustup.rs). On Windows this uses the
MSVC toolchain (install the "Desktop development with C++" / Build Tools
workload so the linker is available).

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

`clawlight install` registers the built-in hook backend (`clawlight hook`)
in `~/.claude/settings.json` for the `SessionStart`, `UserPromptSubmit`,
`Stop`, `Notification`, `SessionEnd`, and `PreToolUse` events. After that,
every Claude Code session automatically reports its status to
`~/.claude/clawlight/state.json` (`%USERPROFILE%\.claude\clawlight\state.json`
on Windows), which the TUI watches in real time. The hook is the clawlight
binary itself — there's no separate shell script and no `jq` dependency.

## Menu bar / system tray

`clawlight install` also sets up the tray daemon to start at login:

- **macOS** — a launchd LaunchAgent. The plist lives at
  `~/Library/LaunchAgents/io.roush.clawlight.menubar.plist`; logs are at
  `~/.claude/clawlight/menubar.{log,err}`.
- **Windows** — an `HKCU\…\CurrentVersion\Run` registry entry named
  `clawlight`. The daemon runs without a console window in the system tray.
- **Linux** — an XDG autostart entry at
  `~/.config/autostart/clawlight.desktop`. Tray icon support depends on
  your desktop environment's system-tray/appindicator support — it works
  out of the box on most desktops (KDE, XFCE, Cinnamon, etc.), though GNOME
  needs an extension such as AppIndicator.

Either way it shows a color-coded Clawd icon:

| Icon   | Meaning                           |
|--------|-----------------------------------|
| Green  | All sessions actively working     |
| Yellow | At least one session is inactive  |
| Red    | At least one session needs help   |
| Gray   | No live sessions                  |

Clicking the icon shows session counts, a list of live sessions, and an
"Open clawlight" entry that launches the TUI in a new terminal window
(Terminal on macOS; Windows Terminal or a console on Windows).

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
`clawlight led` (use `--port` to pin a device — e.g.
`--port /dev/cu.usbmodemXXXX` on macOS or `--port COM5` on Windows).

Auto-detection prefers ESP32 native-USB boards. If your board instead
connects through a CH340/CP210x UART bridge and you have other serial
devices attached (e.g. an Arduino), pin it explicitly via `led_port` in
the config (e.g. `COM5` or `/dev/cu.usbserial-XXXX`).

## Uninstall

```bash
clawlight uninstall
```

This removes the login autostart (the launchd LaunchAgent on macOS, the
`Run` registry entry on Windows, or the XDG autostart entry on Linux),
clears the hook entries from `~/.claude/settings.json`, and deletes
`~/.claude/clawlight`.

## License

MIT
