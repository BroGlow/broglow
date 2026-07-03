#!/usr/bin/env bash
#
# flash.sh — build the clawlight firmware for the Seeed XIAO ESP32-C6 and flash
# whichever board is currently plugged in, in one command.
#
# Usage:
#   scripts/flash.sh                      Build the XIAO firmware and flash the
#                                         connected board.
#   scripts/flash.sh --monitor            ...then open a serial monitor.
#   scripts/flash.sh --port /dev/cu.usbmodem101
#                                         Flash a specific serial device.
#   scripts/flash.sh --elf path/to/fw.elf Flash a prebuilt ELF (skip the build).
#   scripts/flash.sh --firmware-dir DIR   Point at the clawlight-firmware repo
#                                         (default: sibling of this repo, or
#                                         $CLAWLIGHT_FIRMWARE_DIR).
#   scripts/flash.sh --connect-timeout N  Seconds to wait at "Connecting…" before
#                                         prompting for download mode (default 15).
#
# If clawlight is driving the LEDs it holds the serial port open, and a serial
# port is exclusive — so this pauses the LEDs (flips `led_enabled` off so the
# menu bar daemon lets go), flashes, then restores the setting, exactly like
# `clawlight update` does.
#
# If the board isn't in download mode, espflash just sits at "Connecting…". This
# caps that wait (--connect-timeout / $FLASH_CONNECT_TIMEOUT), then prints how to
# enter the bootloader by hand and waits for you to press Enter to retry — so a
# first flash usually goes: run it, do the BOOT/RESET dance, Enter.
#
# Requires: espflash (`cargo install espflash`) and, unless --elf is given, a
# checkout of the clawlight-firmware repo.

set -euo pipefail

CHIP="esp32c6"
TARGET="riscv32imac-unknown-none-elf"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

FIRMWARE_DIR="${CLAWLIGHT_FIRMWARE_DIR:-$REPO_DIR/../clawlight-firmware}"
CONFIG="$HOME/.claude/clawlight/config.json"
PORT=""
MONITOR=0
ELF=""
CONNECT_TIMEOUT="${FLASH_CONNECT_TIMEOUT:-15}"   # seconds allowed at "Connecting…"
LED_WAS_ENABLED=0          # set to 1 once we've paused LEDs, so we restore them
ESP_PID=""                 # PID of a running espflash, so cleanup can stop it

die() { printf 'flash.sh: error: %s\n' "$*" >&2; exit 1; }

# Flip clawlight's `led_enabled` flag without disturbing the rest of the config
# (led_port etc.). The file is machine-written one field per line, so a targeted
# substitution on the led_enabled line is safe; write via temp + rename so a
# reader never sees a half-written file.
set_led_enabled() {
    local val="$1" tmp                       # val: true | false
    tmp="$CONFIG.flash.$$.tmp"
    sed -E "s/(\"led_enabled\"[[:space:]]*:[[:space:]]*)(true|false)/\1$val/" \
        "$CONFIG" > "$tmp" && mv "$tmp" "$CONFIG"
}

# Runs on any exit (including Ctrl-C, routed through the INT/TERM traps below):
# turn the LEDs back on if we were the one who turned them off, so a failed or
# interrupted flash can't leave them silently disabled.
restore_leds() {
    if [ "$LED_WAS_ENABLED" -eq 1 ] && [ -f "$CONFIG" ]; then
        set_led_enabled true
        LED_WAS_ENABLED=0
        echo "==> Re-enabled clawlight LEDs; the daemon reconnects on its next scan."
    fi
}

# Single EXIT handler (Ctrl-C is routed here via the INT/TERM traps): stop any
# espflash we backgrounded, then re-enable the LEDs.
cleanup() {
    if [ -n "$ESP_PID" ] && kill -0 "$ESP_PID" 2>/dev/null; then
        kill "$ESP_PID" 2>/dev/null || true
    fi
    restore_leds
}
trap cleanup EXIT
trap 'exit 130' INT TERM

