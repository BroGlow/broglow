<a name="round3x2"></a>

# Magnetic enclosure: printing and assembly

[English](GUIDE_ROUND3X2.en.md) | [Русский](GUIDE_ROUND3X2.md)

Download **broglow-enclosure-magnetic-vVERSION.zip** from the project's GitHub
Releases; the website shows its SHA-256. The mascot holds on by
magnets: pull it off, put it back, or swap in any mascot you like, any time.

<a name="fit"></a>

## Magnets and fit

Use **four axially magnetized Ø3×2 mm N52 neodymium magnets**: two in the mascot,
two in the traffic light (tower). Pockets are **Ø3.05 × 2.30 mm** with a teardrop-shaped top, so
they print without supports with the part on its side. The hand stays slim, and
the tower has no external mounting platform.

The 3.05 mm diameter was confirmed with magnets measuring 3.01–3.04 mm:
easy insertion with slight retention, suitable for glue. The 2.30 mm depth
and assembled holding force still need physical confirmation. Printer and magnet
tolerances vary; dry-fit before bonding. Magnet faces are designed to sit flush
with the mating surface and touch, with no gap.

<a name="print"></a>

## What to print

The archive has no subfolders: these five STL parts to print, the same parts as
STEP, two assembly views and `BUILD.json`.

| File | Quantity | Color |
| --- | --- | --- |
| 01_mascot_magnetic_orange.stl | 1 | Orange |
| 02_eye_black_x2.stl | **2** | Black |
| 03_tower_magnetic_black.stl | 1 | Black |
| 04_rear_cover_black.stl | 1 | Black |
| 05_LED_retainer_black.stl | 1 | Black |

Use all parts from the same package. STLs are bed-oriented: **100% scale, no auto-orientation, no supports**.
The eye file contains one eye: duplicate it in the slicer.
Reference slicing profile (sliced and checked, not a print test): **A1 mini / PETG
Basic / 0.4 mm nozzle / 0.20 mm layers / 4 walls / 15% infill / 5 top and bottom
layers**. Set temperatures for your filament.

The package has no sliced 3MF or G-code. The STEP files are the same parts in
assembly coordinates; use STL for the supplied print orientations.
**round3x2.step** and **round3x2_exploded.step** show the assembly with reference
magnets and electronics; they are not for printing. The LED retainer (05) is
recommended: without it the LEDs sit by friction only and can be pushed inside.

<a name="assembly"></a>

## Assembly and bonding

You also need an ESP32-C3 SuperMini, three 5 mm LEDs, wiring, resistors, glue for
the magnets and eyes, and **four DIN 7982 ST2.2 × 6.5 mm countersunk screws**
(three for the cover, one for the retainer; skip that one without the retainer). Disconnect USB while assembling.
Wire the LEDs as in the
[hardware guide]({{ repo_url }}/blob/HEAD/site/guide/en/hardware.md#wiring-and-resistor-choice):
red/yellow/green anodes through 330/330/220 Ω resistors to GPIO0/1/3, all cathodes
to GND.

1. Clean off any stringing and dry-fit the actual board, cable, LEDs and magnets.
   The board enters straight from the rear, USB toward the rear cover. Keep it
   centered; do not force components past the guides or pull the board into place
   with screws.
2. Insert LEDs from the rear: red on top, yellow in the middle, green below.
   Route the top LED around the upper screw boss. Keep leads clear of flange seats.
3. Insert retainer 05 from the rear **1.3 mm below** its final position, slide it
   **up 1.3 mm** into the roof pocket and secure the lower screw. Keep leads and
   wires clear of its slide path, guides and screw bosses.
4. Before closing the cover, test the LEDs with the cover off. The board needs
   BroGlow firmware; see
   [Check the wiring before connecting BroGlow]({{ repo_url }}/blob/HEAD/site/guide/en/hardware.md#check-the-wiring-before-connecting-broglow).
   Leave BOOT and RESET reachable and enough wire slack to slide the board out.
   Then close the rear with three screws, tightened by hand; the cover must close
   without bending the board or pinching wires.
5. Check that each facing magnet pair **attracts** and mark polarity. Glue magnets
   into each half separately, setting their faces **flush with the mating surface**;
   do not push them to the bottom of the pocket. Press the faces against a smooth,
   flat, nonmagnetic surface; if you use release film, it must cover both the
   plastic and the magnet so the magnet does not sink. Keep glue off the magnet
   faces. Join the halves only after the glue cures.
6. Glue both eyes into the front pockets. Bring the hand to the tower sideways;
   the magnet faces should touch, and all mascot feet and the tower base should sit
   flat on the table. Detach the mascot with a gentle sideways pull when it is in
   the way.

At flush position the calculated glue space is 0.30 mm behind a 2.00 mm magnet,
or 0.10 mm behind a 2.20 mm magnet. Actual thickness controls the remaining space.
For servicing, remove the cover and withdraw the board rearward without pulling
wires. To remove the LED retainer, undo its screw, slide down 1.3 mm, then pull back.

<a name="validation"></a>

## Checks and limits

Walls around and behind each pocket are at least 0.80 mm; retainer 05 clears the
backing of the tower pockets by 0.40 mm. The supplied STLs are exactly the ones that
were sliced and reviewed. None of this proves magnet holding force or fit on your
printer: dry-fit first.

<a name="generate-from-source"></a>

## Generate from source

Install [uv](https://docs.astral.sh/uv/). Run from the root of the full repository
or GitHub's source ZIP. Reviewed on macOS 15 (Apple
Silicon) with Python 3.14; other platforms may produce different STL hashes and
fail the slicing check below.

```bash
uv run --no-project --python 3.14 --with-requirements models/broglow/requirements.txt \
  python scripts/package-enclosure.py
```

Output: **dist/enclosure/broglow-enclosure-magnetic-vVERSION.zip** and its
**.sha256**, built from an isolated copy of the source.
