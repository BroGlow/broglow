[English](README.md) | [Русский](README.ru.md)

# ClawLight

A local session monitor for Claude Code, opencode, Codex and GitHub Copilot CLI,
with a menu-bar app, terminal dashboard and optional ESP32-C3 status lamp.
The lamp shows working, idle and needs-input states over USB or Bluetooth LE.
Version **1.0.0** is in preparation.

Maintained by **Suren Khorenyan**. Copyright notices are in [LICENSE](LICENSE).

## Why this version?

This version extends [the original ClawLight](https://github.com/clawlight/clawlight-cli)
with wireless lamp control, a customizable DIY build and everyday desktop improvements.

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

- **Install or update:** [setup manual](docs/hardware-setup.md#install-or-update-the-mac-service).
- **Connect a flashed lamp:** [USB-assisted setup](docs/hardware-setup.md#usb-assisted-setup).
- **Build/wire a C3:** [hardware guide](docs/hardware-setup.md#wiring-and-resistor-choice).
- **Print the optional enclosure:** [printing and assembly](models/clawlight/PRINT_GUIDE.en.md).

Mac packages include the app, C3 firmware and manuals; Windows archives are
**CLI-only**, and Linux builds from source. BLE control is macOS-only.
Apple Silicon has been tested locally; see [verification limits](docs/hardware-setup.md#repeatable-checks-and-results)
for other machines. The enclosure is a separate 1.0.0 release-candidate download.

## Install

Download the full Mac package for your architecture and its matching `.sha256`
file from **this repository's Releases**. Follow the [checksum and installation
steps](docs/hardware-setup.md#install-a-prebuilt-package). Builds are not Developer
ID notarized; the manual describes macOS's normal per-app approval flow.

To build the app from a source checkout instead:

```bash
cargo install --locked --path .
"$HOME/.cargo/bin/clawlight" install
```

On Linux, install the [system development libraries](CONTRIBUTING.md#development-setup)
before compiling. On Windows, use the Rust MSVC
toolchain and put the built executable on PATH, then run `clawlight install`.
No board is flashed by app installation.

## Connect your coding agents

Installation registers hooks only for detected agents; already-running sessions
may need a restart.

| Agent | Setup note |
| --- | --- |
| Claude Code | Hooks in `~/.claude/settings.json`. |
| opencode | Installed plugin; restart running sessions. |
| Codex | Codex >= 0.144; review hooks via `/hooks`. [Local desktop-task setup](docs/hardware-setup.md#connect-local-codex-sessions). |
| GitHub Copilot CLI | Installed hooks; restart sessions open during installation. |

Cloud tasks and ordinary ChatGPT conversations do not emit this Mac's local hooks.

## Use it

Click the menu-bar icon for sessions and **Lamps**: add a lamp over USB, name it
or adjust its brightness. Multiple saved lamps work independently.
Session **×** hides a row until new activity; the conversation stays in its coding app.

```text
clawlight --version          Identify the installed release
clawlight                    Open the terminal dashboard
clawlight ble pair           Identify and remember a flashed C3 over USB
clawlight ble status         Check current lamp delivery
clawlight dashboard-button show   Enable optional dashboard shortcuts
clawlight uninstall          Remove hooks, autostart and ClawLight settings
```

The app starts at login. Needs input takes color priority; Settings controls
whether mixed working/idle sessions show yellow or green. See the
[manual](docs/hardware-setup.md) for firmware, pairing and troubleshooting.

## Contributing

Bug reports, ideas, documentation improvements and code contributions are welcome.
See the [contributing guide](CONTRIBUTING.md) to get started.

## License

[MIT](LICENSE). Packages include dependency notices. See the [media policy](docs/media.md)
for project images.
