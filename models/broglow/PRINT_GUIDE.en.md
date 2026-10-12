<a name="broglow"></a>

# BroGlow — glued enclosure (alternative)

[English](PRINT_GUIDE.en.md) | [Русский](PRINT_GUIDE.md)

This is the **glued alternative**, printed on a single extruder. The recommended
enclosure has a removable mascot on four Ø3×2 mm N52 magnets and ships as
`broglow-enclosure-magnetic-vVERSION.zip`.

Download **`broglow-enclosure-vVERSION.zip` and its `.sha256`** from the project's
GitHub release. The extracted ZIP opens with `START_HERE.md`; the links in this
guide reach the included STL, STEP, CAD source and reports. STL and STEP files are
not stored in the Git repository; their links work in the extracted ZIP.
GitHub's **Source code (zip)** needs [generation from source](#generate-from-source).

The assembled enclosure measures approximately **64.5 × 29.35 × 45.5 mm**,
excluding projecting LEDs and the cable. Print the supplied files at **100% scale**.

<a name="design-and-fit"></a>

## Design and fit

The PCB slides into integral C-shaped guides from the rear, with USB facing the
cover. Check the actual board before printing:

- The lower guides support **0.80 mm** strips along the PCB edges. Under the two
  far-end corner lips the board needs **2.7 × 1.0 mm** patches clear of components
  and solder.
- The model allows components **0.6 mm below / 4 mm above** the PCB. The underside
  gap is **0.8 mm**, leaving **0.2 mm** below the modeled components.
- The channel allows **±0.60 mm** bare-board movement, but modeled components clear
  only **±0.15 mm** sideways offset. Insert the board centered and without force.
- Check the actual USB cable: its molding must clear the cover and table.

Dimensions and fit checks are in [validation.json](validation.json).

<a name="print-order"></a>

## Print order

1. Print [USB-01 — base](stl/coupons/01_USB_base_coupon.stl) and
   [USB-02 — cover](stl/coupons/02_USB_cover_coupon.stl). Use the actual PCB, cable
   and two DIN 7982 ST2.2 × 6.5 mm screws. Check insertion/removal, solder clearance,
   closure without bending the PCB, USB alignment and full cable seating.
2. Print parts **01–05**, with **two copies of eye 02**. Dry-fit before gluing.
   The **14 mm** USB coupon checks PCB/USB fit only; check LEDs, the retainer,
   upper tab pocket and wiring in the full tower.

<a name="stl"></a>

## STLs and orientation

There are **7 STLs: 5 unique assembly parts and 2 coupon parts**. The assembly needs **6 printed pieces**: two copies of eye 02 and one of each other part. All files use millimeters, are already oriented and rest on Z = 0. **Do not enable automatic orientation.** Print the orange and black parts in separate jobs; neither AMS nor color changes within a job are needed.

| Part | Dimensions in print orientation, mm |
| --- | --- |
| [01 — orange mascot with hand and pins](stl/01_mascot_orange.stl) | 47.3 × 35.75 × 20.5 |
| [02 — eye, **2 copies**](stl/02_eye_black_x2.stl) | 3.25 × 6.5 × 2.7 each |
| [03 — tower](stl/03_tower_black.stl) | 24 × 45.5 × 25 |
| [04 — rear cover](stl/04_rear_cover_black.stl) | 24 × 45.5 × 2.4 |
| [05 — flat LED retainer](stl/05_LED_retainer_black.stl) | 7.1 × 26.6 × 3.75 |
| [USB-01 — USB coupon base](stl/coupons/01_USB_base_coupon.stl) | 24 × 14 × 25 |
| [USB-02 — USB coupon cover](stl/coupons/02_USB_cover_coupon.stl) | 24 × 14 × 2.4 |

Reference (validated) settings: **A1 Mini, PETG Basic, 0.4 mm nozzle, 0.2 mm
layers, 3 walls, 15% infill, 5 top and bottom layers, supports disabled**; adapt
them for other printers. Keep the plate/material settings used for the successful
coupon. Add a brim to the small eyes if needed. Inspect slots, rails, pins and the
USB opening in the slicer. Clear strings from the retainer tab, pocket and lead
passages.

<a name="fasteners"></a>

## Measurements and fasteners

