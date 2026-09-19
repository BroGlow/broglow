[English](README.md) | [Русский](README.ru.md)

# ClawLight

A local session monitor for Claude Code, opencode, Codex and GitHub Copilot CLI,
with a menu-bar app, terminal dashboard and optional ESP32-C3 status lamp.
The lamp shows working, idle and needs-input states over USB or Bluetooth LE.
Version **1.0.0** is in preparation.

## Why this version?

This version extends [the original ClawLight](https://github.com/clawlight/clawlight-cli)
with wireless lamp control, a customizable DIY build and everyday desktop improvements.

| What’s better | What you gain |
| --- | --- |
| **Wireless lamp connection** | Place your lamp within Bluetooth range without a data cable running to your Mac. |
| **Adjustable brightness** | Keep the light comfortable for your room, with a separate brightness setting for each lamp. |
| **Remembered lamps** | Save and name your lamps, so your home and office setups are easy to recognize. |
| **Find your lamp** | Make a selected lamp flash with one click, so you know which one you’re controlling. |
| **Clear paused sessions** | Clear the clutter in one click. Active work and requests for your attention stay visible; your conversations stay safe. |
| **Open the right Codex task** | Supports local tasks in the Codex desktop app: jump straight from the monitor to the conversation you need. |
| **Make it yours** | Print and customize your own enclosure using the included design files. |
| **Guides in two languages** | Follow setup and assembly instructions in English or Russian. |

Wireless control and direct Codex task opening are available on macOS.
The lamp still needs power and is designed for DIY assembly.

## Start here

- **Install or update:** [setup manual](docs/hardware-setup.md#install-or-update-the-mac-service).
- **Connect a flashed lamp:** [USB-assisted setup](docs/hardware-setup.md#usb-assisted-setup).
- **Build/wire a C3:** [hardware guide](docs/hardware-setup.md#wiring-and-resistor-choice).
- **Print the optional enclosure:** [printing and assembly](models/clawlight/PRINT_GUIDE.en.md).
- **Publish a release or website:** [maintainer runbook](docs/releasing.md) in the source repository.

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

On Linux, install the system development libraries listed in
`.github/workflows/ci.yaml` before compiling. On Windows, use the Rust MSVC
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

## Python formatting

From the repository root, install the development tools and enable the commit hook:

```bash
python3 -m venv .venv
. .venv/bin/activate
python -m pip install -r requirements-dev.txt
prek install
black .
```

Black uses its default style for all Python files. The prek hook formats staged Python
files; CI runs `black --check --diff .` and fails if formatting is needed.
Run `prek run --all-files` to check every tracked file locally.
Keep Black's version in `requirements-dev.txt` and `prek.toml` in sync.

## License

[MIT](LICENSE). Packages include dependency notices. See the [media policy](docs/media.md)
for project images.