# Make clawlight let go of the serial port before we flash. Disabling led_enabled
# makes the always-on menu bar daemon release it; a foreground `clawlight led`
# has to be stopped by hand, which we detect and point out.
free_serial_port() {
    local port="$1" waited=0
    if [ -f "$CONFIG" ] && grep -Eq '"led_enabled"[[:space:]]*:[[:space:]]*true' "$CONFIG"; then
        echo "==> Pausing clawlight LEDs to free the serial port…"
        set_led_enabled false
        LED_WAS_ENABLED=1
    fi
    # Wait for whatever held the port to actually release it (daemon polls the
    # config roughly twice a second). Only possible when we know the device.
    if [ -n "$port" ] && command -v lsof >/dev/null 2>&1; then
        while lsof "$port" >/dev/null 2>&1; do
            if [ "$waited" -ge 8 ]; then
                printf 'flash.sh: %s is still in use after pausing LEDs.\n' "$port" >&2
                printf '  A foreground `clawlight led` is probably holding it — stop it with:\n' >&2
                printf '    pkill -f "clawlight led"\n' >&2
                die "serial port busy"
            fi
            sleep 1
            waited=$((waited + 1))
        done
    elif [ "$LED_WAS_ENABLED" -eq 1 ]; then
        # Unknown device or no lsof: give the daemon a beat to notice the flag.
        sleep 3
    fi
}

# Printed when espflash can't reach the chip — almost always the XIAO's flaky
# auto-reset, which lands it here on the first flash. The manual BOOT/RESET dance
# forces it into ROM download mode.
bootloader_help() {
    cat >&2 <<'HELP'

flash.sh: espflash couldn't sync with the board.

It stalled at "Connecting…" (or you saw `espflash::timeout`) — the XIAO didn't
enter download mode on its own. Put it there by hand:

  1. Press and HOLD the BOOT button (labeled B, next to the USB-C port).
  2. While holding BOOT, tap the RESET button (labeled R) once.
     (No luck? Unplug and replug the USB-C cable while still holding BOOT.)
  3. RELEASE BOOT.

The board stays in download mode until its next reset, so there's no rush.
espflash resets it back into the app after a successful flash.

Still timing out? Use a DATA USB-C cable (not charge-only), and leave only the
one board plugged in (or pass --port).
HELP
}

# Block until the user presses Enter, reading from the terminal even if the
# script's stdin is redirected. Returns non-zero when there's no terminal to
# prompt on (e.g. a non-interactive/CI run), so the caller can bail instead of
# looping forever.
wait_for_enter() {
    if [ -r /dev/tty ]; then
        read -r _ < /dev/tty
    elif [ -t 0 ]; then
        read -r _
    else
        return 1
    fi
}

# Output espflash prints once it's past "Connecting…" — the chip-info banner and
# the flashing steps. Seeing any of these means the sync succeeded, so we stop
# enforcing the connect timeout and let the flash run as long as it needs.
CONNECTED_RE='Chip type|Crystal frequency|MAC address|Flash size|Features|App/part|Segment|Erasing|Flashing|Writing|Uploading|Verifying'

# Run espflash but give it only $CONNECT_TIMEOUT seconds to get past
# "Connecting…". espflash has no such flag (3.3.0), and if it owns the terminal
# we can't see its progress — so capture its output to a log, stream that to the
# user, and watch for the connected banner. If the banner never shows in time the
# board almost certainly isn't in download mode: kill espflash and return 124 to
# trigger the manual-bootloader prompt. Returns espflash's own status otherwise.
run_espflash() {
    local log connected=0 waited=0 printed=0 total status=0
    log="$(mktemp "${TMPDIR:-/tmp}/flashsh.XXXXXX")"

    espflash "${flash_args[@]}" > "$log" 2>&1 &
    ESP_PID=$!

    while kill -0 "$ESP_PID" 2>/dev/null; do
        # Stream any newly completed lines to the terminal.
        total=$(awk 'END { print NR + 0 }' "$log")
        if [ "$total" -gt "$printed" ]; then
            awk -v p="$printed" 'NR > p' "$log"
            printed=$total
        fi

        if [ "$connected" -eq 0 ] && grep -Eq "$CONNECTED_RE" "$log"; then
            connected=1
        fi

        if [ "$connected" -eq 0 ] && [ "$waited" -ge "$CONNECT_TIMEOUT" ]; then
            kill "$ESP_PID" 2>/dev/null || true
            wait "$ESP_PID" 2>/dev/null || true
            ESP_PID=""
            awk -v p="$printed" 'NR > p' "$log"
            rm -f "$log"
            printf 'flash.sh: still stuck at "Connecting…" after %ss.\n' "$CONNECT_TIMEOUT" >&2
            return 124
        fi

        sleep 1
        waited=$((waited + 1))
    done

    wait "$ESP_PID" || status=$?
    ESP_PID=""
    awk -v p="$printed" 'NR > p' "$log"      # flush the tail (incl. a final line)
    rm -f "$log"
    return "$status"
}

usage() {
    # Print the top comment block (from the title line to the first blank line),
    # stripped of the leading "# " — no line-number coupling to keep in sync.
    awk 'NR >= 3 { if ($0 ~ /^#/) { sub(/^# ?/, ""); print } else exit }' "$0"
}

