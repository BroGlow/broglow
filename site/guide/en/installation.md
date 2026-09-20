<a href="../../">Back to the project</a>

<a name="install"></a>

# Install and connect

<a name="installation"></a>

## Install BroGlow and the firmware

Use this project's complete Mac package, which includes the app, ESP32-C3 firmware
and operating guide. Choose the build for your Mac's processor. Apple Silicon has
been tested on hardware; Intel builds still need installation and Bluetooth checks.

<!-- mac-downloads -->

<a name="agent-setup"></a>

## Set it up with your coding agent

Unpack the package or source archive, open it in your coding agent, and paste this
request. Keep the package's checksum file nearby. Prefer manual setup? Follow the
[installation steps](hardware.md#install-or-update-the-mac-service).

```text
Set up BroGlow following the Agent runbook in docs/hardware-setup.md.
Choose the package or source path, verify checksums for a package, and back up settings
in .recovery/. Preserve hooks, lamp names, brightness and bonds; keep one tray service.
Verify the board is an ESP32-C3 SuperMini before hardware commands; flash only if needed.
Connect one lamp by USB for automatic setup; use Lamps → Add lamp to retry. Leave macOS
and Codex /hooks approval to me; for Codex, use the same CODEX_HOME as the app.
Verify broglow works from a fresh terminal outside the project, the background service
reaches my lamp, and a real local agent task changes from Working to Idle/Paused.
Check Identify scans red/yellow/green three times and restores the status and saved brightness;
if firmware lacks this mode, report that a matching firmware update is needed.
If I use Codex desktop, test a desktop task. Report any unverified physical checks.
```

<a name="path"></a>

## Run BroGlow from any folder

To open the Mac app, press **⌘ Space**, search **BroGlow** and press Return.
Its traffic-light icon opens the session controls. You can also open
`~/Applications/BroGlow.app` in Finder or run `open "$HOME/Applications/BroGlow.app"`.
It starts at login; the plain `broglow` command opens the terminal dashboard.

BroGlow installs to `~/.cargo/bin/broglow`; prebuilt installs do not need Cargo.
In a fresh terminal outside the project, check:

--8<-- "docs/hardware-setup.md:path-check"

If the first result is not `~/.cargo/bin/broglow`, add this line **once** to
`~/.zshrc` (default macOS shell), preserving existing settings:

--8<-- "docs/hardware-setup.md:path-setup"

Open a new terminal and repeat the checks. Avoid duplicates if Rust already configured
`~/.cargo/env`; use the matching startup file for other shells. `broglow install`
does not set `PATH`. Scripts can use `"$HOME/.cargo/bin/broglow"` directly;
the login service already uses an absolute path.

<a name="pairing"></a>

## Pair once, then place it on your desk

Connect one flashed lamp with a USB data cable. On macOS, BroGlow starts setup
automatically within five seconds. Allow Bluetooth and approve any macOS pairing prompt.
Use **Lamps → Add lamp** to retry. Add lamps one at a time; existing lamps stay saved.

The Terminal alternative is:

```sh
broglow ble pair
broglow ble status
```

Wait for **Connected** on the intended lamp's row: “Lamp added” only means saved.
Connected confirms a status write, not visible LED operation. Keep the lamp powered
from your Mac or a USB adapter; BroGlow reconnects while the service, lamp control
and Bluetooth are on and the lamp is nearby.

<a name="controls"></a>

## Adjust brightness and manage lamps

Open **Lamps** and expand a row:

- **Name:** up to 40 characters, saved on this Mac; blank restores the automatic label.
- **Identify:** scans that connected lamp's red → yellow → green LEDs three times,
  one at a time at 100%, 100 ms per LED (0.9 seconds total), then restores the latest
  status and saved brightness without changing settings.
- **Light brightness:** 0–100% for that lamp. **All lamps** replaces every saved lamp's
  individual level. Preview ends two seconds after the last change.
- **Forget lamp:** removes it from this Mac; other lamps and Bluetooth bonds remain.

At 0%, LEDs stay dark while Bluetooth remains connected; **Identify** still works.
Offline brightness changes apply on reconnection without enabling lamp control.
Identify needs matching firmware; older firmware silently ignores the scan mode.
Updating the app does not flash the board; see the [firmware update steps](hardware.md#build-and-identify-the-board).

The terminal dashboard is optional: show its shortcut under
**Settings → Appearance → Show Dashboard button**.

See the [hardware reference](hardware.md) for flashing, recovery and current
[hardware test results](hardware.md#repeatable-checks-and-results).
