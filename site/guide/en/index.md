<a href="../">Back to the project</a>

<a name="build"></a>

# Build your lamp

A three-LED status lamp (ESP32-C3 SuperMini with red, yellow and green LEDs) in a
printed enclosure that mirrors your agent sessions. The mascot holds on by magnets:
pull it off, put it back, or swap in any mascot you like, any time.

Already assembled? [Install BroGlow and connect the lamp](installation.md).

<a name="parts"></a>

## Choose your parts

<!-- model-download -->

Print settings and assembly steps: [Printing and assembly](printing.md).

| Part | Qty | Needed | What to look for |
| --- | --- | --- | --- |
| Board | 1 | ✅ | ESP32-C3 SuperMini; the firmware targets ESP32-C3 only. Before flashing, [identify the board](hardware.md#build-and-identify-the-board). |
| LEDs | 3 | ✅ | 5 mm, one each red, yellow and green (the enclosure holes fit 5 mm). |
| Resistors | 3 | ✅ | 330 Ω for red and yellow, 220 Ω for green; 1/8 W or 1/4 W. |
| Wires | — | ✅ | For the LEDs and a shared ground. |
| USB cable | 1 | ✅ | A USB-C data cable (not charge-only) for setup and power. |
| USB adapter | 1 | ➖ | macOS only: powers the lamp away from the Mac, which then talks to it over Bluetooth. |
| Printed enclosure | 1 set | ✅ | Print package above: mascot, two eyes, traffic light and rear cover. PETG in orange and black. |
| LED retainer | 1 | ⭐ | Optional but highly recommended; printed with the enclosure. Without it the LEDs sit by friction only and can be pushed inside. |
| Cover screws | 3 | ✅ | DIN 7982 ST2.2 × 6.5 mm countersunk self-tapping. |
| Retainer screw | 1 | ⭐ | The same screw type; needed only with the LED retainer. |
| Magnets | 4 | ✅ | Axially magnetized Ø3×2 mm N52: two in the mascot, two in the traffic light. |
| Glue | — | ✅ | For the eyes and the magnets. |

✅ required · ⭐ recommended · ➖ optional

<a name="wiring"></a>

## Wire up the LEDs

Disconnect USB power. On the ESP32-C3 SuperMini use pads 0, 1 and 3 on the GND side
(skip GPIO2, a boot-strapping pin). Connect each pad through its own resistor to an
LED anode (+). All cathodes (−) go to GND; do not connect the LEDs to 5 V.

--8<-- "site/guide/en/hardware.md:lamp-wiring"

<div class="wiring-figure" role="img" aria-label="Wiring diagram: pads 0, 1 and 3 each through a 330, 330 or 220 ohm resistor to the anode of the red, yellow or green LED; pad 2 stays unconnected; all cathodes to GND">
--8<-- "site/guide/assets/wiring.svg"
</div>

These values give the reference build's LEDs even brightness. For other LEDs, follow
the [resistor guidance](hardware.md#wiring-and-resistor-choice).

Next: [print and assemble the enclosure](printing.md), then
[install BroGlow and connect the lamp](installation.md).
