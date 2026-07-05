# clawlight

<img width="1164" height="655" alt="claw-light-gif" src="https://github.com/user-attachments/assets/3e148527-169f-4f36-b2cd-0bcc3160527b" />

**The simple way to manage your Claude Code sessions.**

clawlight is a TUI dashboard and menu bar / system tray indicator for
monitoring [Claude Code](https://claude.ai/code) sessions in real time.


## Features

- **TUI dashboard**: live view of every Claude Code session with status
  (working, idle, needs help), updating in real time via file watching
- **Jump to session**: press `↵` in the TUI (or click a session in the
  tray popover) and clawlight raises the exact terminal window or tab
  hosting that session, whether it's a Terminal/iTerm tab, a tmux pane,
  or an IDE's integrated terminal
- **Menu bar / system tray icon**: a pixel-art Clawd that changes color
  with aggregate session health; auto-starts at login (launchd on macOS,
  a registry Run key on Windows, XDG autostart on Linux)
- **Desktop notifications**: a native notification the moment a session
  flips to "needs help," so permission prompts never sit unnoticed
- **Auto-naming**: sessions are named from their first prompt using the
  Claude CLI, so the list reads "fix flaky auth test," not a UUID
- **ESP32 status light (optional)**: mirror the aggregate state to a
  physical desk light over USB serial ([clawlight.dev](https://clawlight.dev))


## Install

### Homebrew

```bash
brew install clawlight/tap/clawlight
```

### From GitHub Releases

Download a binary from [Releases](https://github.com/clawlight/clawlight-cli/releases)
(replace `VERSION`, e.g. `0.9.0`):

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


```bash
git clone https://github.com/clawlight/clawlight-cli.git
cd clawlight-cli
cargo install --path .
```

## Quick Start

```bash
# Install hooks into Claude Code and start the tray daemon
clawlight install

# Launch the TUI dashboard
clawlight
```

`clawlight install` registers the built-in hook and menu bar icon.

## Usage

```
clawlight                     Launch the TUI dashboard
clawlight install             Install hooks and start the menu bar daemon
clawlight uninstall           Remove hooks, unload the daemon, clean up
clawlight menubar             Run the tray daemon in the foreground (debugging)
clawlight led [--port PORT]   Mirror state to the ESP32 in the foreground (debugging)
clawlight update <firmware>   Push new ESP32 firmware over serial, no cable reflash
```

Inside the dashboard:

```
↵:focus session   j/k:nav   r:reload   x:clear   l:toggle ESP32 LEDs   q/Esc:quit
```

`↵` raises the terminal window or tab that hosts the selected session.
The hook backend records each session's hosting window (terminal
emulator, IDE integrated terminal, or tmux pane) as it runs, so the jump
lands on the right one.

## Menu bar / system tray

`clawlight install` sets up the tray daemon to start at login:

- **macOS**: a launchd LaunchAgent. The plist lives at
  `~/Library/LaunchAgents/io.roush.clawlight.menubar.plist`; logs are at
  `~/.claude/clawlight/menubar.{log,err}`.
- **Windows**: an `HKCU\…\CurrentVersion\Run` registry entry named
  `clawlight`. The daemon runs without a console window in the system tray.
- **Linux**: an XDG autostart entry at
  `~/.config/autostart/clawlight.desktop`. Tray support depends on your
  desktop's appindicator support; it works out of the box on most desktops
  (KDE, XFCE, Cinnamon, etc.), though GNOME needs an extension such as
  AppIndicator.

The icon is a color-coded Clawd:

| Icon   | Meaning                           |
|--------|-----------------------------------|
| Green  | All sessions actively working     |
| Yellow | At least one session is idle      |
| Red    | At least one session needs help   |
| Gray   | No live sessions                  |

On macOS and Windows, clicking the icon opens a popover with session
counts, the list of live sessions (click one to jump to its terminal),
a **Settings** view, and buttons to open the full TUI or quit. On Linux
the tray uses the native menu.

### Notifications

When a session transitions to "needs help" (e.g. Claude is waiting on a
permission prompt), clawlight fires a native desktop notification: macOS
Notification Center, WinRT toast on Windows, freedesktop notifications on
Linux.

### Yellow-light behavior

By default any idle session turns the aggregate light yellow, even while
others are working. If you'd rather stay green while *anything* is active
(yellow only when every session is idle), switch the mode in the tray
popover's Settings view.

## ClawLight (optional)

A ready-made desk light is at [clawlight.dev](https://clawlight.dev).

## Uninstall

```bash
clawlight uninstall
```

This removes the login autostart (launchd LaunchAgent on macOS, the `Run`
registry entry on Windows, or the XDG autostart entry on Linux), clears
the hook entries from `~/.claude/settings.json`, and deletes
`~/.claude/clawlight`.

## License

MIT
