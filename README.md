# ClawLight

A local coding-session monitor with a menu-bar app, terminal dashboard and optional
ESP32-C3 status lamp. It integrates Claude Code, opencode, Codex and GitHub Copilot
CLI. The lamp shows working, idle and needs-input states over USB or Bluetooth LE.

We are preparing **1.0.0**.

## Start here

- **Install or update:** [setup manual](docs/hardware-setup.md#install-or-update-the-mac-service).
- **Connect a flashed lamp:** [USB-assisted setup](docs/hardware-setup.md#usb-assisted-setup).
- **Build/wire a C3:** [hardware guide](docs/hardware-setup.md#wiring-and-resistor-choice).
- **Publish a release or website:** [maintainer runbook](docs/releasing.md) in the source repository.

The release workflow builds complete Mac packages for Apple Silicon and Intel:
app executable, C3 firmware, manuals, licenses and checksums. Windows archives
are labeled **CLI-only**. BLE lamp control is macOS-only. Linux users build from
source. Local runtime validation has used Apple Silicon; other machines need
separate acceptance checks. See [verified behavior and limits](docs/hardware-setup.md#repeatable-checks-and-results).

The enclosure release candidate is ready for the **1.0.0** release. Its separate
print package includes STL, STEP, source and the print guide; see
[printing and assembly](models/clawlight/PRINT_GUIDE.md) in the source checkout.
The app and wired C3 lamp can also be used without a printed case.

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

Click the menu-bar icon for sessions and **Lamps**. Add a lamp over USB, name it,
set its brightness or forget it. A connected indication requires a recent
acknowledged write. Several saved lamps are supported; an absent lamp does not
block the others. Session **×** dismisses a row until new activity, keeping the
conversation in its coding app.

```text
clawlight --version          Identify the installed release
clawlight                    Open the terminal dashboard
clawlight ble pair           Identify and remember a flashed C3 over USB
clawlight ble status         Check current lamp delivery
clawlight dashboard-button show   Enable optional dashboard shortcuts
clawlight uninstall          Remove hooks, autostart and ClawLight settings
```

The lamp's colors represent aggregate session state: needs input takes priority;
Settings controls whether mixed working/idle sessions show yellow or green.
The app starts at login after installation. The [manual](docs/hardware-setup.md)
contains recovery, firmware upload, pairing and diagnostics.

## License

[MIT](LICENSE). Dependency license notices are included in distributed packages.
Project media use our own photos, screenshots, diagrams and renders. See the
[photo checklist and media policy](docs/media.md).
