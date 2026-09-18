<a href="../">Back to the project</a>

# Build your lamp { #build }

ClawLight includes an app, ESP32-C3 firmware and a printable enclosure.

Already assembled? [Install ClawLight and connect the lamp](installation.md).

## Choose your parts { #parts }

<!-- model-download -->

| Part | What to look for |
| --- | --- |
| Board · 1 | ESP32-C3 SuperMini; the project's firmware targets this board. |
| LEDs · 3 | One red, one yellow, one green. The enclosure uses 5 mm LEDs. |
| Resistors · 3 | One for each LED; use the values in the wiring diagram below. |
| Wires | For the LEDs and a shared ground. |
| USB cable · 1 | A data cable that fits your board, for initial setup and power from your Mac. |
| USB adapter · optional | For placing the lamp away from your Mac. You can keep using the Mac’s USB port instead. |
| Enclosure · optional | 1.0.0 release candidate; use the print package above. The lamp can operate without it. |

Verify the actual chip before flashing; the supplied firmware targets ESP32-C3 only.

## Wire up the LEDs { #wiring }

Disconnect USB power. Connect each pin through its own resistor to the LED’s anode (+).
All cathodes (−) go to GND; do not connect the LEDs to 5V.

Wiring for ESP32-C3 SuperMini: use the pads on the GND side and skip
GPIO2, a boot-strapping pin.

--8<-- "docs/hardware-setup.md:lamp-wiring"

These values balance the LEDs on the current lamp. For other LEDs, follow the
[resistor guidance](hardware.md#wiring-and-resistor-choice).