| Actual component | Value used in the model |
| --- | --- |
| Three LEDs | Total plastic length **8.5 mm**, including flange **Ø5.4 × 1.2 mm**; barrel **7.3 mm**, body **Ø4.75**; **Ø5.1 mm** fitting holes were selected from the coupon independently of LED diameter |
| ESP32-C3 SuperMini | PCB **22.4 × 18 × 0.7 mm**, total length including USB **23.9 mm**, USB projection beyond PCB **1.5 mm** |
| USB-C | Body **8.9 × 3.1 mm**; opening center Z = **5.25 mm**, correction `USB_SEATING_Z_OFFSET=0.2`; modeled USB end **Y = 27.65 mm**; check actual cable seating |
| Screws | **4 identical DIN 7982 ST2.2 × 6.5 mm** countersunk Phillips self-tapping screws, 304 stainless steel: three for the cover, one for the LED retainer |

The countersunk screw's **6.5 mm length includes its head**. Head seat: **80°** angle, **4.6 mm** outer countersink diameter, **2.5 mm** clearance hole and **1.9 mm** pilot. Standard dimensions are in the [Böllhoff DIN 7982 table](https://eshop.boellhoff.de/out/media/pdf/DIN_7982_Edelstahl_A2___en.pdf); the difference between DIN 7982 and ISO 7050 angles is described in [REYHER, page 11](https://www.reyher.de/fileadmin/user_upload/Downloadbereich/Technische_Informationen_2016_11_EN_KS_klein.pdf#page=11). Tighten by hand until clamped; do not apply extra force after the head seats.

The only additional supplies are glue for the eyes (and the hand, if you want it permanent) and, if needed, soft **0.25 mm** shims behind the LED flanges. Do not glue the electronics or PCB guides.

<a name="assembly"></a>

## Assembly and servicing

