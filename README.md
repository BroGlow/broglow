[English](README.md) | [Русский](README.ru.md)

# BroGlow

<p align="center">
  <a href="https://github.com/BroGlow/broglow/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/BroGlow/broglow?style=for-the-badge&labelColor=181A19&color=C25B2A&label=release&logo=github&logoColor=white"></a>
  <a href="https://github.com/BroGlow/broglow/actions/workflows/ci.yaml"><img alt="CI status" src="https://img.shields.io/github/actions/workflow/status/BroGlow/broglow/ci.yaml?branch=master&style=for-the-badge&labelColor=181A19&label=CI&logo=githubactions&logoColor=white"></a>
  <a href="https://github.com/BroGlow/broglow/releases"><img alt="Total downloads" src="https://img.shields.io/github/downloads/BroGlow/broglow/total?style=for-the-badge&labelColor=181A19&color=C25B2A&label=downloads"></a>
  <a href="site/guide/en/installation.md"><img alt="Install with Homebrew" src="https://img.shields.io/badge/install-Homebrew-FBB040?style=for-the-badge&labelColor=181A19&logo=homebrew&logoColor=white"></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-3D444D?style=for-the-badge&labelColor=181A19"></a>
</p>
<p>
  <a href="https://docs.anthropic.com/en/docs/claude-code"><img alt="Works with Claude Code" src="https://img.shields.io/badge/works%20with-Claude%20Code-D97757?style=for-the-badge&labelColor=181A19&logo=claude&logoColor=white"></a>
  <a href="https://github.com/openai/codex"><img alt="Works with Codex" src="https://img.shields.io/badge/works%20with-Codex-10A37F?style=for-the-badge&labelColor=181A19&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyBmaWxsPSIjZmZmIiByb2xlPSJpbWciIHZpZXdCb3g9IjAgMCAyNCAyNCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48dGl0bGU%2BT3BlbkFJPC90aXRsZT48cGF0aCBkPSJNMjIuMjgxOSA5LjgyMTFhNS45ODQ3IDUuOTg0NyAwIDAgMC0uNTE1Ny00LjkxMDggNi4wNDYyIDYuMDQ2MiAwIDAgMC02LjUwOTgtMi45QTYuMDY1MSA2LjA2NTEgMCAwIDAgNC45ODA3IDQuMTgxOGE1Ljk4NDcgNS45ODQ3IDAgMCAwLTMuOTk3NyAyLjkgNi4wNDYyIDYuMDQ2MiAwIDAgMCAuNzQyNyA3LjA5NjYgNS45OCA1Ljk4IDAgMCAwIC41MTEgNC45MTA3IDYuMDUxIDYuMDUxIDAgMCAwIDYuNTE0NiAyLjkwMDFBNS45ODQ3IDUuOTg0NyAwIDAgMCAxMy4yNTk5IDI0YTYuMDU1NyA2LjA1NTcgMCAwIDAgNS43NzE4LTQuMjA1OCA1Ljk4OTQgNS45ODk0IDAgMCAwIDMuOTk3Ny0yLjkwMDEgNi4wNTU3IDYuMDU1NyAwIDAgMC0uNzQ3NS03LjA3Mjl6bS05LjAyMiAxMi42MDgxYTQuNDc1NSA0LjQ3NTUgMCAwIDEtMi44NzY0LTEuMDQwOGwuMTQxOS0uMDgwNCA0Ljc3ODMtMi43NTgyYS43OTQ4Ljc5NDggMCAwIDAgLjM5MjctLjY4MTN2LTYuNzM2OWwyLjAyIDEuMTY4NmEuMDcxLjA3MSAwIDAgMSAuMDM4LjA1MnY1LjU4MjZhNC41MDQgNC41MDQgMCAwIDEtNC40OTQ1IDQuNDk0NHptLTkuNjYwNy00LjEyNTRhNC40NzA4IDQuNDcwOCAwIDAgMS0uNTM0Ni0zLjAxMzdsLjE0Mi4wODUyIDQuNzgzIDIuNzU4MmEuNzcxMi43NzEyIDAgMCAwIC43ODA2IDBsNS44NDI4LTMuMzY4NXYyLjMzMjRhLjA4MDQuMDgwNCAwIDAgMS0uMDMzMi4wNjE1TDkuNzQgMTkuOTUwMmE0LjQ5OTIgNC40OTkyIDAgMCAxLTYuMTQwOC0xLjY0NjR6TTIuMzQwOCA3Ljg5NTZhNC40ODUgNC40ODUgMCAwIDEgMi4zNjU1LTEuOTcyOFYxMS42YS43NjY0Ljc2NjQgMCAwIDAgLjM4NzkuNjc2NWw1LjgxNDQgMy4zNTQzLTIuMDIwMSAxLjE2ODVhLjA3NTcuMDc1NyAwIDAgMS0uMDcxIDBsLTQuODMwMy0yLjc4NjVBNC41MDQgNC41MDQgMCAwIDEgMi4zNDA4IDcuODcyem0xNi41OTYzIDMuODU1OEwxMy4xMDM4IDguMzY0IDE1LjExOTIgNy4yYS4wNzU3LjA3NTcgMCAwIDEgLjA3MSAwbDQuODMwMyAyLjc5MTNhNC40OTQ0IDQuNDk0NCAwIDAgMS0uNjc2NSA4LjEwNDJ2LTUuNjc3MmEuNzkuNzkgMCAwIDAtLjQwNy0uNjY3em0yLjAxMDctMy4wMjMxbC0uMTQyLS4wODUyLTQuNzczNS0yLjc4MThhLjc3NTkuNzc1OSAwIDAgMC0uNzg1NCAwTDkuNDA5IDkuMjI5N1Y2Ljg5NzRhLjA2NjIuMDY2MiAwIDAgMSAuMDI4NC0uMDYxNWw0LjgzMDMtMi43ODY2YTQuNDk5MiA0LjQ5OTIgMCAwIDEgNi42ODAyIDQuNjZ6TTguMzA2NSAxMi44NjNsLTIuMDItMS4xNjM4YS4wODA0LjA4MDQgMCAwIDEtLjAzOC0uMDU2N1Y2LjA3NDJhNC40OTkyIDQuNDk5MiAwIDAgMSA3LjM3NTctMy40NTM3bC0uMTQyLjA4MDVMOC43MDQgNS40NTlhLjc5NDguNzk0OCAwIDAgMC0uMzkyNy42ODEzem0xLjA5NzYtMi4zNjU0bDIuNjAyLTEuNDk5OCAyLjYwNjkgMS40OTk4djIuOTk5NGwtMi41OTc0IDEuNDk5Ny0yLjYwNjctMS40OTk3WiIvPjwvc3ZnPg%3D%3D&logoColor=white"></a>
  <a href="https://opencode.ai"><img alt="Works with opencode" src="https://img.shields.io/badge/works%20with-opencode-3D444D?style=for-the-badge&labelColor=181A19&logo=opencode&logoColor=white"></a>
  <a href="https://github.com/github/copilot-cli"><img alt="Works with GitHub Copilot CLI" src="https://img.shields.io/badge/works%20with-Copilot%20CLI-6E40C9?style=for-the-badge&labelColor=181A19&logo=githubcopilot&logoColor=white"></a>