while [ $# -gt 0 ]; do
    case "$1" in
        -m|--monitor)      MONITOR=1; shift ;;
        -p|--port)         PORT="${2:-}"; [ -n "$PORT" ] || die "--port needs a value"; shift 2 ;;
        --elf)             ELF="${2:-}"; [ -n "$ELF" ] || die "--elf needs a value"; shift 2 ;;
        --firmware-dir)    FIRMWARE_DIR="${2:-}"; [ -n "$FIRMWARE_DIR" ] || die "--firmware-dir needs a value"; shift 2 ;;
        --connect-timeout) CONNECT_TIMEOUT="${2:-}"; case "$CONNECT_TIMEOUT" in ''|*[!0-9]*) die "--connect-timeout needs a whole number of seconds";; esac; shift 2 ;;
        -h|--help)         usage; exit 0 ;;
        *)                 die "unknown argument: $1 (see --help)" ;;
    esac
done

command -v espflash >/dev/null 2>&1 \
    || die "espflash not found. Install it with: cargo install espflash"

# Auto-detect the port when the caller didn't pin one, so a single plugged-in
# board flashes non-interactively. espflash otherwise prompts when several
# serial devices are present. macOS: /dev/cu.usbmodem*  Linux: /dev/ttyACM*.
if [ -z "$PORT" ]; then
    ports=()
    for dev in /dev/cu.usbmodem* /dev/ttyACM*; do
        if [ -e "$dev" ]; then ports+=("$dev"); fi
    done
    if [ "${#ports[@]}" -eq 1 ]; then
        PORT="${ports[0]}"
    elif [ "${#ports[@]}" -gt 1 ]; then
        printf 'flash.sh: multiple serial devices found — pass --port to choose:\n' >&2
        printf '  %s\n' "${ports[@]}" >&2
        die "refusing to guess between multiple boards"
    fi
fi

# Build the XIAO firmware unless the caller supplied a prebuilt ELF.
if [ -z "$ELF" ]; then
    [ -d "$FIRMWARE_DIR" ] \
        || die "firmware repo not found at $FIRMWARE_DIR (pass --firmware-dir or set CLAWLIGHT_FIRMWARE_DIR)"
    echo "==> Building clawlight-firmware for the Seeed XIAO ESP32-C6…"
    ( cd "$FIRMWARE_DIR" && cargo build --release )
    ELF="$FIRMWARE_DIR/target/$TARGET/release/clawlight-firmware"
fi
[ -f "$ELF" ] || die "firmware ELF not found: $ELF"

# Assemble the espflash invocation. The two-slot partition table is what lets
# later `clawlight update` serial-OTA pushes work, so include it when present.
flash_args=(flash --chip "$CHIP")
PART_TABLE="$FIRMWARE_DIR/partitions.csv"
if [ -f "$PART_TABLE" ]; then flash_args+=(--partition-table "$PART_TABLE"); fi
if [ -n "$PORT" ]; then flash_args+=(--port "$PORT"); fi
if [ "$MONITOR" -eq 1 ]; then flash_args+=(--monitor); fi
flash_args+=("$ELF")

# Free the port from the clawlight daemon just before flashing (kept as late as
# possible so the LEDs stay live during the slow build). restore_leds re-enables
# them on exit.
free_serial_port "$PORT"

# Flash, retrying interactively on failure: the XIAO usually needs a manual
# BOOT/RESET into download mode the first time. LEDs stay paused across retries
# (restore_leds re-enables them once, on exit) so we don't fight the daemon for
# the port between attempts.
attempt=1
while true; do
    echo "==> Flashing $ELF (attempt $attempt)"
    if [ -n "$PORT" ]; then echo "    port: $PORT"; else echo "    port: (espflash auto-detect)"; fi

    # --monitor wants an interactive TTY, so run espflash straight through there
    # (it enforces its own connect timeout). Otherwise wrap it so we can cap the
    # "Connecting…" wait and prompt for the bootloader ourselves.
    if [ "$MONITOR" -eq 1 ]; then
        if espflash "${flash_args[@]}"; then break; fi
    else
        if run_espflash; then break; fi
    fi

    bootloader_help
    printf '\n==> Put the XIAO in download mode (steps above), then press Enter to retry (Ctrl-C aborts)… ' >&2
    wait_for_enter \
        || die "espflash failed and there's no terminal to prompt on — re-run after entering download mode."
    attempt=$((attempt + 1))
done

echo "==> Done. The Seeed XIAO ESP32-C6 will reboot into the new firmware."
