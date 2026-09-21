[English](PRINT_GUIDE.en.md) | [Русский](PRINT_GUIDE.md)

<a name="broglow"></a>

# BroGlow — compact version for single-extruder printing

Download **`broglow-enclosure-vVERSION.zip` and its `.sha256`** from this project's
GitHub Release assets. Extract the ZIP and open `models/broglow/PRINT_GUIDE.en.md`;
its links point to the included STL, STEP, CAD source and reports. GitHub's
**Source code (zip)** needs [generation from source](#generate-from-source).

The assembled enclosure measures approximately **64.5 × 29.35 × 45.5 mm**,
excluding projecting LEDs and the cable. Print the supplied files at **100% scale**.

<a name="_1"></a>

## Design and fit

The PCB slides into integral C-shaped guides from the rear, with USB facing the
cover. Check the actual board before printing:

- The lower guides support **0.80 mm** strips along the PCB edges. Both far-end
  corners need **2.7 × 1.0 mm** clear of components and solder.
- The model allows components **0.6 mm below / 4 mm above** the PCB. The underside
  gap is **0.8 mm**, leaving **0.2 mm** below the modeled components.
- The channel allows **±0.60 mm** bare-board movement, but modeled components clear
  only **±0.15 mm** sideways offset. Insert the board centered and without force.
- Check the actual USB cable: its molding must clear the cover and table.

Dimensions and fit checks are in [validation.json](validation.json); editable
parameters are in [broglow.py](broglow.py) and [hand_joint.py](hand_joint.py).

<a name="_2"></a>

## Print order

1. Print [USB-01 — base](stl/coupons/01_USB_base_coupon.stl) and
   [USB-02 — cover](stl/coupons/02_USB_cover_coupon.stl). Use the actual PCB, cable
   and two DIN 7982 ST2.2 × 6.5 screws. Check insertion/removal, solder clearance,
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

Settings: **A1 Mini, PETG Basic, 0.4 mm nozzle, 0.2 mm layers, 3 walls,
15% infill, 5 top and bottom layers, supports disabled**. Keep the plate/material
settings used for the successful coupon. Add a brim to the small eyes if needed.
Inspect slots, rails, pins and the USB opening in the slicer. Clear strings from
the retainer tab, pocket and lead passages.

<a name="_3"></a>

## Measurements and fasteners

| Actual component | Value used in the model |
| --- | --- |
| Three LEDs | Total plastic length **8.5 mm**, including flange **Ø5.4 × 1.2 mm**; barrel **7.3 mm**, body **Ø4.75**; **Ø5.1 mm** fitting holes were selected from the coupon independently of LED diameter |
| ESP32-C3 SuperMini | PCB **22.4 × 18 × 0.7 mm**, total length including USB **23.9 mm**, USB projection beyond PCB **1.5 mm** |
| USB-C | Body **8.9 × 3.1 mm**; opening center Z = **5.25 mm**, correction `USB_SEATING_Z_OFFSET=0.2`; modeled USB end **Y = 27.65 mm**; check actual cable seating |
| Screws | **4 identical DIN 7982 ST2.2 × 6.5 mm** countersunk Phillips self-tapping screws, 304 stainless steel: three for the cover, one for the LED retainer |

The countersunk screw's **6.5 mm length includes its head**. Head seat: **80°** angle, **4.6 mm** outer countersink diameter, **2.5 mm** clearance hole and **1.9 mm** pilot. Standard dimensions are in the [Böllhoff DIN 7982 table](https://eshop.boellhoff.de/out/media/pdf/DIN_7982_Edelstahl_A2___en.pdf); the difference between DIN 7982 and ISO 7050 angles is described in [REYHER, page 11](https://www.reyher.de/fileadmin/user_upload/Downloadbereich/Technische_Informationen_2016_11_EN_KS_klein.pdf#page=11). Tighten by hand until clamped; do not apply extra force after the head seats.

The only additional supplies are glue for the eyes/optional permanent hand attachment and, if needed, soft **0.25 mm** shims behind the LED flanges. Do not glue the electronics or PCB guides. PCB installation needs no cable ties or additional screws.

<a name="_4"></a>

## Assembly and servicing

1. Remove strings and blobs from guide entries, PCB slots and holes. Check the actual component-free edge strips and GPIO solder joints against the **0.80 mm** lower bearing and two **2.0 mm** far-end lips. Do not force a tight board into the slots.
2. Insert the LEDs through the tower’s open rear: **red at the top, yellow in the middle and green at the bottom**. For the upper red LED, pass the upper screw boss slightly toward the mascot side, then align with the hole and push forward. Seat each flange against its front support. Arrange the leads clear of the flange contact areas; do not load the leads to hold an LED in position.
3. Keep the LED leads on the open side of the retainer, away from the mascot, so the retainer can slide vertically. Approach from the rear with retainer 05 **1.3 mm below its final position**, then slide it **up 1.3 mm** to engage the rigid upper tab in the roof pocket. Secure the lower end with one screw. The retainer does not require a flexible clip. There is **0.25 mm** nominal axial clearance; if needed, use a thin soft shim behind the flange without obstructing leads. Keep the cover off during wiring and electrical checks.
4. With the rear cover removed, hold the PCB horizontal with USB facing the open rear. Slide its two long edges straight into the integral C-guides and under the far-end corner lips until the far edge meets the front stops. No tilt, flexible clips or separate tray are needed. Slide the PCB centered beneath the lips. The component model clears at **±0.15 mm** sideways offset, while the bare-board channel allows **±0.60 mm**; full sideways travel can contact components and is not a safe insertion allowance. Center USB in the cover before closing; do not use the screws to force alignment. Upper lips limit lifting with the existing **0.40 mm** vertical play; this is not a friction clamp.
5. Wire and electrically test the lamp using the [hardware guide](../../site/guide/en/hardware.md#wiring-and-resistor-choice) and [LED check](../../site/guide/en/hardware.md#check-the-wiring-before-connecting-broglow). Leave enough flexible wire slack to slide the PCB out; keep wires clear of guides, the cover and screw bosses. Keep wires clear of the retainer, tab pocket and screw; check the actual bundle before closing the cover. BOOT and RESET must remain accessible with the cover open.
6. Close the cover with three screws; the USB coupon uses only the lower two. Ensure no leads or wires are pinched and each LED projects evenly before tightening. The fixed front stops and cover stops take loads through the PCB end edges and limit its longitudinal travel. The cover must close without pressing components or bending the board. The cover outer face is **Y = 27.4 mm** and the nominal USB end is **Y = 27.65 mm**; the modeled **0…0.5 mm** projection has not been established by physical measurement. Check that the cable seats fully and the PCB remains located during connection and removal.
7. Align the hand's diamond pins with the tower sockets, including the enlargements for the pin-root haunches. Holding the tower still, move the mascot **straight right toward the tower, only along +X, by 1.8 mm**. No forward/backward movement along Y is needed. To separate them, pull straight in the opposite direction, along −X. **The mascot's four feet and the tower base must all stand on the same plane.** For permanent attachment, apply very little glue only to the pins/hand-to-shell contact. Keep glue away from the removable cover and its joint at **Y = 25 mm**; the cover spans Y = **25…27.4 mm**. Glue the two identical eyes into the shallow front pockets.

To remove the board, disconnect USB, remove the rear cover, and slide the PCB straight backward out of the guides without pulling its wires. No clips need releasing. Check that solder joints clear the guide entries during removal.

The flat cover locates the PCB independently of the separate LED retainer. The LEDs remain retained when the cover is removed. For LED service, support the LEDs before removing the retainer’s lower screw. Slide the retainer down **1.3 mm** to release its upper tab, then withdraw it rearward; keep leads clear throughout. Cable ties, a glued-in USB panel and separate PCB mounts are unnecessary. Inside the tower, the hand-socket reinforcements rest on 45° haunches.

<a name="_5"></a>

## Files and configuration

- [broglow.py](broglow.py) — parametric geometry for six assembly pieces and export of five unique STLs; [hand_joint.py](hand_joint.py) — the joint's pins and sockets.
- [broglow.step](broglow.step) — only the printable parts in assembly; [parts.json](parts.json) — STL dimensions and volumes after generation.
- [appearance.step](appearance.step) and [exploded.step](exploded.step) — appearance and exploded assemblies with illustrative electronics; sources [appearance.py](appearance.py) and [exploded.py](exploded.py).
- [usb_coupon.py](usb_coupon.py) — USB coupon made by trimming the working tower and cover to **14 mm** high; [usb_coupon.step](usb_coupon.step) — two parts in print orientation. [USB-01 — base](stl/coupons/01_USB_base_coupon.stl) and [USB-02 — cover](stl/coupons/02_USB_cover_coupon.stl) are used with the actual PCB and two standard screws. Electronics are not included in these files.

Edit dimensions and fits in [broglow.py](broglow.py); hand-joint parameters are in
[hand_joint.py](hand_joint.py). Regenerate and validate after changing them.

<a name="generate-from-source"></a>

## Generate from source

Use **macOS 15 on Apple Silicon with Python 3.14**, the verified generation
platform, on a fresh source checkout or the extracted full enclosure package.
Start in the directory containing `Cargo.toml`. No Codex, agent skill,
PlatformIO, connected board or running app is required. Install the pinned
[requirements.txt](requirements.txt) and run the same command used by release CI:

```bash
python3.14 -m venv .venv-cad
.venv-cad/bin/python -m pip install -r models/broglow/requirements.txt
.venv-cad/bin/python scripts/package-enclosure.py
```

The CAD environment `.venv-cad/` and output directory `dist/enclosure/` are ignored.
Local Python 3.14.2 and 3.14.6 builds reproduced all seven reviewed STL hashes.
Other platforms may tessellate differently: Linux produced different STL bytes
with the same dependency pins and failed the strict evidence check. Use the
verified platform, or obtain real slicing evidence for another platform's meshes
before adding its support. Windows generation is not verified.

The command starts an isolated build under `dist/enclosure/source/`, exports the
current printable parts and coupons, and calls `gen_step()` for the additional
views. It runs full CAD **and mesh** checks, generates dimensioned PNG/SVG images,
and invokes the existing packager and its `--check` verification. Outputs are:

- `dist/enclosure/source/models/broglow/stl/` and `stl/coupons/`: printable meshes.
- `dist/enclosure/source/models/broglow/`: `broglow.step`, `appearance.step`,
  `exploded.step`, `usb_coupon.step`, `parts.json` and `validation.json`.
- `dist/enclosure/source/models/broglow/preview/dimensions.png` and `.svg`:
  dimensions generated by [dimensions.py](dimensions.py).
- `dist/enclosure/broglow-enclosure-vVERSION.zip` and `.zip.sha256`: the full
  ready-to-print release package; `VERSION` comes from `Cargo.toml`.
- `dist/enclosure/broglow-print-pack.zip`, `broglow-compact-STL.zip` and
  `broglow-USB-fit-test.zip`: the full, STL-only and coupon convenience packages.

Use `--output-dir PATH` to choose another ignored output directory. Generation
copies sources into a fresh staging tree, so old local STL files cannot leak into
a package and reviewed reports in the checkout stay unchanged. Existing exports
beside the editable CAD source remain available for your own inspection.

Packaging compares every generated STL hash with [slicer-validation.json](slicer-validation.json)
and checks its recorded slicing results. A mismatch fails the build. Investigate
the pinned toolchain and obtain real slicing evidence before changing reviewed
hashes; CAD checks are not slicing. CI verifies existing matching slicer evidence,
not a new slicer run. Check each new print using the assembly instructions above.

<a name="agent-generation"></a>

## Generate with an agent

Copy this request while the agent is working in the source checkout:

```text
Generate the enclosure release artifacts from this checkout's CAD source. Follow
PRINT_GUIDE.en.md's macOS 15 Apple Silicon platform and pinned Python 3.14
environment, then run scripts/package-enclosure.py.
Export all printable STLs and all four STEP views, generate dimensions, run full
CAD/mesh and packaging checks, and verify the reviewed slicer evidence against
the generated meshes. Investigate any mismatch; do not replace evidence with
unchecked hashes. Inspect the primary assembly with CAD tools when available.
Produce the enclosure ZIP and SHA-256 under ignored dist/enclosure/. Report the
source commit, validation results, generated files and output paths. Leave
geometry, firmware, the installed desktop app and Git history unchanged; do not
commit, tag or publish the outputs.
```

<a name="_6"></a>

## Checks and viewing

[CAD validation](validation.json) covers **19 check groups**, including watertight
meshes, assembly collisions, PCB insertion and the retainer's removal path.
[Slicer validation](slicer-validation.json) records the **7 STL hashes** and
reference-profile results. These reports do not measure retention strength;
check each new print during assembly.

For local viewing, open the [assembly](broglow.step), [view with electronics](appearance.step), or [exploded assembly](exploded.step) in a CAD viewer. These relative links also work in the unpacked print package; STEP and STL dimensions are in millimeters.
