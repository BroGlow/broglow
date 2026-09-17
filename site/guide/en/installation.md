<a href="../../">Back to the project</a>

# Install and connect { #install }

## Install ClawLight and the firmware { #installation }

Download the BLE package for a Mac with Apple Silicon (M1 or newer). It includes the app and C3
firmware. Intel Macs need a separate build; the upstream Homebrew package does not include this
BLE version.

[Download for Mac · Apple Silicon ↓](../../../dist/clawlight-v0.13.0-aarch64-apple-darwin-20260917T131708267560Z.tar.gz){ download }

[Download checksum](../../../dist/clawlight-v0.13.0-aarch64-apple-darwin-20260917T131708267560Z.tar.gz.sha256){ download }

## Set it up with your coding agent { #agent-setup }

Unpack the package or source archive, open that folder in Codex or another coding agent, and
paste this request. For a prebuilt package, keep its checksum file nearby. Detailed manual
steps are in `docs/hardware-setup.md` inside the folder.

```text
Help me set up ClawLight using docs/hardware-setup.md. If this is a prebuilt package, verify its
checksums; if it is a source checkout, follow the source build instructions. Back up existing
settings and keep one menu bar service. Verify in a fresh terminal that clawlight resolves to
the installed BLE version from outside the project; configure PATH only if needed, preserving
shell settings and avoiding duplicate entries. Identify my actual board before hardware commands.
This project's firmware targets ESP32-C3; another Bluetooth LE SuperMini needs
matching firmware and pins. If the package does not support my board, explain what is needed
before flashing. Flash only if needed and keep existing pairings. Tell me when to connect the
cable or approve a macOS dialog, then check that the background service reaches my lamp. For
Codex, guide me through reviewing and trusting the six ClawLight hooks via /hooks in the CLI
with the same CODEX_HOME as the app; leave approval to me. Verify a real local task in the CLI
or desktop app changes from Working to Idle/Paused in ClawLight; BLE status alone is not
enough. For desktop use, validate in the desktop app.
```

## Run ClawLight from any folder { #path }

ClawLight is installed once for your Mac user account at `~/.cargo/bin/clawlight`.
You do not need to open the project folder to run it. Prebuilt installs do not need Cargo.
In a fresh Terminal, check:

--8<-- "docs/hardware-setup.md:path-check"

The first result should point to your `~/.cargo/bin/clawlight`. If it does, no changes are
needed. If the command is missing or an older Homebrew copy comes first, add this line **once**
to `~/.zshrc` (macOS's default zsh), preserving its existing settings:

--8<-- "docs/hardware-setup.md:path-setup"

Open a new Terminal and repeat the checks. Running `export` only in the current terminal
does not configure future terminals. Rust may already set this up through `~/.cargo/env`;
avoid duplicate entries. For another shell, use its startup file.

`clawlight install` sets up hooks and the tray service; it does not configure `PATH`.
Scripts and agent subprocesses may not load your shell settings: use
`"$HOME/.cargo/bin/clawlight"` there or set their `PATH`. The login service already uses
an absolute executable path and does not depend on your terminal's `PATH`.

## Pair once, then place it on your desk { #pairing }

Connect the flashed lamp to your Mac with a USB data cable. Run these commands in Terminal,
allow ClawLight to use Bluetooth, and approve the macOS pairing prompt.

```sh
clawlight ble pair
clawlight ble status
```

Check “connected” for your intended lamp: the background service has sent it a status update.
This does not test the LEDs themselves. Keep it powered from your Mac or use a USB adapter.
ClawLight retries connections while its service is running, lamp control and Bluetooth are
enabled, and the powered lamp is nearby.

## Adjust brightness and manage lamps { #controls }

Click the lamp status at the bottom of the menu-bar popup to open **Lamps**.
Use **Light brightness** (0–100%); connected Bluetooth lamps preview all three LEDs
while you adjust it, then return to session status two seconds after the last preview.
At 0% the controlled LEDs stay dark while Bluetooth remains connected.

Expand a lamp's row to see its connection details or **Forget lamp**. The terminal
dashboard is optional: show its shortcut under **Settings → Appearance → Show Dashboard button**.

See the [hardware reference](hardware.md) for flashing, recovery and current
[hardware test results](hardware.md#repeatable-checks-and-results).
