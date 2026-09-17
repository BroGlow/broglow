<a href="../">Back to the project</a>

# Build your lamp { #build }

Already assembled? [Install ClawLight and connect the lamp](installation.md).

## Choose your parts { #parts }

[Download models · ZIP ↓](../../models/clawlight-print-pack.zip){ download }

| Part | What to look for |
| --- | --- |
| Board · 1 | SuperMini form factor with Bluetooth LE. ESP32-C3 SuperMini recommended. |
| LEDs · 3 | One red, one yellow, one green. The supplied enclosure is designed for 5 mm LEDs. |
| Resistors · 3 | 330 Ω, one for each LED. |
| Wires | For the LEDs and a shared ground. |
| USB cable · 1 | A data cable that fits your board, for initial setup and power from your Mac. |
| USB adapter · optional | For placing the lamp away from your Mac. You can keep using the Mac’s USB port instead. |
| Enclosure · optional | Use `clawlight-print-pack.zip`. Try the fit-test pieces first. |
| Case hardware | For the supplied enclosure: 5 DIN 7982 ST2.2 × 6.5 mm countersunk screws, 2 cable ties (2.5 mm wide, at most 1 mm thick), and plastic-compatible glue. |

Using another SuperMini with Bluetooth LE? Adapt the firmware and pin assignments for its chip,
and check the enclosure fit. The included firmware targets ESP32-C3.

## Wire up the LEDs { #wiring }

Disconnect USB power. Connect each pin through its own 330 Ω resistor to the LED’s anode (+).
All cathodes (−) go to GND; do not connect the LEDs to 5V.

Wiring for the recommended ESP32-C3 build: use the pads on the GND side and skip
GPIO2, a boot-strapping pin. Check the pin assignments for other boards.

- Red: GPIO0 → 330 Ω → LED (+)
- Yellow: GPIO1 → 330 Ω → LED (+)
- Green: GPIO3 → 330 Ω → LED (+)
