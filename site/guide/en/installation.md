<a href="../../">Back to the project</a>

# Install and connect { #install }

## Install ClawLight and the firmware { #installation }

Download the BLE package for a Mac with Apple Silicon (M1 or newer). It includes the app and C3
firmware. Intel Macs need a separate build; the upstream Homebrew package does not include this
BLE version.

[Download for Mac · Apple Silicon ↓](../../../dist/clawlight-v0.13.0-aarch64-apple-darwin-20260916T194919685892Z.tar.gz){ download }

[Download checksum](../../../dist/clawlight-v0.13.0-aarch64-apple-darwin-20260916T194919685892Z.tar.gz.sha256){ download }

## Set it up with your coding agent { #agent-setup }

Unpack the package or source archive, open that folder in Codex or another coding agent, and
paste this request. For a prebuilt package, keep its checksum file nearby. Detailed manual
steps are in `docs/hardware-setup.md` inside the folder.

```text
Help me set up ClawLight using docs/hardware-setup.md. If this is a prebuilt package, verify its
checksums; if it is a source checkout, follow the source build instructions. Back up existing
settings and keep one menu bar service. Identify my actual board before hardware commands.
This project's firmware targets ESP32-C3; another Bluetooth LE SuperMini needs
matching firmware and pins. If the package does not support my board, explain what is needed
before flashing. Flash only if needed and keep existing pairings. Tell me when to connect the
cable or approve a macOS dialog, then check that the background service reaches my lamp. For
Codex, guide me through reviewing and trusting the six ClawLight hooks via /hooks in the CLI
with the same CODEX_HOME as the app; leave approval to me. Verify a real local task in the CLI
or desktop app changes from Working to Idle/Paused in ClawLight; BLE status alone is not
enough. For desktop use, validate in the desktop app.
```

## Pair once, then place it on your desk { #pairing }

Connect the flashed lamp to your Mac with a USB data cable. Run these commands in Terminal,
allow ClawLight to use Bluetooth, and approve the macOS pairing prompt.

```sh
~/.cargo/bin/clawlight ble pair
~/.cargo/bin/clawlight ble status
```

Check “connected” for your intended lamp: the background service has sent it a status update.
This does not test the LEDs themselves. Keep it powered from your Mac or use a USB adapter.
ClawLight retries connections while its service is running, lamp control and Bluetooth are
enabled, and the powered lamp is nearby.