1. Remove strings and blobs from guide entries, PCB slots and holes. Check the actual component-free edge strips and GPIO solder joints against the **0.80 mm** lower bearing and the two **2.0 mm** far-end corner lips. Do not force a tight board into the slots.
2. Insert the LEDs through the tower’s open rear: **red at the top, yellow in the middle and green at the bottom**. Guide the red LED past the upper screw boss on the mascot side, then align it with its hole and push forward. Seat each flange against its front support. Arrange the leads clear of the flange contact areas; do not load the leads to hold an LED in position.
3. Keep the LED leads on the open side of the retainer, away from the mascot, so the retainer can slide vertically. Approach from the rear with retainer 05 **1.3 mm below its final position**, then slide it **up 1.3 mm** to engage the rigid upper tab in the roof pocket. Secure the lower end with one screw. There is **0.25 mm** nominal axial clearance; if needed, use a thin soft shim behind the flange without obstructing leads. Keep the cover off during wiring and electrical checks.
4. With the rear cover removed, hold the PCB horizontal with USB facing the open rear. Slide its two long edges straight into the C-guides and under the corner lips until the far edge meets the front stops. Keep it centered (see [Design and fit](#design-and-fit)); do not force it or use the screws to align USB. The upper lips limit lifting with **0.40 mm** vertical play; this is not a friction clamp.
5. Wire and electrically test the lamp using the [hardware guide](../../site/guide/en/hardware.md#wiring-and-resistor-choice) and [LED check](../../site/guide/en/hardware.md#check-the-wiring-before-connecting-broglow). Leave enough flexible wire slack to slide the PCB out. Keep wires clear of the guides, retainer, tab pocket, screw bosses and cover; check the bundle before closing. BOOT and RESET must remain accessible with the cover open.
6. Close the cover with three screws. Before tightening, ensure no leads or wires are pinched and each LED projects evenly. The cover must close without pressing components or bending the board; its stops and the fixed front stops limit the PCB's longitudinal travel. The modeled USB projection past the cover is **0…0.5 mm** and has not been physically measured: check that the cable seats fully and the PCB stays in place when you connect and disconnect it.
7. Align the hand's diamond pins with the tower sockets, including the enlargements for the pin-root haunches. Holding the tower still, move the mascot **straight right toward the tower, only along +X, by 1.8 mm**. To separate them, pull straight in the opposite direction, along −X. **The mascot's four feet and the tower base must all stand on the same plane.** For permanent attachment, apply very little glue only to the pins/hand-to-shell contact. Keep glue away from the removable cover and its joint at **Y = 25 mm**; the cover spans Y = **25…27.4 mm**. Glue the two identical eyes into the shallow front pockets.

To remove the board, disconnect USB, remove the rear cover, and slide the PCB straight backward out of the guides without pulling its wires. Check that solder joints clear the guide entries during removal.

The flat cover locates the PCB independently of the separate LED retainer. The LEDs remain retained when the cover is removed. For LED service, support the LEDs before removing the retainer’s lower screw. Slide the retainer down **1.3 mm** to release its upper tab, then withdraw it rearward; keep leads clear throughout.

<a name="files"></a>

## Files and configuration

- [broglow.py](broglow.py) — parametric geometry and fits; exports the 7 STLs, `broglow.step` and `parts.json`. [hand_joint.py](hand_joint.py) — the joint's pins and sockets. Regenerate after changing either.
- [broglow.step](broglow.step) — only the printable parts in assembly; [parts.json](parts.json) — STL dimensions and volumes after generation.
- [appearance.step](appearance.step) and [exploded.step](exploded.step) — appearance and exploded assemblies with illustrative electronics; sources [appearance.py](appearance.py) and [exploded.py](exploded.py).
- [usb_coupon.py](usb_coupon.py) — USB coupon made by trimming the working tower and cover to **14 mm** high; [usb_coupon.step](usb_coupon.step) — its two parts in print orientation, without electronics.

<a name="generate-from-source"></a>

## Generate from source

Needed only for GitHub's **Source code (zip)**, a repository clone, or after
editing the CAD. Use **macOS 15 on Apple Silicon**, the platform of the reviewed
slicer results; other platforms may tessellate differently and fail the hash check.
Install [uv](https://docs.astral.sh/uv/getting-started/installation/) and run, from
the directory containing `Cargo.toml` (a clone, the source archive or the extracted
enclosure ZIP):

```bash
uv run --no-project --python 3.14 --with-requirements models/broglow/requirements.txt \
  python scripts/package-enclosure.py --variant glued
```

The script builds in an isolated copy under `dist/enclosure/source/`, runs the CAD
and mesh checks and writes `dist/enclosure/broglow-enclosure-vVERSION.zip` with
its `.sha256`. It fails if a generated STL differs from the reviewed hashes in
[slicer-validation.json](slicer-validation.json); do not edit those hashes without
real slicer results. Output details and the release flow:
[Release and publication]({{ repo_url }}/blob/HEAD/site/guide/en/releasing.md#package-glued-enclosure).

<a name="agent-generation"></a>

## Generate with an agent

Paste this into an agent:

```text
Build the BroGlow glued enclosure package from source, following
{{ repo_url }}/blob/HEAD/models/broglow/PRINT_GUIDE.en.md#generate-from-source

Preconditions: this host is macOS 15 on Apple Silicon, uv is installed, and the
current directory contains Cargo.toml and scripts/package-enclosure.py (a BroGlow
clone, source archive or extracted enclosure ZIP). If any is missing, stop and
tell me; do not install or download anything without asking.

Run exactly, from that directory:
uv run --no-project --python 3.14 --with-requirements models/broglow/requirements.txt python scripts/package-enclosure.py --variant glued

Never edit slicer-validation.json, STL hashes, validation reports or CAD sources.
If the script fails (including an STL hash mismatch), stop and report the error;
do not retry with changes. Do not commit, tag, publish or upload anything.

Report: platform, source_commit and source_dirty from
dist/enclosure/source/BUILD.json, pass/fail with the failing step if any, and the
paths of broglow-enclosure-vVERSION.zip and its .sha256.
```

<a name="checks"></a>

## Checks and viewing

[CAD validation](validation.json) covers **19 check groups**, including watertight
meshes, assembly collisions, PCB insertion and the retainer's removal path.
[Slicer validation](slicer-validation.json) records the **7 STL hashes** and
reference-profile results. These reports do not measure retention strength;
check each new print during assembly.

In the extracted ZIP, open the [assembly](broglow.step), [view with electronics](appearance.step) or [exploded assembly](exploded.step) in a CAD viewer; STEP and STL dimensions are in millimeters.