</p>
<p>
  <img alt="Lamp: ESP32-C3" src="https://img.shields.io/badge/lamp-ESP32--C3-3D444D?style=for-the-badge&labelColor=181A19&logo=espressif&logoColor=white">
  <img alt="Lamp connects over USB or Bluetooth LE" src="https://img.shields.io/badge/connects%20over-USB%20%C2%B7%20Bluetooth%20LE-3D444D?style=for-the-badge&labelColor=181A19&logo=bluetooth&logoColor=white">
  <img alt="Runs on macOS, Windows and Linux" src="https://img.shields.io/badge/runs%20on-macOS%20%C2%B7%20Windows%20%C2%B7%20Linux-3D444D?style=for-the-badge&labelColor=181A19">
</p>

A local session monitor for Claude Code, opencode, Codex and GitHub Copilot CLI,
with a menu-bar app, terminal dashboard and optional ESP32-C3 status lamp.
The lamp, connected over USB or Bluetooth LE, shows working, idle and needs-input states.

You can [install and configure BroGlow](site/guide/en/installation.md) before you
build or get a lamp. The app and terminal dashboard already show your local agent
sessions and their status without any hardware; add a lamp whenever you're ready.

<p align="center">
  <img src="site/assets/photos/broglow-all-lights.jpg" width="420" alt="The BroGlow lamp with all three lights glowing red, yellow and green, next to the orange mascot">
</p>

