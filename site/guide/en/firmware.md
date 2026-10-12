---
title: Firmware reference
---

# Firmware protocol and diagnostics

For installation, wiring and normal operation, use [the setup guide](hardware.md).
This reference describes the C3 firmware and its host protocol.

## Pairing feedback and diagnostics without external LEDs

The table shows each mode on the three external LEDs and on the onboard LED, so a
board without external LEDs can still be diagnosed. The configured onboard LED is a
single-color, active-low GPIO8 LED; it encodes session colors by pulse count, not
by changing color. The [SuperMini manufacturer guide](https://www.nologo.tech/en/product/esp32/esp32c3SuperMini/esp32C3SuperMini.html)
documents that LED, and [Apache NuttX's SuperMini reference](https://nuttx.apache.org/docs/latest/platforms/risc-v/esp32c3/boards/esp32c3-supermini/index.html)
describes GPIO8's polarity and reset strapping constraint. Board variants differ:
this build does not detect whether an LED is fitted. A separate power LED is not
controlled by this firmware. See the
[current verification limits](hardware.md#repeatable-checks-and-results).
GPIO8 is driven only after normal boot; do not add external
circuitry that pulls this strapping pin low during reset.

| Mode / condition | External red/yellow/green LEDs | Onboard LED |
| --- | --- | --- |
| `startup`: first three seconds after firmware setup | All three breathe together for two 1.5-second rise-and-fall cycles | Same fades, with its peak scaled to 32/255 |
| `preview`: brightness slider on a connected lamp | All three steadily show the preview percentage; returns to status two seconds after the last preview | Same preview percentage applied to its 32/255 base |
| `identify`: Identify on a connected lamp | Red → yellow → green three times, one LED at a time, 100 ms per LED (0.9 seconds total); the app requests 100% | One 40 ms pulse per step at 32/255, scaled by the requested percentage |
| `connected`: first valid status write (`N`, `R`, `Y` or `G`) of each BLE connection; none if it arrives during startup breathing | All three on once for 250 ms | On for the same 250 ms at 32/255 brightness |
| `status`: fresh `R`, `Y` or `G` | Corresponding LED steadily on | Repeating groups: green = one, yellow = two, red = three 100 ms pulses, spaced 250 ms apart; one group every three seconds at 32/255 |
| `status`: fresh `N` | Off | Off |
| `pairing`: window open, no fresh status | Red → yellow → green, 250 ms per step | One 100 ms pulse per step at 32/255 |
| `unpaired`: no board bonds, window closed, no fresh status | All three one second on, one second off | Same cadence at 32/255 |
| `waiting`: board has bonds, BLE disconnected, window closed, no fresh status | All three pulse for 100 ms at 8/255 every 30 seconds; first pulse after 30 seconds in this mode | Same quiet pulse |
| `stale`: BLE connected, no fresh status, and either confirmed status expired, or the board has bonds and the window is closed | Off | Off |

The duty values above are at **100% lamp brightness**. The saved setting scales
every controlled output, including startup, connection and quiet waiting indications;
preview temporarily uses the slider's percentage instead; Identify uses its requested
percentage (100% from the app). At 0% that selected
level is dark; the board's red power LED is unchanged. See [brightness control](hardware.md#brightness-control).

The firmware picks the first matching mode in this order:

1. Startup breathing: once per boot, for three seconds. Status and pairing
   continue in the background.
2. Identify or preview: on a confirmed, authenticated connection with fresh status.
   Disconnect, lost authentication or stale status cancels it.
3. Connection flash: 250 ms after the first valid status write of a connection.
   If that write arrives during startup breathing, no flash follows; startup already
   confirms power. Further `N`/`R`/`Y`/`G` writes update the stored status without
   canceling or restarting the flash. Disconnect, lost authentication or status
   expiry cancels it, so even a delayed first write cannot light the LEDs past the
   ten-second stale deadline.
4. Fresh status, including serial commands and `N`.
5. `stale` on a confirmed connection whose status expired, even if the pairing
   window is open.
6. `pairing`, then `unpaired`, then `stale` (connected), then `waiting`.

These patterns do not change the one-byte protocol or the ten-second status timeout.

Outputs use 2 kHz, eight-bit PWM. In `platformio.ini`, `LED_RED_PIN=0`,
`LED_YELLOW_PIN=1` and `LED_GREEN_PIN=3` select the external LED pins (GPIO0..10,
except 2, 8 and 9), and `STATUS_TIMEOUT_MS=10000` sets the stale timeout (must exceed
2000 ms). `BUILTIN_LED_PIN=8` and
`BUILTIN_LED_ACTIVE_LOW=1` select the usual onboard LED; set the pin to `-1` to
disable that output on a different board. `BUILTIN_LED_BRIGHTNESS=32` controls its
normal pulses, and `WAIT_LED_BRIGHTNESS=8` controls quiet waiting pulses on all
four outputs before the runtime brightness percentage is applied. Set
`WAIT_LED_BRIGHTNESS=0` to silence those quiet pulses. Rebuild
and flash the identified board after changing these options; PWM values are
commanded duty levels, not measured brightness.

With USB serial connected, the firmware prints these lines:

- At boot: `gatt_change_manual=<0|1> result=<n>`, then
  `lamp=BroGlow C3 XXXX security=sc-bonded button=BOOT`.
- `pairing=open duration_ms=60000` and `pairing=closed`.
- `authentication=ok reason=0` or `authentication=failed reason=<n>`, where `n` is
  the Bluetooth SMP failure code.
- `bond_reset=requested count=<n>` or `bond_reset=error` after a 10-second BOOT hold.
  `requested` means deletion was requested, not confirmed.

A `?\n` request returns a diagnostic line.
This read-only diagnostic is allowed while BLE owns the lamp; LED-setting serial
commands remain ignored until BLE disconnects.

Set `BOARD_PORT` to the identified board as in
[Build and identify the board](hardware.md#build-and-identify-the-board). Only one
program can open the port. If BroGlow mirrors to this board over USB, run
`broglow ble off` and wait three seconds (`broglow ble on` restores it); close
foreground `broglow led` and other serial monitors. Run from
the source checkout or the extracted package root:

```bash
uv run --frozen python - "$BOARD_PORT" <<'PY'
import sys
import time
import serial

with serial.Serial(sys.argv[1], 115200, timeout=0.5, write_timeout=2) as port:
    port.rts = False
    port.dtr = True
    time.sleep(0.3)
    port.reset_input_buffer()
    port.write(b"?\n")
    port.flush()
    until = time.monotonic() + 3
    while time.monotonic() < until:
        line = port.readline().decode(errors="replace").strip()
        if line:
            print(line)
        if line.startswith("state="):
            break
PY
```

The line reports stored session `state`, commanded `red/yellow/green` on/off flags, `owner`,
`connected`, `authenticated`, `pairing`, `bonds`, `age_ms` (board uptime before the
first status), and valid-state
`accepted` count. It also reports `mode`, `red_pwm`, `yellow_pwm`, `green_pwm`,
`builtin_pwm`, `builtin_pin`, and `brightness` (current percentage; volatile, resets
to 100 on reboot). The last four fields, `gatt_change_manual`, `gatt_change_requests`,
`gatt_change_request` and `gatt_change_result`, belong to the
[GATT cache check](#gatt-cache-after-a-firmware-upgrade).
During `mode=preview` or `mode=identify`, the duties use the temporary requested percentage; the
`brightness` field and stored session `state` continue to describe the underlying settings.

On/off flags mean a nonzero commanded duty, not pin readback. PWM values are 0..255
light-on duty after brightness scaling and before onboard active-low inversion, so
`state=N` can coexist with nonzero duties during startup, preview, Identify, setup
or the connection flash. `authenticated=1` means
the BLE link reached encrypted access; `age_ms` and `accepted` help verify
two-second heartbeat delivery. None of these fields proves wiring, current or
visible light. After diagnostics,
move to brick-only power if testing operation with no USB data connection.

For setup, exact `INFO\n` (or CRLF) returns the identity without changing pairing,
LEDs or timeout state. `PAIR\n` opens/renews the window and returns the same
identity; it does not delete bonds or disconnect the current central. The CLI
sends `INFO` before `PAIR`. Example reply (identity varies by board):

```text
broglow_pair identity=001122334455 service=7d2ea28a-f7bd-485a-bd9d-92ad6ecfe93e
```

These setup commands work while BLE owns the LED outputs; arbitrary serial color
commands still do not override that ownership. The advertised identity is the six
base-MAC bytes, not the possibly changing over-the-air private Bluetooth address.

## BLE contract and firmware behavior

| Item | Value |
| --- | --- |
| Name | `BroGlow C3` plus a four-hex-digit board suffix |
| Advertisement | Full service UUID and manufacturer data `0xffff` followed by the six base-MAC bytes (the `INFO` identity) |
| Service UUID | `7d2ea28a-f7bd-485a-bd9d-92ad6ecfe93e` |
| Writable status characteristic | `7d2ea28b-f7bd-485a-bd9d-92ad6ecfe93e` |
| Writable brightness characteristic | `7d2ea28c-f7bd-485a-bd9d-92ad6ecfe93e` |
| GATT operation | Write with response |
| Status payload | Exactly one ASCII byte: `R`, `Y`, `G`, or `N`; no newline |
| Brightness payload | Exactly one binary byte, percentage 0..100; not ASCII digits. Board default 100; volatile until reset. |
| Preview payload | On the same encrypted brightness characteristic: two binary bytes `[percent, 1]`, with percent 0..100. Temporarily overrides all controlled lights for two seconds from receipt; does not change saved brightness. |
| Identify payload | On the same encrypted brightness characteristic: two binary bytes `[percent, 2]`, with percent 0..100. Scans red → yellow → green three times at 100 ms per LED (0.9 seconds from receipt), then restores the latest status and saved brightness. The app sends `[100, 2]`; no preferences change. |
| Host heartbeat | Two seconds |
| Board stale timeout | Ten seconds since the last valid status; clears session color, then [indications](#pairing-feedback-and-diagnostics-without-external-leds) follow connection/bond state |
| Ownership | Connected BLE client; serial only while BLE is disconnected |
| Security | LE Secure Connections Just Works bonding; both writable characteristics require encryption. New pairing is admitted only while the pairing window is open. |
| Pairing window | 60 seconds. Opens at boot when the board has no bonds, on serial `PAIR`, or when BOOT (GPIO9) is held three seconds after normal boot. Holding BOOT ten seconds requests deletion of all bonds and reopens the window. See [Pair without a USB data cable](hardware.md#pair-without-a-usb-data-cable). |

After discovery, the host writes brightness before its initial `N`/status write,
so the firmware uses the saved level for its first connection indication.
Later percentage changes are written on the existing connection. If the
writable brightness characteristic is absent, the host does not connect and
reports an error: the lamp needs firmware with both the status and the
brightness characteristics.

The UI throttles slider preview changes to about 80 ms and renews a held pointer every
500 ms. Identify sends one request for the selected connected lamp. The host retains
only the latest event per target (or all-lamps request) for two seconds for both slider
preview and Identify. Each connection uses the newer matching targeted
or all-lamps event; a targeted request cannot affect a different lamp. The latest
preview or Identify request replaces the previous one. Normal heartbeats do not renew either.
Firmware requires a confirmed authenticated connection and fresh status, and
discards an expired or previous-connection request.

Older firmware silently ignores `[percent, 2]`, even if it acknowledges the write
and supports slider previews. The scan requires matching firmware; updating the app
does not flash the board. Back up and flash the identified board as in
[Flash and recover USB upload](hardware.md#flash-and-recover-usb-upload), or use
[Flash prebuilt C3 firmware](hardware.md#flash-prebuilt-c3-firmware).

Empty or multi-byte **status** payloads, unknown status bytes, brightness above
100, and brightness payloads other than one byte, `[percent, 1]` or `[percent, 2]` are ignored.
Brightness, preview and Identify do not refresh the status timeout. The GATT stack's
write response acknowledges receipt; it is not a custom
application validation response and never proves electrical LED operation. All
GPIO changes and timeout decisions happen in the main loop; callbacks transfer
state through synchronized FreeRTOS mailboxes. The timeout uses wrap-safe unsigned
clock arithmetic. Disconnect restarts advertising.

### GATT cache after a firmware upgrade

Adding the brightness attribute can leave a bonded Mac using its cached
status-only service. The firmware requests a Service Changed indication for the
connecting peer from `ESP_GATTS_CONNECT_EVT`, using
`BLEDevice::setCustomGattsHandler` and the public
`esp_ble_gatts_send_service_change_indication(gatts_if, remote_bda)` API
(see the [Espressif GATTS API](https://docs.espressif.com/projects/esp-idf/en/v4.4.7/esp32c3/api-reference/bluetooth/esp_gatts.html)
and the [IDF 4.4.7 indication dispatch and mode setter](https://raw.githubusercontent.com/espressif/esp-idf/v4.4.7/components/bt/host/bluedroid/bta/gatt/bta_gatts_api.c)).
This is intended to trigger rediscovery while preserving the existing bond.

The SDK bundled with pinned Arduino 2.0.17 defaults to automatic Service Changed
mode, which rejects this explicit request. Startup therefore selects manual mode
with `BTA_GATTS_SetServiceChangeMode(1)`. Its `uint8_t` signature was checked in
IDF 4.4.7 source and its export in the actual C3 `libbt.a`; it is a dependency of
this pinned SDK, not a portable Arduino API. Before changing the toolchain,
recheck the exported symbol, its signature,
manual-mode value and callback behavior, then repeat a bonded firmware-upgrade
test.

To check it: `gatt_change_manual=1` at boot and in `?` confirms manual mode;
`gatt_change_requests` counts connections that requested the indication;
`gatt_change_request=0` means the indication was queued; `gatt_change_result` is the
stack's send status (`-1` until reported). None of these proves that macOS refreshed
its cache. Do not erase bonds as a routine brightness upgrade step.
