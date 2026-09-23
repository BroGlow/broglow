---
title: Hardware reference
---

# BroGlow: setup and operation

BroGlow sends local coding-agent status to ESP32-C3 SuperMini lamps over USB or,
on macOS, Bluetooth LE. A Mac can drive several lamps; each needs USB power, no Wi-Fi.
Run source commands from the checkout root and package commands from the extracted
package. The optional enclosure is a separate **1.0.0** release print package.
See [verification limits](#repeatable-checks-and-results) before claiming hardware support.

## Choose a setup path

| Task | Start here |
| --- | --- |
| Ask Codex or another agent to do the setup | [Agent runbook](#agent-runbook) |
| Install without compiling Rust | [Install a prebuilt package](#install-a-prebuilt-package) |
| Build on your own Mac | [Build from source](#build-from-source) |
| Connect an already flashed lamp | [USB-assisted setup](#usb-assisted-setup) |
| Find the icon or validate the service | [Menu bar icon and service validation](#menu-bar-icon-and-service-validation) |
| Show local Codex task statuses from the desktop app | [Connect local Codex sessions](#connect-local-codex-sessions) |
| Flash a new C3 using ready firmware | [Flash prebuilt C3 firmware](#flash-prebuilt-c3-firmware) |
| Isolate LED wiring from Bluetooth and session control | [Standalone LED test firmware](#standalone-led-test-firmware) |

### Agent runbook

Ask an agent opened in the package or checkout:

> Install BroGlow using site/guide/en/hardware.md. Preserve my settings and unrelated
> hooks, keep one menu-bar service, and verify its status. Connect my already-flashed
> C3 lamp if attached; ask before flashing an unknown board.

1. Check Mac architecture. `BUILD.json` + `bin/broglow` means a prebuilt package;
   `Cargo.toml` + `src/` means source. Use the matching installation path below.
2. Inspect the service and [snapshot it](#install-or-update-the-mac-service) before
   changes. Reuse a working installation for connection-only requests.
3. Identify the board, then connect one lamp by USB; macOS setup starts automatically.
   Use **Lamps → Add lamp** or `broglow ble pair` to retry.
   Working firmware needs no reflash or bond reset.
4. Leave Bluetooth consent and [Codex hook trust](#connect-local-codex-sessions)
   to the user. Preserve foreign hook order/settings; never manufacture trust
   hashes or overwrite Codex configuration.
5. Verify one signed bundled service, CLI on PATH and a fresh lamp acknowledgement.
   Report missing consent, board or visual LED checks. A build or saved ID is insufficient.

For development, also read `AGENTS.md` and `CLAUDE.md` in the source checkout.

## Wiring and resistor choice

Disconnect USB power before wiring. Use a separate **330 Ω resistor for red/yellow
and 220 Ω for green**, rated 1/8 W or 1/4 W.

<!-- --8<-- [start:lamp-wiring] -->
```text
GPIO0 ── 330 Ω ── red LED anode (+)    cathode (−) ── GND
GPIO1 ── 330 Ω ── yellow LED anode (+) cathode (−) ── GND
GPIO3 ── 220 Ω ── green LED anode (+)  cathode (−) ── GND
```
<!-- --8<-- [end:lamp-wiring] -->

The cathode is normally the shorter lead/flat rim side; verify trimmed LEDs with
a multimeter's diode mode. Cathodes share GND, **never a resistor**. Power the board
from USB-C; LEDs use GPIO **3.3 V**, not the 5 V pin. Use separate indicator LEDs,
not an RGB LED or addressable strip.

| LED | Board pad | Command | Meaning |
| --- | --- | --- | --- |
| Red / top | `0` / GPIO0 | `R` | Needs you |
| Yellow / middle | `1` / GPIO1 | `Y` | Idle |
| Green / bottom | `3` / GPIO3 | `G` | Working |
| No session color | — | `N` | Off while fresh, except startup, connection confirmation, brightness preview or Identify |

Pad labels **0, 1, 3 and G (GND) are on the same side**; they are not physical pin
positions. Leave pad 2 unused. GPIO0/1/3 support PWM and avoid boot-strapping pins
2/8/9, USB 18/19, BOOT 9 and the blue LED 8. This lamp does not use GPIO0/1's optional
32.768 kHz crystal function. [Espressif pin functions](https://documentation.espressif.com/esp32-c3_datasheet_en.html),
[GPIO reference](https://docs.espressif.com/projects/esp-idf/en/v5.4/esp32c3/api-reference/peripherals/gpio.html).

`I = (3.3 V − Vf) / R`: at `Vf = 2.0 V`, 330 Ω gives **3.9 mA / 5 mW**;
470 Ω gives 2.8 mA and 220 Ω gives 5.9 mA. Actual brightness varies with LED forward
voltage. For different LEDs, start at 330 Ω each and adjust within their current
ratings. Never omit a resistor.

## Build and identify the board

Install [uv](https://docs.astral.sh/uv/getting-started/installation/); `pyproject.toml`
and `uv.lock` provide PlatformIO Core **6.2.0**, esptool **5.4.0** and pyserial.
Rust remains in `~/.cargo/bin/`. Keep firmware pinned to `espressif32 @ 7.1.3` /
Arduino-ESP32 **2.0.17**, including its BLE library.
The host locks **btleplug 0.13.1** for CoreBluetooth write-error propagation;
macOS pairing approval is still required.

```bash
# Start in this project's BroGlow source checkout.
export PATH="$HOME/.cargo/bin:$PATH"

# Build only; neither command accesses the board.
uv run flash --build-only
cargo build --locked

# Enumerate USB serial devices; this does not identify an exact board model.
uv run platformio device list
```

Before any upload or serial command, physically identify the **ESP32-C3 SuperMini**
and compare ports before/after plugging it in with a data cable. Disconnect similar
boards if needed. Espressif VID `0x303A` is shared across chips: do not choose by VID,
example name or list order. Stop if no matching board appears. Replace this placeholder:

```bash
BOARD_PORT='/dev/cu.REPLACE_WITH_IDENTIFIED_BOARD'
```

The `esp32-c3-devkitm-1` profile specifies C3, **4 MB flash**, DIO / 40 MHz and native
USB, not the SuperMini PCB layout. After identifying the board, verify chip and flash:

```bash
~/.platformio/penv/bin/python ~/.platformio/packages/tool-esptoolpy/esptool.py \
  --chip esp32c3 --port "$BOARD_PORT" chip_id
~/.platformio/penv/bin/python ~/.platformio/packages/tool-esptoolpy/esptool.py \
  --chip esp32c3 --port "$BOARD_PORT" flash_id
```

These reads reset the board into its bootloader. Require **ESP32-C3 / 4 MB** and
resolve mismatches before uploading. [PlatformIO profile](https://docs.platformio.org/en/latest/boards/espressif32/esp32-c3-devkitm-1.html).

## Flash and recover USB upload

The standard path is the same from a source checkout and an extracted Mac package:

```bash
uv run flash
uv run flash --backup
uv run flash --skip-info
uv run flash --port PORT
```

The command selects only one unambiguous USB serial board, verifies ESP32-C3 and
exactly 4 MB flash, asks before writing, and requires an exact BroGlow `INFO`
response after reset. `--skip-info` skips only the pre-flash firmware probe.
Backups are opt-in: `--backup` reads all 4 MB into a private `.recovery/` file,
prints its SHA-256 and warns that it may contain Bluetooth keys. By default no
backup is made. Noninteractive `--yes` also requires an explicit `--port`.
The command pauses installed lamp control before the first serial probe and
waits three seconds for the worker to stop. It restores previously enabled
control on the selected transport after verification, cancellation or failure;
an already disabled lamp stays disabled. The documented `~/.cargo/bin/broglow`
installation is also found when it is not on PATH.

In a source checkout the command builds and uploads with PlatformIO. In a package
it validates `flash-manifest.json` and the four included images before using
esptool. It never performs a routine full erase or writes `firmware.bin` at zero.

For advanced manual source upload, identify the board first. If BroGlow is
installed, disable lamp control:

```bash
broglow ble off
```

Wait three seconds; close foreground `broglow led` and serial monitors, then upload:

```bash
~/.platformio/penv/bin/pio run -d firmware/esp32-c3-supermini \
  -t upload --upload-port "$BOARD_PORT"
```

If connection fails: hold **BOOT**, tap/release **RESET**, release **BOOT**; re-list
ports and update `BOARD_PORT`. Retry; tap RESET if the app does not start. Check the
data cable. [Espressif flashing instructions](https://docs.espressif.com/projects/esp-idf/en/stable/esp32c3/get-started/establish-serial-connection.html).

Outputs are in `firmware/esp32-c3-supermini/.pio/build/esp32-c3-supermini/`.
PlatformIO uploads bootloader, partition table and app at their proper offsets.
**Never write application-only `firmware.bin` at zero.** Updates use this USB upload
or prebuilt esptool below; no in-application serial/BLE updater exists.

Rebuild after changing `LED_RED_PIN`, `LED_YELLOW_PIN`, `LED_GREEN_PIN` or
`STATUS_TIMEOUT_MS` in [platformio.ini](../../../firmware/esp32-c3-supermini/platformio.ini).
See [indications](#pairing-feedback-and-diagnostics-without-external-leds) for onboard LED options.

### Flash prebuilt C3 firmware

Skip for a working flashed lamp. The verified package contains four images and
`firmware/esp32-c3-supermini/flash-manifest.json`; no firmware compiler is needed.
After verifying package checksums, use `uv run flash` from the package root.
The manual esptool procedure below is advanced recovery. First
[identify the board](#build-and-identify-the-board). If esptool is missing, use
Python 3 ([macOS installer](https://www.python.org/downloads/macos/)):

```bash
python3 -m venv "$HOME/.local/share/broglow-flash-tools"
ESPTOOL_PY="$HOME/.local/share/broglow-flash-tools/bin/python"
"$ESPTOOL_PY" -m pip install 'esptool==5.4.0'
"$ESPTOOL_PY" -m serial.tools.list_ports -v
# Set BOARD_PORT to the physically identified port, not an example from this guide.
"$ESPTOOL_PY" -m esptool --chip esp32c3 --port "$BOARD_PORT" chip-id
"$ESPTOOL_PY" -m esptool --chip esp32c3 --port "$BOARD_PORT" flash-id
```

Require **ESP32-C3 / 4 MB** and the verified `PACKAGE_DIR`. Disable lamp control,
wait three seconds and close serial clients. To opt into a manual full backup:

```bash
RECOVERY_ROOT="$PACKAGE_DIR/.recovery"
mkdir -p -m 700 "$RECOVERY_ROOT"
chmod 700 "$RECOVERY_ROOT"
FLASH_BACKUP="$RECOVERY_ROOT/c3-before-flash-$(date +%Y%m%d-%H%M%S).bin"
(umask 077; "$ESPTOOL_PY" -m esptool --chip esp32c3 --port "$BOARD_PORT" \
  read-flash 0x0 0x400000 "$FLASH_BACKUP")
shasum -a 256 "$FLASH_BACKUP"
```

Keep the backup private: it may contain Bluetooth keys. Compare the manifest with
these offsets, then flash with pinned [esptool 5.4.0](https://pypi.org/project/esptool/5.4.0/):

```bash
FIRMWARE_DIR="$PACKAGE_DIR/firmware/esp32-c3-supermini"
cat "$FIRMWARE_DIR/flash-manifest.json"
"$ESPTOOL_PY" -m esptool --chip esp32c3 --port "$BOARD_PORT" \
  --baud 460800 write-flash --flash-mode dio --flash-freq 40m --flash-size 4MB \
  0x0 "$FIRMWARE_DIR/bootloader.bin" \
  0x8000 "$FIRMWARE_DIR/partitions.bin" \
  0xe000 "$FIRMWARE_DIR/boot_app0.bin" \
  0x10000 "$FIRMWARE_DIR/firmware.bin"
```

Require successful write/hash verification and a running app, then
[pair over USB](#usb-assisted-setup). These writes preserve NVS `0x9000..0xe000`
only for the matching partition layout; unrelated firmware needs a full backup.
Do not use `erase_flash` routinely or write the application image at zero.
Use the BOOT/RESET recovery above if needed.

### Full-flash recovery image

Keep full-flash dumps private with their checksum and board identity: they contain
firmware and Bluetooth keys. Stop serial clients and restore only to the same board;
restoration replaces firmware and bonds. A complete **4 MB** dump starts at zero,
unlike application-only `firmware.bin`. Never distribute another board's backup.

## Standalone LED test firmware

Build `led-test` to isolate wiring from Bluetooth, session state and PWM. It cycles
**red GPIO0, yellow GPIO1, green GPIO3** at full digital brightness, one second each;
the onboard blue LED pulses for 100 ms per change. Keep one resistor per LED and
shared cathode GND. The package includes test sources, not a prebuilt test image.

[Re-identify the board/port](#build-and-identify-the-board), verify **C3 / 4 MB**,
close serial monitors and run from the source checkout:

```bash
broglow ble off
# Wait for the existing lamp worker to release the device.
sleep 3
~/.platformio/penv/bin/pio run -d firmware/esp32-c3-supermini -e led-test
~/.platformio/penv/bin/pio run -d firmware/esp32-c3-supermini -e led-test \
  -t upload --upload-port "$BOARD_PORT"
~/.platformio/penv/bin/pio device monitor --port "$BOARD_PORT" --baud 115200
```

Require verified upload. Compare `led-test color=red gpio=0 duration_ms=1000` and
`pin_levels red=1 yellow=0 green=0` with the visible LED; yellow/1 and green/3 follow.
Exactly the named pin should read high. Logic readback does not measure voltage,
current or LED health. For a dark LED, disconnect power and check polarity, resistor,
contacts and GND; this test alone does not identify the failed component.

Test firmware has **no BLE, `INFO`, pairing, `?`, serial color commands or production
startup breath**. Ctrl-C closes the monitor, not the cycle; power-off stops it and
reset restarts it. Leave a requested test board in this mode until normal operation
is requested. Only then close the monitor, re-identify its port and restore production:

```bash
~/.platformio/penv/bin/pio run -d firmware/esp32-c3-supermini \
  -e esp32-c3-supermini -t upload --upload-port "$BOARD_PORT"
```

For a replacement/new board, then use [USB pairing](#usb-assisted-setup); retain
existing lamp IDs unless asked to forget them. Images live under the firmware
directory's `.pio/build/led-test/` and `.pio/build/esp32-c3-supermini/`.

## Check the wiring before connecting BroGlow

This check needs **production firmware**, an identified `BOARD_PORT`, disabled lamp
control and closed serial monitors. Disconnect BLE: a connected BLE client owns
the LEDs. Use PlatformIO Python or the [prebuilt flashing environment](#flash-prebuilt-c3-firmware):

```bash
# Source setup:
SERIAL_PY="$HOME/.platformio/penv/bin/python"
# For prebuilt setup instead: SERIAL_PY="$HOME/.local/share/broglow-flash-tools/bin/python"
"$SERIAL_PY" - "$BOARD_PORT" <<'PY'
import sys
import time
import re
import serial

with serial.Serial(sys.argv[1], 115200, timeout=0.5, write_timeout=2) as port:
    port.rts = False
    port.dtr = True
    time.sleep(0.3)
    port.reset_input_buffer()

    def request(command, prefix):
        port.write(command)
        port.flush()
        until = time.monotonic() + 3
        while time.monotonic() < until:
            reply = port.readline().decode(errors="replace").strip()
            if reply.startswith(prefix):
                return reply
        raise RuntimeError("No firmware response; recheck the identified port")

    identity = request(b"INFO\n", "broglow_pair ")
    if not re.fullmatch(r"broglow_pair identity=[0-9a-f]{12} service=7d2ea28a-f7bd-485a-bd9d-92ad6ecfe93e", identity):
        raise RuntimeError("Unexpected firmware identity; stop and identify the board")
    print(identity, flush=True)
    diagnostic = request(b"?\n", "state=")
    if " connected=0 " not in f" {diagnostic} ":
        raise RuntimeError("BLE still owns the lamp; disable it before this test")
    try:
        time.sleep(3)  # Let any startup breathing finish.
        for _ in range(10):
            for command, label in [(b"R\n", "red"), (b"Y\n", "yellow"), (b"G\n", "green")]:
                print(label, flush=True)
                port.write(command)
                port.flush()
                time.sleep(1)
    finally:
        port.write(b"N\n")
        port.flush()
        print("all off", flush=True)
PY
```

Expect red, yellow and green individually for one second each, ten cycles, then
`N`/off; interruption also attempts `N`. After the three-second startup breath,
fresh serial commands override waiting/pairing animation. Check dark/faint LEDs
with power disconnected: polarity, GPIO, resistor and shared GND. If all are dark,
also check firmware, port, brightness and BLE disconnection. Power-cycling with BLE
disabled restores 100% brightness.

Serial accepts complete `R\n`, `Y\n`, `G\n`, `N\n` lines at **115200 baud**, including
CRLF; empty, unknown, multi-character and oversized lines are ignored. To test
expiry, send one `R\n` and leave the port open without writes: state must become
`N` after **ten seconds**, then disconnected pairing/waiting indications resume.
Diagnostic `state`, `age_ms` and `mode` distinguish those from stale session color.

## Local recovery files

Keep snapshots and private hardware evidence in `.recovery/` at the checkout or
package root, mode **0700**; Git and public packages exclude it. Firmware dumps may
contain Bluetooth keys. Take one snapshot before installation changes, reuse it
for retries and retain it until verification. Builds/docs need no snapshot.
Preserve unknown firmware unless the owner explicitly designated the board expendable;
recovery files are not required to build, install or run a release.

## Install or update the Mac service

Run as the logged-in Mac user, **without sudo**. Inspect the installation first;
missing commands/services are normal on a fresh Mac:

```bash
command -v broglow
launchctl print "gui/$(id -u)/io.github.broglow.menubar"
pgrep -fl 'broglow.*menubar'
```

Before changes, snapshot settings/hooks, CLI and service from the checkout/package
root. Keep `.recovery/` even if removing the package while rollback is needed:

```bash
RECOVERY_ROOT="$PWD/.recovery"
mkdir -p -m 700 "$RECOVERY_ROOT"
chmod 700 "$RECOVERY_ROOT"
BACKUP_DIR="$RECOVERY_ROOT/install-$(date +%Y%m%d-%H%M%S)"
mkdir -m 700 "$BACKUP_DIR"
for item in \
  "$HOME/.claude/settings.json" \
  "$HOME/.claude/broglow/config.json" \
  "$HOME/.cargo/bin/broglow" \
  "${CODEX_HOME:-$HOME/.codex}/hooks.json" \
  "$HOME/.config/opencode/plugins/broglow.js" \
  "${COPILOT_HOME:-$HOME/.copilot}/hooks/broglow.json" \
  "$HOME/Library/LaunchAgents/io.github.broglow.menubar.plist"
do
  if [ -f "$item" ]; then
    mkdir -p "$BACKUP_DIR$(dirname "$item")"
    cp -p "$item" "$BACKUP_DIR$item"
  fi
done
if [ -d "$HOME/Applications/BroGlow.app" ]; then
  cp -R "$HOME/Applications/BroGlow.app" "$BACKUP_DIR/BroGlow.app"
fi
```

Both installation routes use `~/.cargo/bin/broglow` so hooks survive package
deletion; prebuilt use needs no Cargo. `broglow install` updates the app, hooks and
existing login service while preserving unrelated entries. Preserve Codex hook
order/approval, use [user hook review](#connect-local-codex-sessions), and never
overwrite `config.toml`. Restart running opencode sessions for the new plugin.
If PATH finds another copy, use `~/.cargo/bin/broglow install` explicitly.

Service `io.github.broglow.menubar`, registered at
`~/Library/LaunchAgents/io.github.broglow.menubar.plist`, runs:

```text
~/Applications/BroGlow.app/Contents/MacOS/broglow menubar
```

Redeploy this service; do not start a second `cargo run -- menubar`. Process and
BLE locks prevent duplicate tray instances and simultaneous scan/worker clients.
Restore only affected BroGlow settings from backups, preserving unrelated changes.

### Install a prebuilt package

Download this project's full Mac package and `.tar.gz.sha256`: choose
`aarch64-apple-darwin` for Apple Silicon or `x86_64-apple-darwin` for Intel.
Windows tray app and CLI archives are not Mac/C3 packages. No Rust, Cargo, Node.js or PlatformIO
is needed for app installation.

```bash
# Replace with the actual supplied archive path.
PACKAGE_ARCHIVE='/path/to/broglow-vVERSION-aarch64-apple-darwin.tar.gz'
cd "$(dirname "$PACKAGE_ARCHIVE")"
shasum -a 256 -c "$(basename "$PACKAGE_ARCHIVE").sha256"
mkdir -p "$HOME/Downloads"
tar -xzf "$PACKAGE_ARCHIVE" -C "$HOME/Downloads"
PACKAGE_DIR="$HOME/Downloads/$(basename "$PACKAGE_ARCHIVE" .tar.gz)"
cd "$PACKAGE_DIR"
shasum -a 256 -c SHA256SUMS
uname -m
file bin/broglow
cat BUILD.json
./bin/broglow --version
./bin/broglow ble pair --help
```

Require matching checksums/architecture, expected version and `ble pair` support.
`BUILD.json` records minimum macOS and source revision. Checksums verify integrity,
not publisher identity: use a trusted source. Local builds are not Developer ID
notarized. If blocked, verify the source then use macOS's per-app
[Open Anyway](https://support.apple.com/en-us/102445); never disable Gatekeeper globally.
After the [snapshot](#install-or-update-the-mac-service), install:

```bash
mkdir -p "$HOME/.cargo/bin"
install -m 755 "$PACKAGE_DIR/bin/broglow" "$HOME/.cargo/bin/broglow"
cmp "$PACKAGE_DIR/bin/broglow" "$HOME/.cargo/bin/broglow"
"$HOME/.cargo/bin/broglow" install
export PATH="$HOME/.cargo/bin:$PATH"
```

The installer locally signs the stable app bundle. Finish [PATH setup](#run-broglow-from-any-directory-path),
[service validation](#menu-bar-icon-and-service-validation) and [USB pairing](#usb-assisted-setup).
Only new/unflashed boards need the included [C3 images](#flash-prebuilt-c3-firmware).

### Build from source

Use a complete source checkout containing `Cargo.lock`, `src/ble.rs` and
`firmware/esp32-c3-supermini/platformio.ini`; the app archive is not a source
distribution. Preserve local changes. Identify builds by revision/`BUILD.json`,
not version alone.

Install only missing tools:

- Apple tools: `xcode-select -p`; if missing, `xcode-select --install`.
- [Rust stable](https://doc.rust-lang.org/book/ch01-01-installation.html);
  add checks with `rustup component add rustfmt clippy`.
- [PlatformIO Core](https://docs.platformio.org/en/latest/core/installation/methods/installer-script.html)
  for firmware (usual path `~/.platformio/penv/bin/pio`).
- [Node.js](https://nodejs.org/en/download) for JS tests;
  [Python 3](https://www.python.org/downloads/macos/) for firmware checks/packaging.

From the source root, build and check without uploading:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
node --test tests/js/*.test.mjs
cargo build --release --locked

# Companion firmware: build and checks only, no upload.
~/.platformio/penv/bin/python -m pip install clang-format==23.1.1
~/.platformio/penv/bin/clang-format --dry-run --Werror \
  firmware/esp32-c3-supermini/src/*.cpp firmware/esp32-c3-supermini/tests/*.cpp
python3 firmware/esp32-c3-supermini/tests/check.py
~/.platformio/penv/bin/pio run -d firmware/esp32-c3-supermini
```

First builds download pinned dependencies; use `--offline` only with a complete
Cargo cache. After the [snapshot](#install-or-update-the-mac-service), install:

```bash
cargo install --path . --locked
"$HOME/.cargo/bin/broglow" install
```

Run both after application code changes: compilation alone leaves the old service
running. Firmware needs an explicit upload to change a board. Finish
[service validation](#menu-bar-icon-and-service-validation), [PATH checks](#run-broglow-from-any-directory-path)
and, if requested, [lamp setup](#usb-assisted-setup).

### Run broglow from any directory (PATH)

The per-user CLI lives at `~/.cargo/bin/broglow` for both installation routes.
Source files are needed only to rebuild. Check PATH outside the checkout:

<!-- --8<-- [start:path-check] -->
```bash
cd /tmp
command -v broglow
type -a broglow
broglow --help
```
<!-- --8<-- [end:path-check] -->

The first path should be your `~/.cargo/bin/broglow`. If missing or shadowed,
add this **once** to `~/.zshrc`, open a new terminal and repeat:

<!-- --8<-- [start:path-setup] -->
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```
<!-- --8<-- [end:path-setup] -->

If Rust's `~/.cargo/env` already handles PATH, change nothing. Preserve existing
shell settings; other shells use their own startup files. A terminal-only export
does not affect future terminals. `broglow install` registers hooks/service,
not PATH. Scripts/jobs may skip shell startup files: use `"$HOME/.cargo/bin/broglow"`.
The LaunchAgent already uses the bundle's absolute path.

### Make a local prebuilt package

After checking and rebuilding changed host/firmware sources:

```bash
python3 scripts/package-macos.py
```

This packages existing outputs in ignored `dist/`, verifies image offsets and
partition boundaries, and records checksums/provenance. Transfer the `.tar.gz`
and `.sha256` together. The package contains CLI, four C3 images/manifest, this
manual, agent entry points and metadata. It does not build, install, flash or
publish. For tagged releases see `site/guide/en/releasing.md` in the source checkout.

## Menu bar icon and service validation

After installation, press **⌘ Space**, search **BroGlow** and press Return.
The traffic-light app icon opens the session controls, including when BroGlow is
already running. You can also open `~/Applications/BroGlow.app` in Finder or run:

```bash
open "$HOME/Applications/BroGlow.app"
```

BroGlow starts at login and stays in the menu bar. The Terminal command `broglow`
opens the optional TUI dashboard.

Find the **traffic-light icon at the right of the macOS menu bar** (hover:
`BroGlow`). No Dock icon/window is expected. Left-click opens sessions; the
**Lamp / Lamps 1/2 / Lamp off** footer opens **Lamps**, showing transport and
per-lamp status. Expand a row for controls; open **Connection details** for ID and diagnostics.

Dashboard shortcuts are hidden by default. Toggle **Settings → Appearance → Show
Dashboard button**, or:

```bash
broglow dashboard-button show
broglow dashboard-button hide
```

This saves `show_dashboard_button` and updates immediately. Hidden shortcuts also
prevent Terminal fallback when session focus fails. Direct `broglow` still opens
the TUI; **Quit** remains in the right-click menu.

Under each session title, an icon and name show its coding agent (Claude, Codex,
opencode or Copilot) before the project. Turn off **Settings → Appearance → Show
agent names** to keep only the icon; hover it to see the name. This saves
`show_agent_names`.

| Icon | Session state |
| --- | --- |
| ![Gray](../../../assets/icons/broglow-none.png) Gray | No live sessions |
| ![Green](../../../assets/icons/broglow-green.png) Green | Working, subject to Yellow mode |
| ![Yellow](../../../assets/icons/broglow-yellow.png) Yellow | Idle, subject to Yellow mode |
| ![Red](../../../assets/icons/broglow-red.png) Red | Needs input |

Icon color is not service/Bluetooth health; usage text is off by default. If hidden,
reveal the menu bar, leave full-screen mode or expand your menu-bar manager.
[Apple menu-bar help](https://support.apple.com/guide/mac-help/whats-in-the-menu-bar-mchlp1446/mac).

```bash
launchctl print "gui/$(id -u)/io.github.broglow.menubar"
ps -axo pid=,command= | awk '$2 ~ /(^|\/)broglow$/ && $3 == "menubar" {print}'
/usr/bin/plutil -lint "$HOME/Applications/BroGlow.app/Contents/Info.plist"
/usr/bin/codesign --verify --strict "$HOME/Applications/BroGlow.app"
"$HOME/.cargo/bin/broglow" ble status
```

Require launchctl `state = running` and exactly one `broglow menubar` at
`~/Applications/BroGlow.app/Contents/MacOS/broglow`, with the same PID.
Plist/signature checks establish registration/integrity, not Bluetooth permission.
Signing changes the app executable, so do not compare its raw hash with the CLI.
`ble status` reports recent writes: `disabled` before setup and `connected: 1/2`
with one remote lamp are normal. Physical LEDs still need a visual check.

For a loaded but stopped service:

```bash
launchctl kickstart -k "gui/$(id -u)/io.github.broglow.menubar"
```

Repeat checks; restore a missing bundle/job through the chosen install route.
Inspect `~/.claude/broglow/menubar.log` and `menubar.err` after exits. Never start
another tray as a health check. A first TUI launch can install hooks/autostart.

## Connect local Codex sessions

Codex supplies local session events through `$CODEX_HOME/hooks.json` (normally
`~/.codex/hooks.json`). Installation registers six hooks; **the user must trust
their exact definitions** before Codex runs new/changed non-managed hooks.

1. Install BroGlow; skip reinstall if its six `codex-hook` entries already exist.
2. Open Codex CLI with the desktop app's user and `CODEX_HOME`. On the development
   Mac it is `/Applications/ChatGPT.app/Contents/Resources/codex`; elsewhere use the
   actual bundled/installed CLI sharing that configuration.
3. In `/hooks`, review/trust BroGlow's `SessionStart`, `UserPromptSubmit`,
   `PreToolUse`, `PostToolUse`, `PermissionRequest` and `Stop`. Preserve unrelated
   hooks. Agents must not fabricate trust hashes, bypass trust or overwrite
   `config.toml`. Changed definitions may need review again. If explicitly disabled,
   the user can enable `hooks = true` under `[features]`, preserving managed policy.
4. Exit CLI, send a prompt in a new **local desktop task** and observe BroGlow.
   If hooks are not picked up, finish active work, restart the desktop app and retry.

Verify a real task becomes **Working**, then **Idle/Paused** after its response;
check **Needs you/Needs help** on a normal permission request. Other live sessions
and Yellow mode affect the lamp aggregate. `ble status` proves lamp delivery, not
Codex hook delivery; manual test events do not verify the desktop integration.
Old sidebar tasks need a new event; ordinary conversational questions are not
necessarily permission requests. ChatGPT chats and cloud/remote tasks do not feed
this Mac's local hooks.

Titles/renames follow Codex's local index within **five seconds** while the list is
open, without changing activity/status. **×** (TUI **x**) dismisses a session from
BroGlow only; new hook activity brings it back, viewing alone does not.
**Clear paused** beside the Paused heading dismisses all yellow sessions the same
way, keeping green working sessions and red sessions that need help.
Click/Enter focuses captured CLI terminals, otherwise opens
`codex://threads/<session-id>` in the installed desktop app, resolved by bundle ID
even if named ChatGPT.app.

[OpenAI hook trust](https://developers.openai.com/codex/hooks#review-and-trust-hooks),
[supported contexts](https://developers.openai.com/plugins/guides/submit-claude-plugin#review-what-openai-supports).

## Bluetooth identity and permission

Enable Bluetooth and allow **BroGlow** in **System Settings → Privacy & Security
→ Bluetooth**. The installed bundle supplies `NSBluetoothAlwaysUsageDescription`
and stable ID `io.github.broglow.menubar`; a LaunchAgent plist alone cannot.
`ble scan` re-executes that bundle to share its identity.

Local ad-hoc signing uses an identifier-only designated requirement, preserving
identity across rebuilds but **not authenticating a publisher**. Distribution
should consistently set a real `BROGLOW_SIGNING_IDENTITY`, using that signer's
default requirement. Changing signer may require consent again.

Neither signature checks nor a Terminal-authorized scan proves background-service
permission. Verify a write from the installed service with brick-only lamp power,
then service restart/login and recovery after permission denial.

[btleplug permissions](https://github.com/deviceplug/btleplug#macos),
[Apple usage description](https://developer.apple.com/documentation/bundleresources/information-property-list/nsbluetoothalwaysusagedescription),
[signing/TCC](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements),
[designated requirements](https://developer.apple.com/library/archive/technotes/tn2206/).

## Pair and remember the home and office lamps

Lamps advertise `BroGlow C3` plus a four-digit hexadecimal board suffix;
the Mac saves a CoreBluetooth UUID. Each lamp accepts one Mac at a time;
one Mac can update several lamps concurrently.

### USB-assisted setup

Connect one flashed board with a USB data cable; close serial monitors. The macOS
desktop app starts setup automatically within **five seconds**, including when the
board is attached at app startup. For a manual retry, select **Lamps → Add lamp** or run:

```bash
broglow ble pair
broglow ble status
```

Keep it attached while **Adding…**, and approve macOS pairing promptly. The command
identifies firmware, opens pairing and saves the matching BLE ID; **Connected**
requires a later encrypted worker write. Afterwards USB can supply power from
the Mac or a brick; control stays on BLE. For multiple/ambiguous boards, identify
the intended port first:

```bash
broglow ble pair --port "$BOARD_PORT"
```

Setup requires exact `INFO` identity, BroGlow service UUID and the full six-byte
base MAC in manufacturer data (`0xffff`); names/USB VID alone never select a lamp.
Automatic USB detection accepts native Espressif `303A:1001`, deduplicates `tty`/`cu`
and requires one candidate. Ambiguity leaves settings/radio unchanged; concurrent
setup attempts are rejected. Only a verified BroGlow `INFO` reply permits `PAIR`;
the app never flashes firmware automatically.

Automatic setup attempts each USB attachment once. On failure, previous lamp settings
return; use **Add lamp** to retry, or unplug for at least **five seconds** and reconnect.
Verified USB identities are saved in `usb_lamps`. Previously added lamps keep their
selected transport and enable setting; forgotten lamps require manual **Add lamp**.
Failures and progress appear under **Lamps**. Windows/Linux keep their USB behavior;
automatic Bluetooth setup is macOS-only.

Setup pauses lamp controls, waits up to **eight seconds** for the worker, exchanges
`INFO`/`PAIR` at **115200 baud**, releases serial and scans **eight seconds**.
Busy clients cause errors, not takeover. Failure restores prior lamp settings while
preserving concurrent unrelated edits; closing the popover does not stop setup/recovery.
USB assistance still uses Just Works and requires macOS consent.

Repeat for each lamp: IDs are added, existing bonds retained. If `INFO` is unsupported,
use [prebuilt](#flash-prebuilt-c3-firmware) or [source](#flash-and-recover-usb-upload)
flashing on the identified board within the requested scope.

### Pair without a USB data cable

BOOT is **GPIO9, active LOW**. Use it after normal startup, releasing once before
holding. Holding BOOT during power-on/RESET enters the ROM loader instead.

| Action after normal boot | Result |
| --- | --- |
| No saved bonds | Automatic 60-second pairing window |
| Hold BOOT three seconds, release | 60-second window; retain bonds |
| Continue holding ten seconds, release | Request deletion of all board bonds, disconnect, open 60-second window |
| USB `broglow ble pair` | Same window after identification; retain bonds |
| Window expires | Reject new pairing; bonded Macs may reconnect |

**LE Secure Connections Just Works** uses encrypted GATT writes and persistent
bonds, but **has no MITM authentication**, PIN or numeric comparison. Pair the
intended lamp during its physical window. Saved UUIDs select targets; board/macOS
keys are the bond. [Espressif security API](https://docs.espressif.com/projects/esp-idf/en/v4.4.7/esp32c3/api-reference/bluetooth/esp_gap_ble.html).

Power the lamp from a USB brick. Release the worker/radio before discovery:

```bash
broglow ble off
# Wait at least three seconds, then open pairing with BOOT as described above.
broglow ble scan
```

The scan lasts **eight seconds**. Power only the intended lamp or compare scans
with it off/on. Copy its exact host-specific CoreBluetooth UUID, replacing placeholders:

```bash
broglow ble add <HOME_ID_FROM_SCAN>
broglow ble status
broglow ble list
```

`add` enables all remembered lamps. Approve Bluetooth access and the system pairing
alert promptly: its observed timeout was **about 30 seconds**, despite the board's
**60-second** window and host write budget. Reopen pairing/retry if expired.
The first encrypted write is `N`; after acknowledgement the worker reads/sends the
latest aggregate. A **250 ms** first-write flash confirms each connection.
An advertisement or low-level connection is not an acknowledged write.

Repeat off → three-second wait → BOOT → scan → add for an office lamp. Both IDs
remain saved: `Lamps 1/2` is normal with one elsewhere; expect `BLE 2/2` with both
nearby. Each has independent writes/freshness, so one cannot block or mask the other.

`~/.claude/broglow/config.json` stores `ble_peripherals` and `led_transport: "ble"`.
Use **Lamps → row → Forget lamp**, or:

```bash
broglow ble remove <ID_TO_FORGET>
```

Forget works while absent/disabled, saves before removing the row, and leaves the
list intact on save error. It stops reconnection without erasing either side's bonds.
Removing the last ID disables BLE; `ble off` retains IDs, and `ble usb` retains them
while selecting USB. Re-add via USB setup or the scanned ID.

### Replace a Mac or recover mismatched bonds

Disable the old Mac's worker. Install BroGlow on the new Mac, open the board's
three-second pairing window, scan and add its ID. CoreBluetooth UUIDs are
host-specific; do not copy the old Mac's target.

For repeated bond failures, hold BOOT **ten seconds after startup** to clear board
bonds. Also Forget/Remove a stale macOS Bluetooth entry if shown, then rescan/add
during the new window. `ble remove` only changes BroGlow settings, not macOS keys.
Verify recovery by successful write and `authenticated`/`bonds` diagnostics.
Erasure is asynchronous: `bond_reset=requested` is not completion.

### Pairing feedback and diagnostics without external LEDs

At startup, all controlled LEDs breathe twice. Pairing chases the three colors;
unpaired lamps blink; a bonded disconnected lamp gives a dim pulse every 30 seconds.
The first encrypted write produces one short confirmation flash. Fresh session
status then wins. The onboard LED uses one/two/three pulses for green/yellow/red.

`?` over the identified USB serial port reports state, PWM duties, pairing/bonds
and heartbeat age. These are commanded outputs, not proof of visible light.
Detailed pattern timing, `INFO`/`PAIR` and diagnostic commands are in the
[firmware reference](firmware.md#pairing-feedback-and-diagnostics-without-external-leds).

## Everyday operation and status

Toggle the selected transport with TUI `l` or **Lamps → Enable/Disable lamp**.
USB enabling needs an attached board; BLE toggles all saved lamps without USB.
`ble off` retains IDs; `ble add <SAVED_ID>` re-enables without duplicates.

In **Lamps**, expand a row (click or Enter/Space) for controls, **Name** and
**Forget lamp**; open **Connection details** for **Device ID** and **Diagnostic**.
Names are local, up to **40 Unicode characters**;
blank restores `Lamp 1`, `Lamp 2`, … . Save while absent/disabled without changing
advertised name or bonds; CLI: `broglow ble rename <SAVED_ID> "Office"` (`""` clears).
**Troubleshooting** contains service/setup details; Back/Escape returns to sessions.
TUI `d` toggles details; arrows or `j`/`k` scroll, Escape returns.

**Identify** scans only the selected connected Bluetooth lamp's **red → yellow → green**
LEDs **three times**, one LED at a time at **100%**, **100 ms per LED** (**0.9 seconds**
total), then restores its latest status and saved brightness, even if saved at 0%.
It neither enables a lamp nor saves preferences; absent/paused/stale lamps cannot be
identified. Update the board to matching firmware for this scan: older firmware
silently ignores the new mode even if it acknowledges the write. Updating the app
does not flash the board; follow the identified-board backup/flash procedure.

| Status | Meaning / action |
| --- | --- |
| `disabled` | Enable in TUI/Lamps |
| `searching` | Check remembered IDs and power |
| `connecting` | Connection/service discovery in progress |
| `connected` | At least one lamp has a recent successful write; inspect per-lamp count/status |
| `error` | Read permission, Bluetooth, timeout or GATT detail |
| `unavailable` | Worker status missing/stale; check the installed service |

The green connection dot requires a worker write, not advertisement/USB presence.
`1/2` is normal if one lamp is elsewhere; ACK does not prove visible LEDs.

Red wins for any session needing input. **Yellow mode**: `any_inactive` makes any
idle session yellow; `active_wins` stays green while another works. No live sessions
means fresh `N`/dark after the connection flash. Mode changes need no reconnect.

Each lamp receives the latest aggregate on connection/change and every **two seconds**.
Reconnect never replays an old queued color. Disabling attempts `N`, disconnects
and stops scans/retries; the **ten-second** timeout clears undelivered stale status.
`N` cannot distinguish disabled from absent: disconnected bonded lamps eventually
resume their waiting pulse, so disabling does not guarantee permanent darkness.

### Brightness control

In **Lamps**, expand a lamp and set **Light brightness: 0–100%**. **All lamps**
sets every saved lamp, clears overrides and becomes the default for new lamps.
Default is 100%; existing shared levels are retained. Mac settings are
`led_brightness` and optional per-ID `lamp_brightness` in
`~/.claude/broglow/config.json`; absent lamps receive their saved value on reconnect.

Sliders preview all three lights on selected connected lamps and return to status
**two seconds** after the last preview. Holding the pointer renews preview;
keyboard changes also preview. Release/cancel, leave Lamps or close the popup to stop
renewal. Only a committed change saves; closing/canceling alone does not. Preview
never enables a lamp or writes preferences. Saved changes work while absent/disabled;
Forget removes that lamp's name and brightness override.

```bash
broglow ble brightness 40 --id YOUR-SAVED-UUID  # one remembered lamp
broglow ble brightness 60                     # all lamps; clear overrides
```

CLI commands save only: no enable or preview. Changes reach connected lamps without
reconnecting. Brightness scales external and onboard blue LEDs, including indications,
not blink timing or the red power LED. **0% leaves BLE/heartbeats running with controlled
LEDs off**; previews can temporarily show another level. PWM is not calibrated
perceived brightness; eight-bit rounding merges very low levels.

The board stores brightness in RAM and resets to **100%** on power/reset. The Mac
sends the saved value before its first status/connection flash; earlier startup
indications use 100%. Brightness, preview and Identify do not refresh the **ten-second**
status deadline. Brightness changes do not restart patterns. Disconnect, lost
authentication or stale status cancels preview and Identify.

BroGlow requires its matching firmware with writable status and brightness
characteristics; incomplete firmware is rejected.
USB/serial does not send brightness; switching from BLE retains the RAM level.
Power-cycle for 100%; the next BLE connection restores the Mac preference.

### Preserve or return to USB

```bash
broglow ble usb
```

This selects USB with mirroring disabled and preserves existing USB settings and
saved BLE IDs. Connect the identified board, then enable via `l` or Lamps.
With several Espressif devices, set `led_port` to its current identified path while
preserving other config keys; VID alone is insufficient.

For a foreground USB-only diagnostic:

```bash
broglow ble off
broglow led --port "$BOARD_PORT"
```

Stop with Ctrl-C before re-enabling the tray worker. Connected BLE ignores serial
LED commands; read-only diagnostics remain available.

## BLE contract and firmware behavior

BLE uses acknowledged encrypted writes, a **two-second heartbeat** and **ten-second
stale timeout**. Connected BLE owns LED control; serial colors work when disconnected.
Flash updates over USB. UUIDs, payloads, security and diagnostics are defined in the
[firmware reference](firmware.md#ble-contract-and-firmware-behavior).

### GATT cache after a firmware upgrade

Firmware requests Service Changed to expose new characteristics without erasing
bonds. If brightness is missing after upgrade, reconnect and check capabilities
before resetting bonds. Review the [SDK implementation](firmware.md#gatt-cache-after-a-firmware-upgrade)
when changing the pinned toolchain.

## Troubleshooting and service recovery

| Symptom | Check / recovery |
| --- | --- |
| No port / upload fails | Data cable, physical C3/4 MB identification, closed serial clients, BOOT/RESET and fresh port list; never guess. |
| `Run broglow install` / wrong CLI | Use `~/.cargo/bin/broglow install`; check PATH and app bundle. |
| Bluetooth busy | `broglow ble off`, wait three seconds, close other scans, retry. |
| Permission error / Bluetooth off | Enable Bluetooth, allow BroGlow in Privacy & Security → Bluetooth, restart service. |
| No lamps / repeated searching | Check power, firmware, range and saved ID. Disable controlling clients (connected lamps may not advertise); compare scans off/on. |
| Pairing/write rejected | BOOT three seconds after normal boot; pair within 60 seconds, promptly approve macOS (~30 seconds observed). Then try [bond recovery](#replace-a-mac-or-recover-mismatched-bonds). |
| BOOT stops advertising | Release BOOT, tap RESET, wait for normal startup before holding for pairing. |
| `1/2` connected | Normal with one lamp elsewhere. |
| Missing characteristic / brightness update required | Confirm identity/capabilities; upgrade the identified C3 via the documented flash path. |
| Connected but dark/wrong color | Check 0% brightness, live sessions and Yellow mode; disable BLE and run the wiring test. ACK is not electrical proof. |
| Faint LED / one color dark | Disconnect power; inspect polarity, GPIO, resistor, contacts and shared GND. Use [standalone test](#standalone-led-test-firmware); logic readback cannot prove voltage/current or board damage. |
| No local Codex sessions | [Trust six hooks](#connect-local-codex-sessions), then send a new local-task prompt. |
| Serial colors ignored | Disconnect BLE. |
| Light clears during sleep | Ten-second stale timeout; wake must reconnect/resend. |
| `unavailable` / no tray | Check launchd/logs and recover the existing service, never a second tray. |

Restart without reinstalling hooks:

```bash
launchctl kickstart -k "gui/$(id -u)/io.github.broglow.menubar"
broglow ble status
```

Read `~/.claude/broglow/menubar.log` and `menubar.err`. For a missing/broken bundle
or changed binary, repeat the [prebuilt](#install-a-prebuilt-package) or
[source](#build-from-source) install from the stable CLI path. The installer stages
and verifies replacement bundles; retain any `~/Applications/.BroGlow.previous.*.app`
until recovery. Restore only affected BroGlow settings, preserving later unrelated edits.

`broglow uninstall` removes service, BroGlow hooks, bundle and state directory;
do not use it for routine troubleshooting. For intentional removal, run
`broglow ble off` first, then `broglow uninstall`.

## Repeatable checks and results

Run from the source checkout:

```bash
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
node --test tests/js/*.test.mjs
python3 firmware/esp32-c3-supermini/tests/check.py
~/.platformio/penv/bin/pio run -d firmware/esp32-c3-supermini -e esp32-c3-supermini -e led-test
```

CI also checks C++ with clang-format **23.1.1** and host builds/tests on
Linux/macOS/Windows. Firmware tests mock GPIO, clock, serial and Bluetooth; they
cannot establish wiring/radio behavior. See `site/README.md` and `site/guide/en/releasing.md`
for website/release checks.

The electronics and Mac-service observations below predate the BroGlow rebrand
and were recorded with ClawLight. Repeat
physical acceptance with matching BroGlow firmware before claiming those results.

| Area | Evidence and limits |
| --- | --- |
| Apple Silicon Mac | Signed service, USB pairing, bonded reconnect, status and brightness exercised. |
| LEDs | Check all colors and brightness on each assembly. |
| Multiple lamps | Isolation tests pass; simultaneous operation with two physical boards remains unverified. |
| Intel Mac / Windows / Linux | CI/build targets, no physical BLE or fresh-install claim. BLE is macOS-only. |
| Distribution | Local package/site checks precede publication; public downloads, Pages and fresh-Mac Gatekeeper require release acceptance. |
| Enclosure | Fit requirements and assembly checks are in the print guide; CAD/slicer reports are included in the print package. |

Keep dated logs, board IDs and private recovery paths out of this manual.

### Physical and background-service acceptance

For each supported Mac/board combination, record exact app/firmware versions and:

1. One signed tray process, fresh ACK, USB setup, Bluetooth consent and reconnect
   without USB data; verify restart/login and permission-denial recovery.
2. Real-session working/idle/needs-help/off, both Yellow modes, all LED colors and brightness.
3. With board power retained, interrupted updates clear session color within **ten seconds**;
   restore the service immediately.
4. Board reset, Mac sleep/wake, Bluetooth off/on, range loss, disable/re-enable;
   an absent lamp must not block another.
5. On disposable test bonds: physical pairing window, expired-window refusal and bond reset.
6. Enclosure PCB/USB fit, LED mounting and screw engagement against the print guide.

Builds do not replace these checks. Never reflash a working board for an app/docs update.