**[Read the guide on the website](https://broglow.github.io/broglow/docs/)**: install,
build the lamp, connect Codex and fix problems.

Maintained by **Suren Khorenyan**. Copyright notices are in [LICENSE](LICENSE).

<p align="center">
  <img src="site/assets/photos/signal-yellow.jpg" width="420" alt="The BroGlow lamp with only the yellow light glowing, matching the app's paused-session status">
  <img src="site/assets/photos/popover-dark.png" width="353" alt="The BroGlow menu-bar popover: three live sessions, one working and two paused, with Dashboard, Settings and Lamps controls">
</p>

<a name="clawlight"></a>

## How BroGlow differs from ClawLight

BroGlow is a separate, independently maintained application inspired by
[ClawLight](https://github.com/clawlight/clawlight-cli). Most of the app has been
rewritten, with a substantially redesigned interface and expanded capabilities:
sessions open their own chat and carry real names, wireless lamps are managed one
at a time, Homebrew installs and upgrades it, and the magnetic mascot is swappable.
See [Why BroGlow](site/guide/en/why.md) for the full comparison.

## Start here

- **Install or update:** [installation guide](site/guide/en/installation.md) (Homebrew), [alternative installations](site/guide/en/alternatives.md) (Mac package, source, Windows, Linux), or let your coding agent follow the [agent runbook](site/guide/en/hardware.md#agent-runbook).
- **Connect a flashed lamp:** [pair once](site/guide/en/installation.md#pairing).
- **Build and wire the ESP32-C3 lamp:** [wiring guide](site/guide/en/hardware.md#wiring-and-resistor-choice).
- **Print the enclosure:** get the models on [Printables](https://www.printables.com/model/1884645) or [MakerWorld](https://makerworld.com/models/3468773), then follow [printing and assembly](models/magnetic/GUIDE_ROUND3X2.en.md). The mascot holds on by four Ø3×2 mm N52 magnets (two in the mascot, two in the traffic light) and can be swapped for any mascot.
- **Something not working:** [troubleshooting](site/guide/en/troubleshooting.md).

For a new or unflashed C3, follow the [flashing and backup instructions](site/guide/en/hardware.md#flash-and-recover-usb-upload).

| Platform | Package | Lamp |
| --- | --- | --- |
| macOS | App, C3 firmware and manuals | USB or Bluetooth LE |
| Windows | Tray app and CLI; no installer, firmware or manuals | USB |
| Linux | Build from source | USB |

GitHub Releases also provide the enclosure as a separate download.

## Install

On macOS, the quickest route is Homebrew, followed by the required `broglow install`,
which registers the hooks in your coding agents, installs the app and starts it at login:

```bash
brew install broglow/tap/broglow
broglow install
```

To install a specific release instead, use `broglow/tap/broglow@VERSION`, for example
`brew install broglow/tap/broglow@1.0.0`; brew prints how to put it first on your `PATH`.

For the Mac package from Releases, Windows, Linux or a build from source, see
[alternative installations](site/guide/en/alternatives.md). No board is flashed by
app installation.

On macOS, press **⌘ Space**, search **BroGlow** and press Return to open its
controls. The app lives in `~/Applications/BroGlow.app`.
You can also run `open "$HOME/Applications/BroGlow.app"`. In Terminal, `broglow`
opens the optional terminal dashboard once `~/.cargo/bin` is on
[PATH](site/guide/en/alternatives.md#path).

## Connect your coding agents

Installation always registers Claude Code hooks and adds hooks for other agents
it detects. Details per agent:
[Connect your coding agents](site/guide/en/agents.md).

| Agent | Setup note |
| --- | --- |
| Claude Code | Hooks in `~/.claude/settings.json`. |
| opencode | Installed plugin; restart running sessions. |
| Codex | Codex >= 0.144; approve hooks via `/hooks`. [Connect Codex](site/guide/en/agents.md#codex-setup). |
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

Needs input always wins (red); in **Settings** (macOS and Windows) choose whether
mixed working and idle sessions show yellow or green.

## Contributing

Bug reports, ideas, documentation improvements and code contributions are welcome.
See the [contributing guide](CONTRIBUTING.md) to get started.

## License

[MIT](LICENSE). Release packages include third-party notices: the licenses of the Rust crates
the app is built from and the Rust standard library notices; Mac packages also list the
firmware's linked libraries and their licenses. See the [media policy](MEDIA.md)
for project images.
