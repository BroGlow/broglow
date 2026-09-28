<a name="round3x2"></a>

# BroGlow — recommended magnetic enclosure

[English](GUIDE_ROUND3X2.en.md) | [Русский](GUIDE_ROUND3X2.md)

Download **broglow-enclosure-magnetic-vVERSION.zip** and its **.sha256** from the
project's GitHub Release. This is the recommended enclosure: the mascot detaches
and can be reattached, and the traffic light works on its own. The glued design
remains available separately as **broglow-enclosure-vVERSION.zip**.

<a name="fit"></a>

## Magnets and fit

Use **four axially magnetized Ø3×2 mm N52 neodymium magnets**: two in the hand,
two in the tower. Pockets are **Ø3.05 × 2.30 mm** with a teardrop roof for lying
prints. Fit5 keeps the slim hand and no external mounting platform.

The 3.05 mm CAD diameter was confirmed with magnets measuring 3.01–3.04 mm:
easy insertion with slight retention, suitable for glue. The final 2.30 mm depth
and assembled holding force still need physical confirmation. Printer and magnet
tolerances vary; dry-fit before bonding. Intended magnet recess and face gap: zero.

<a name="print"></a>

## What to print

The archive's **stl/** folder contains only these five production models:

| File | Quantity | Color |
| --- | --- | --- |
| 01_mascot_magnetic_orange.stl | 1 | Orange |
| 02_eye_black_x2.stl | **2** | Black |
| 03_tower_magnetic_black.stl | 1 | Black |
| 04_rear_cover_black.stl | 1 | Black |
| 05_LED_retainer_black.stl | 1 | Black |

Use all parts from the same package. Eye 02, cover 04 and retainer 05 are also
compatible with the local fit4 set. Retainers from the glued version differ.
STLs are bed-oriented: **100% scale, no auto-orientation, no supports**.
Reviewed profile: **A1 mini / PETG Basic / 0.4 mm nozzle / 0.20 mm layers /
4 walls / 15% infill / 5 top and bottom layers**. Set temperatures for your filament.
The eye file contains one eye: duplicate it in the slicer. STEP files in **step/**
are in assembly coordinates; use STL for the supplied print orientations.

Release packages contain STL and STEP, not machine-specific sliced 3MF or G-code.
Open the STL files in your slicer and apply the settings above. No coupons or
depth gauges are included. Assembly STEP views contain reference magnets/electronics;
print only the five numbered parts.

<a name="assembly"></a>

## Assembly and bonding

You also need an ESP32-C3 SuperMini, three 5 mm LEDs, wiring, resistors and
**four DIN 7982 ST2.2 × 6.5 mm countersunk screws** (three cover, one retainer).
Disconnect USB while assembling. Red/yellow/green LED anodes connect through
330/330/220 Ω resistors to GPIO0/1/3; all cathodes connect to GND.

1. Remove strings and dry-fit the actual board, cable, LEDs and magnets. The board
   enters straight from the rear, USB toward the cover. Keep it centered; do not
   force components past the guides or pull the board into place with screws.
2. Insert LEDs from the rear: red on top, yellow in the middle, green below.
   Route the top LED around the upper screw boss. Keep leads clear of flange seats.
3. Insert retainer 05 from the rear **1.3 mm below** its final position, slide it
   **up 1.3 mm** into the roof pocket and secure the lower screw. Keep leads and
   wires clear of its slide path, guides and screw bosses.
4. Check wiring and cable seating before closing the rear with three screws.
   Tighten by hand; the cover must close without bending the board or pinching wires.
5. Check that each facing magnet pair **attracts** and mark polarity. Glue magnets
   into each half separately, setting their faces **flush with the mating surface**.
   Do not automatically press them to the pocket bottom. Use a smooth nonmagnetic
   plane; any release film must cover both plastic and magnet to avoid a recess.
   Keep glue off the magnet faces. Join the halves only after the glue cures.
6. Glue both eyes into the front pockets. Bring the hand to the tower sideways;
   the magnet faces should touch and all feet and the tower base should sit flat.
   Detach the mascot with a gentle sideways pull when it is in the way.

At flush position the calculated glue space is 0.30 mm behind a 2.00 mm magnet,
or 0.10 mm behind a 2.20 mm magnet. Actual thickness controls the remaining space.
For servicing, remove the cover and withdraw the board rearward without pulling
wires. To remove the LED retainer, undo its screw, slide down 1.3 mm, then pull back.

<a name="validation"></a>

## Checks and limits

Pocket backing and surrounding walls are at least 0.80 mm; the modified retainer
has 0.40 mm internal clearance. Release generation checks the current variant and
matches all five STL hashes against reviewed slicing evidence. The stored slicer
advisories concern filament bed temperature and timelapse, not supports or bed bounds.
These checks do not establish magnetic holding force or physical fit of every printer.

<a name="generate-from-source"></a>

## Generate from source

Use the full repository, GitHub source ZIP, or extracted magnetic package, with
macOS 15 Apple Silicon and Python 3.14 (the reviewed CAD platform):

```bash
uv run --no-project --python 3.14 --with-requirements models/broglow/requirements.txt \
  python scripts/package-enclosure.py --variant magnetic
```

Output: **dist/enclosure/broglow-enclosure-magnetic-vVERSION.zip** and its
**.sha256**, generated in isolation from the editable source. Only parts 01–05
are exported. Parameter changes live in **models/magnetic/refined.py**.
A changed STL requires updated slicing evidence before release packaging succeeds.
The full repository also supports **--variant glued** for the older enclosure.
