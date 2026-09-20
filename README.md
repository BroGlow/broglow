[English](README.md) | [Русский](README.ru.md)

# BroGlow

A local session monitor for Claude Code, opencode, Codex and GitHub Copilot CLI,
with a menu-bar app, terminal dashboard and optional ESP32-C3 status lamp.
The lamp shows working, idle and needs-input states over USB or Bluetooth LE.
Version **1.0.0** is in preparation.

Maintained by **Suren Khorenyan**. Copyright notices are in [LICENSE](LICENSE).

## Why this version?

BroGlow is a separate, independently maintained application based on
[ClawLight](https://github.com/clawlight/clawlight-cli). It substantially reworks
and extends the original with macOS Bluetooth lamp control, improved Codex
desktop integration, per-lamp setup and brightness controls, a customizable
printable enclosure, and a bilingual interactive assembly guide.

| What’s better | What you gain |
| --- | --- |
| **Wireless lamp connection** | Place your lamp within Bluetooth range without a data cable to your Mac. |
| **Codex desktop (ChatGPT app) support** | Jump directly to the right conversation; renamed tasks stay recognizable. |
| **Clear paused sessions** | Remove clutter in one click while keeping active work and approval requests visible. |
| **Brightness with live preview** | Adjust each lamp to a comfortable level and see the result before saving. |
| **Setup from the app** | Connect the correct lamp through guided USB-assisted setup. |
| **Clear connection status** | Know whether each lamp is receiving updates or needs attention. |
| **Remembered, named lamps** | Keep home and office lamps configured and easy to recognize. |
| **Print and customize the enclosure** | Choose the appearance and adapt the design to your preferences. |
| **Identify a lamp** | Flash the selected lamp to confirm which one you’re controlling. |
| **Interactive assembly guide** | Rotate and zoom the separated parts to understand how they fit together. |

Wireless control and direct Codex task opening are available on macOS.
The lamp still needs power and is designed for DIY assembly.

## Start here

- **Install or update:** [setup manual](site/guide/en/hardware.md#install-or-update-the-mac-service).
- **Connect a flashed lamp:** [USB-assisted setup](site/guide/en/hardware.md#usb-assisted-setup).
- **Build/wire a C3:** [hardware guide](site/guide/en/hardware.md#wiring-and-resistor-choice).
- **Print the optional enclosure:** [printing and assembly](models/broglow/PRINT_GUIDE.en.md).

Mac packages include the app, C3 firmware and manuals; Windows archives include
**the graphical tray app and CLI**, and Linux builds from source. Windows packages
support USB lamp control but include no installer, firmware or manuals. BLE control is macOS-only.
Apple Silicon has been tested locally; see [verification limits](site/guide/en/hardware.md#repeatable-checks-and-results)
for other machines. The enclosure is a separate 1.0.0 release-candidate download.

## Install

Download the full Mac package for your architecture and its matching `.sha256`
file from **this repository's Releases**. Follow the [checksum and installation
steps](site/guide/en/hardware.md#install-a-prebuilt-package). Builds are not Developer
ID notarized; the manual describes macOS's normal per-app approval flow.

On Windows, extract the Windows archive from Releases into a permanent folder.
In PowerShell from that folder, run `.\broglow.exe install` to register hooks,
enable startup at login and launch the tray app. `.\broglow.exe menubar` launches
the graphical tray interface directly; `.\broglow.exe` opens the terminal dashboard.

To build the app from a source checkout instead:

```bash
cargo install --locked --path .
"$HOME/.cargo/bin/broglow" install
```

On Linux, install the [system development libraries](CONTRIBUTING.md#development-setup)
before compiling. On Windows, use the Rust MSVC
toolchain and put the built executable on PATH, then run `broglow install`.
No board is flashed by app installation.

On macOS, press **⌘ Space**, search **BroGlow** and press Return to open its
controls. The app lives in `~/Applications/BroGlow.app` and starts at login.
You can also run `open "$HOME/Applications/BroGlow.app"`. In Terminal, `broglow`
opens the optional terminal dashboard.

## Connect your coding agents

Installation registers hooks only for detected agents; already-running sessions
may need a restart.

| Agent | Setup note |
| --- | --- |
| Claude Code | Hooks in `~/.claude/settings.json`. |
| opencode | Installed plugin; restart running sessions. |
| Codex | Codex >= 0.144; review hooks via `/hooks`. [Local desktop-task setup](site/guide/en/hardware.md#connect-local-codex-sessions). |
| GitHub Copilot CLI | Installed hooks; restart sessions open during installation. |

Cloud tasks and ordinary ChatGPT conversations do not emit this Mac's local hooks.

## Use it

Click the menu-bar icon for sessions and **Lamps**: add a lamp over USB, name it
or adjust its brightness. Multiple saved lamps work independently.
Session **×** hides a row until new activity; the conversation stays in its coding app.

```text
broglow --version          Identify the installed release
broglow                    Open the terminal dashboard
broglow ble pair           Identify and remember a flashed C3 over USB
broglow ble status         Check current lamp delivery
broglow dashboard-button show   Enable optional dashboard shortcuts
broglow uninstall          Remove hooks, autostart and BroGlow settings
```

The app starts at login. Needs input takes color priority; Settings controls
whether mixed working/idle sessions show yellow or green. See the
[manual](site/guide/en/hardware.md) for firmware, pairing and troubleshooting.

## Contributing

Bug reports, ideas, documentation improvements and code contributions are welcome.
See the [contributing guide](CONTRIBUTING.md) to get started.

## License

[MIT](LICENSE). Packages include dependency notices. See the [media policy](site/guide/en/media.md)
for project images.
