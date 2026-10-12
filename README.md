[English](README.md) | [Русский](README.ru.md)

# BroGlow

A local session monitor for Claude Code, opencode, Codex and GitHub Copilot CLI,
with a menu-bar app, terminal dashboard and optional ESP32-C3 status lamp.
The lamp, connected over USB or Bluetooth LE, shows working, idle and needs-input states.

**[Read the guide on the website](https://broglow.github.io/broglow/docs/)**: install,
build the lamp, connect Codex and fix problems.

Maintained by **Suren Khorenyan**. Copyright notices are in [LICENSE](LICENSE).

<a name="why-this-version"></a>

## How BroGlow differs from ClawLight

BroGlow is a separate, independently maintained application based on
[ClawLight](https://github.com/clawlight/clawlight-cli). It substantially reworks
and extends the original with macOS Bluetooth lamp control, improved Codex
desktop integration, per-lamp setup and brightness controls, a customizable
printable enclosure, and a bilingual interactive assembly guide.

| What's better | What you gain |
| --- | --- |
| **Wireless lamp connection** | Put the lamp anywhere in Bluetooth range; no data cable to the Mac. |
| **Codex desktop (ChatGPT app) support** | Jump directly to the right conversation; renamed tasks stay recognizable. |
| **Clear paused sessions** | Remove clutter in one click while keeping active work and approval requests visible. |
| **Brightness with live preview** | Adjust each lamp to a comfortable level and see the result before saving. |
| **Setup from the app** | Connect the correct lamp through guided USB-assisted setup. |
| **Clear connection status** | Know whether each lamp is receiving updates or needs attention. |
| **Remembered, named lamps** | Keep home and office lamps configured and easy to recognize. |
| **Print and customize the enclosure** | Choose the appearance and adapt the design to your preferences. |
| **Identify a lamp** | Flash the selected lamp to confirm which one you're controlling. |
| **Interactive assembly guide** | Rotate and zoom the separated parts to understand how they fit together. |

Bluetooth lamp features (wireless control, setup from the app, naming, per-lamp
brightness, **Identify**) and direct Codex task opening are macOS-only.
The lamp still needs power and is designed for DIY assembly.

## Start here

- **Install or update:** [installation guide](site/guide/en/installation.md), or let your coding agent do it: [agent setup](site/guide/en/installation.md#agent-setup).
- **Connect a flashed lamp:** [pair once](site/guide/en/installation.md#pairing).
- **Build and wire the ESP32-C3 lamp:** [wiring guide](site/guide/en/hardware.md#wiring-and-resistor-choice).
- **Print the optional enclosure:** [recommended magnetic enclosure](models/magnetic/GUIDE_ROUND3X2.en.md), with a removable mascot and four Ø3×2 mm N52 magnets. [Glued alternative](models/broglow/PRINT_GUIDE.en.md).
- **Something not working:** [troubleshooting](site/guide/en/troubleshooting.md).

For a new or unflashed C3, follow the [flashing and backup instructions](site/guide/en/hardware.md#flash-and-recover-usb-upload).

| Platform | Package | Lamp |
| --- | --- | --- |
| macOS | App, C3 firmware and manuals | USB or Bluetooth LE |
| Windows | Tray app and CLI; no installer, firmware or manuals | USB |
| Linux | Build from source | USB |

Apple Silicon has been tested locally; see [verification limits](site/guide/en/hardware.md#repeatable-checks-and-results)
for other machines. The enclosure is a separate download on the same release.

## Install

Download the full Mac package for your architecture and its matching `.sha256`
file from [Releases](https://github.com/broglow/broglow/releases/latest). Follow the
[checksum and installation steps](site/guide/en/hardware.md#install-a-prebuilt-package).
Builds are not signed with a Developer ID or notarized by Apple; the manual describes
macOS's normal per-app approval flow.

On Windows, extract `broglow-vVERSION-x86_64-pc-windows-msvc.tar.gz` from Releases
into a permanent folder (for example, `tar -xf <file>` in PowerShell).
In PowerShell from that folder, run `.\broglow.exe install` to register hooks,
enable startup at login and launch the tray app. `.\broglow.exe menubar` launches
the graphical tray interface directly; `.\broglow.exe` opens the terminal dashboard.

To build the app from a source checkout instead (Rust 1.90+):

```bash
cargo install --locked --path .
"$HOME/.cargo/bin/broglow" install
```

On Linux, install the [system development libraries](CONTRIBUTING.md#development-setup)
before compiling. The Linux tray uses GTK 3 bindings with known advisories; see
[distribution](site/guide/en/releasing.md#distribution). On Windows, use the Rust MSVC
toolchain, run `cargo install --locked --path .`, then `broglow install`.
No board is flashed by app installation.

On macOS, press **⌘ Space**, search **BroGlow** and press Return to open its
controls. The app lives in `~/Applications/BroGlow.app` and starts at login.
You can also run `open "$HOME/Applications/BroGlow.app"`. In Terminal, `broglow`
opens the optional terminal dashboard once `~/.cargo/bin` is on
[PATH](site/guide/en/installation.md#path).

## Connect your coding agents

Installation always registers Claude Code hooks and adds hooks for other agents
it detects; already-running sessions may need a restart.

| Agent | Setup note |
| --- | --- |
| Claude Code | Hooks in `~/.claude/settings.json`. |
| opencode | Installed plugin; restart running sessions. |
| Codex | Codex >= 0.144; approve hooks via `/hooks`. [Connect Codex](site/guide/en/codex.md#codex-setup). |
| GitHub Copilot CLI | Installed hooks; restart sessions open during installation. |

Cloud tasks and ordinary ChatGPT conversations do not trigger this Mac's local hooks.

## Use it

Click the menu-bar or tray icon for sessions. On macOS, **Lamps** adds a lamp
(USB-assisted Bluetooth setup), names it or adjusts its brightness; multiple saved
lamps work independently. On Windows and Linux, the lamp is connected over USB.
Session **×** hides a row until new activity; the conversation stays in its coding app.

```text
broglow --version               Identify the installed release
broglow                         Open the terminal dashboard
broglow ble pair                Set up a flashed C3 over USB and pair it over Bluetooth (macOS)
broglow ble status              Check whether status reaches the lamp
broglow dashboard-button show   Show terminal-dashboard buttons in the tray app
broglow uninstall               Remove hooks, autostart, the app (macOS) and all BroGlow data
```

The app starts at login. Needs input always wins (red); in **Settings** (macOS and
Windows) choose whether mixed working and idle sessions show yellow or green.
See the [guide](site/guide/en/installation.md) and [troubleshooting](site/guide/en/troubleshooting.md);
the [manual](site/guide/en/hardware.md) covers firmware, wiring and verification.

## Contributing

Bug reports, ideas, documentation improvements and code contributions are welcome.
See the [contributing guide](CONTRIBUTING.md) to get started.

## License

[MIT](LICENSE). Release packages include third-party notices: the licenses of the Rust crates
the app is built from and the Rust standard library notices; Mac packages also list the
firmware's linked libraries and their licenses. See the [media policy](site/guide/en/media.md)
for project images.
