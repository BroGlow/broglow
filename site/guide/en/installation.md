<a href="../../">Back to the project</a>

<a name="install"></a>

# Install and connect

<a name="homebrew"></a>

## Install with Homebrew

```bash
brew install broglow/tap/broglow
broglow install
```

`broglow install` is a required step: it registers BroGlow's hooks in your coding
agents (Claude Code always; Codex, opencode and GitHub Copilot CLI when they are
installed), installs the menu-bar app and starts it at login. No Homebrew, or on
Windows or Linux? See [Alternative installations](alternatives.md).

<a name="path"></a>

## Open BroGlow and check the command

To open the Mac app, press **⌘ Space**, search **BroGlow** and press Return.
Its traffic-light icon opens the session controls. You can also open
`~/Applications/BroGlow.app` in Finder or run `open "$HOME/Applications/BroGlow.app"`.
It starts at login. The plain `broglow` command opens the terminal dashboard; on its
first run on an unconfigured Mac it also registers the hooks and login startup.

With Homebrew, the `broglow` command lives in `$(brew --prefix)/bin/broglow`, which is
already on PATH. The package and source routes put it in `~/.cargo/bin/broglow` (the
prebuilt package does not need Cargo). In a fresh terminal outside the project, check:

--8<-- "site/guide/en/hardware.md:path-check"

If you installed from a package or source and the first line is not your
`~/.cargo/bin/broglow` (shown as a full path, such as `/Users/you/.cargo/bin/broglow`),
add this line **once** to `~/.zshrc` (default macOS shell), preserving existing settings:

--8<-- "site/guide/en/hardware.md:path-setup"

Open a new terminal and repeat the checks. If `~/.zshrc` already loads
`"$HOME/.cargo/env"` (the Rust installer adds it), change nothing; other shells use
their own startup file. `broglow install` does not change `PATH`.

<a name="pairing"></a>

## Pair once, then place it on your desk

Connect one flashed lamp with a USB data cable. On macOS, BroGlow starts setup
automatically within five seconds. Allow Bluetooth and approve any macOS pairing prompt.
To retry, click the **Lamp** button in the menu footer (it reads **Lamps** once you have
several) and choose **Add lamp**. Add lamps one at a time; existing lamps stay saved.

The Terminal alternative is:

```sh
broglow ble pair
broglow ble status
```

Wait for **Connected** on the intended lamp's row: **Lamp added** only means saved.
Connected confirms a status write, not visible LED operation. Then power the lamp from
your Mac or a USB adapter. BroGlow reconnects automatically when the lamp is in range,
as long as the app is running and lamp control and Bluetooth are on.
For several boards or failed setup, see [USB-assisted setup](hardware.md#usb-assisted-setup).

<a name="controls"></a>

## Adjust brightness and manage lamps

Open the **Lamp** button in the menu footer and expand a lamp's row:

- **Name:** up to 40 characters, saved on this Mac; blank restores the automatic label.
- **Identify:** scans that connected lamp's red → yellow → green LEDs three times,
  one at a time at 100%, 100 ms per LED (0.9 seconds total), then restores the latest
  status and saved brightness without changing settings.
- **Light brightness:** 0–100% for that lamp. **All lamps** replaces every saved lamp's
  individual level. Preview ends two seconds after the last change.
- **Forget lamp:** removes it, with its name and brightness, from this Mac; other lamps
  and Bluetooth bonds remain. Forgetting the last lamp also turns lamp control off.

At 0%, the controlled LEDs stay dark (the board's red power LED stays on) while
Bluetooth remains connected; **Identify** still works on a connected lamp.
You can change brightness while a lamp is offline: it applies on reconnection and does
not turn lamp control on.
Identify needs current BroGlow firmware; older firmware ignores the scan.
Updating the app does not flash the board; see the [firmware update steps](hardware.md#flash-and-recover-usb-upload).

The terminal dashboard is optional: show its shortcut under
**Settings → Appearance → Show Dashboard button**, or run `broglow dashboard-button show`.

See the [hardware reference](hardware.md) for flashing, recovery and current
[hardware test results](hardware.md#repeatable-checks-and-results).
