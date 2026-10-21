[English](README.md) | [Русский](README.ru.md)

# BroGlow

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
