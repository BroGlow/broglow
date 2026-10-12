<a href="../">Back to the project</a>

<a name="build"></a>

# Build your lamp

A three-LED status lamp (ESP32-C3 SuperMini with red, yellow and green LEDs) that
mirrors your agent sessions. The printable enclosure is optional.

Already assembled? [Install BroGlow and connect the lamp](installation.md).

<a name="parts"></a>

## Choose your parts

<!-- model-download -->

| Part | What to look for |
| --- | --- |
| Board · 1 | ESP32-C3 SuperMini; the firmware targets ESP32-C3 only. Before flashing, [identify the board](hardware.md#build-and-identify-the-board). |
| LEDs · 3 | 5 mm, one each red, yellow and green (the enclosure holes fit 5 mm). |
| Resistors · 3 | 330 Ω for red and yellow, 220 Ω for green; 1/8 W or 1/4 W. |
| Wires | For the LEDs and a shared ground. |
| USB cable · 1 | A USB-C data cable (not charge-only) for setup and power. |
| USB adapter · optional | macOS only: powers the lamp away from the Mac, which then talks to it over Bluetooth. |
| Enclosure · optional | Print package above: magnetic (recommended) or glued. PETG in orange and black. The lamp works without it. |
| Screws · 4 · enclosure only | DIN 7982 ST2.2 × 6.5 mm countersunk self-tapping: three for the cover, one for the LED retainer. |
| Magnets · 4 · magnetic enclosure only | Axially magnetized Ø3×2 mm N52 for the removable mascot. |
| Glue · enclosure only | For the eyes, and the magnets in the magnetic enclosure. |

<a name="wiring"></a>

## Wire up the LEDs

Disconnect USB power. On the ESP32-C3 SuperMini use pads 0, 1 and 3 on the GND side
(skip GPIO2, a boot-strapping pin). Connect each pad through its own resistor to an
LED anode (+). All cathodes (−) go to GND; do not connect the LEDs to 5 V.

--8<-- "site/guide/en/hardware.md:lamp-wiring"

These values give the reference build's LEDs even brightness. For other LEDs, follow
the [resistor guidance](hardware.md#wiring-and-resistor-choice).

Next: [print and assemble the enclosure](printing.md) (optional), then
[install BroGlow and connect the lamp](installation.md).
