[English](PRINT_GUIDE.en.md) | [Русский](PRINT_GUIDE.md)

# Claw Light — compact version for single-extruder printing { #claw-light }

**Status: enclosure release candidate.** This guide covers the current CAD, STL and print packages. Results of geometry, slicer and physical checks are reported separately below.

The enclosure is designed for **A1 Mini, PETG Basic, a 0.4 mm nozzle and 0.2 mm layers**. The mascot's width and height, and the eyes, remain at 80% scale; the electronics, self-tapping screws and verified fits retain their actual dimensions. The assembly is approximately **79 × 29.8 × 58 mm** including the eyes, excluding protruding LEDs and the cable; the main enclosure depth is **27.4 mm**.

**Already physically tested:** the coupon on your printer, DIN 7982 ST2.2 × 6.5 screws and all three LEDs. The selected holes are **1.9 mm for screws** and **5.1 mm for LEDs**. The smallest socket on the old coupon, **3.3 mm**, fitted the pin; the tower uses this size. At the last fitting, the board and tray were installed in the base, but USB was **1.2 mm above the opening**, recessed relative to the cover, and the cover left a gap. The corrected version described below needs its base, cover, tray and both USB coupons reprinted and fitted again. The LED retainer and final assembly also need physical checks; geometry and slicing do not replace them.

## What changed and why { #_1 }

| Your observation | Before → after | Reason |
| --- | --- | --- |
| 1. USB is above the cover opening | Opening center Z = **8.05 mm** | Your measured **+1.2 mm** correction remains in `USB_SEATING_Z_OFFSET`; the new flat floor lowers the tray, board and opening together by another **0.8 mm** relative to the intermediate version with an 8.85 mm center. |
| 2–3. The cover leaves a gap; protruding rails interfere | Rails and reinforcements end at the joint plane **Y = 25 mm** | The fingers extending beyond the base end are removed; the cover also has no internal ribs. The tray's rear side tabs are now captured by rails within the base itself. The arm's diamond pins are retained. |
| 4. The USB opening is too loose | Opening **9.5 × 3.7 → 9.3 × 3.5 mm** | The measured **8.9 × 3.1 mm** connector has **0.2 mm** clearance on each side; **0.2 mm** entry chamfers are applied on both sides, preserving the working opening between them. The inner recess leaves a **1 mm** wall, with **2.4 mm** retained at the screws. |
| 5. USB is recessed; reduce the base depth | Enclosure depth **28.4 → 27.4 mm**, tray **24.25 → 23.45 mm** | The board ends are now at Y = **3.75 / 26.15 mm**, and USB at **27.65 mm**. With the tray fully seated, nominal USB projection beyond the cover is **0.25 mm**; with ±0.25 mm of longitudinal board play, it ranges from **0 to 0.5 mm**. |
| Remove the steps inside the floor | **2.4 mm** floor with side ledges another **0.4 mm** high → one flat **2 mm** floor | The tray rests directly on the floor; the separate lower ledges are removed. Board and USB heights are recalculated from the new support surface. |

The tray sits on the base's flat floor at **Z = 2 mm**, occupies Y = **2.7…26.15 mm** and meets the base at its front end; the previous extra gap in front of the tray is removed so seating it does not recess USB. The cover limits rearward board movement. Clearances remain **0.6 mm** at the outer tabs, **0.35 mm** at the sides and **0.45 mm** vertically; the gap beneath the board is **1 mm**. The shared depth parameter also shortens the mascot by **1 mm**, keeping the assembly's common rear plane; the **58 mm** tower height, **9 mm** LED spacing, eyes and electronics dimensions are unchanged.

To retain the tray's rear tabs inside the base without cover ribs, the latch hooks are moved forward and shortened along the tray to **0.55 mm**. The flexible section remains **12 mm** long and **0.8 mm** thick. The new hooks need latching and board-retention checks on a printed tray.

The cover now has **0.2 mm** chamfers on the inner perimeter, inner-recess edge and inner screw-hole entries. The inner USB entry also has a **0.2 mm** chamfer; the top of the recess is raised to **Z = 10.25 mm** so the chamfer runs around the entire opening without a step. The outer perimeter retains its **0.4 mm** chamfer.

Earlier fixes are retained: solid walls at the tower-to-base transition, a flat outer cover surface for printing on the bed, LED-retainer chamfers and one eye file for two copies.

**Arm and tower:** two diamond pins print as part of the arm and fit into the tower sockets. There are no separate pins. The pin axes are **straight and strictly perpendicular to the arm end**: move the mascot toward the tower only **to the right, along +X, by 1.8 mm**. The pin's front diamond has a full diagonal of **3 mm**; the socket uses the coupon-tested **3.3 mm** size and is **2 mm** deep. The pins and their **1.8 mm** projection are unchanged. An elongated heel at the pin root has a 45° slope for printing the mascot on its back; the slope does not change the insertion direction. The socket has a matching enlargement. The pins align the parts; if permanent attachment is needed, apply a little glue after final fitting. The tower still stands on its own base.

Main STL files are in `stl/`; the two remaining USB coupons are in `stl/coupons/`. `02_eye_black_x2.stl` contains **one eye**: add **two copies** in the slicer. Main parts are numbered 01–06; coupons have separate numbering within their folder. Renaming did not change geometry. The eye's rear tongue retains straight chamfers of **0.4 mm on the corners** and **0.2 mm on the entry end**. The eye still prints face down; its fit envelope and visible surface are preserved.

## What to print next { #_2 }

1. First print [USB-01 — the USB coupon base](stl/coupons/01_USB_base_coupon.stl), [USB-02 — its cover](stl/coupons/02_USB_cover_coupon.stl) and **the current production [tray 06](stl/06_PCB_tray_black.stl)**. The coupon is the bottom **19.2 mm** of the production base and cover, with their full width, depth and rails. Install the actual board as described below, holding it against the front stop; close it with two standard DIN 7982 ST2.2 × 6.5 screws. Check free tray movement, latching and board retention by the shortened hooks, closure without bending the board, USB alignment and connection of your cable. The tray must slide out after removing the cover. No separate duplicate tray is included with the coupons: this 06 is later used in the enclosure.
2. After a successful USB fitting, print **tower 03, cover 04 and LED retainer 05**. Transfer the tested tray and check the LEDs, wiring and closure of the full assembly. Then print **mascot 01** and **two eyes from file 02**. Dry-fit every joint first; use glue only after fitting.

LED/screw and arm-joint coupons were removed from the package after physical tests. The verified dimensions remain: **5.1 mm** LED holes, **1.9 mm** pilots and **3.3 mm** arm sockets. Only the USB coupon remains for the assembly that has not yet been tested.

## STL files and orientation { #stl }

There are **8 STL files: 6 unique assembly parts and 2 coupon parts**. The assembly needs **7 printed parts**: two copies of eye 02 and one of every other part. All files use millimeters and are already oriented and placed at Z = 0. **Do not enable automatic orientation.** Print the orange and black parts as separate jobs; neither AMS nor color changes within a job are required.

| Part | Dimensions in print orientation, mm | Side on the bed |
| --- | --- | --- |
| [01 — orange mascot with arm and pins](stl/01_mascot_orange.stl) | 57.8 × 44 × 27.4 | Back; face upward |
| [02 — eye, **2 copies**](stl/02_eye_black_x2.stl) | 4 × 8 × 3.32 each | Visible surface; tongue upward |
| [03 — tower](stl/03_tower_black.stl) | 28 × 58 × 25 | Front surface; cavity upward |
| [04 — flat rear cover](stl/04_rear_cover_black.stl) | 28 × 58 × 2.4 | **Outer rear surface; countersinks downward** |
| [05 — LED retainer](stl/05_LED_retainer_black.stl) | 9.2 × 30 × 4.95 | Rear surface with countersinks; flange supports upward |
| [06 — latching tray](stl/06_PCB_tray_black.stl) | 22.8 × 23.45 × 5.85 | Flat floor; latches upward |
| [USB-01 — USB coupon base](stl/coupons/01_USB_base_coupon.stl) | 28 × 19.2 × 25 | Front surface; cavity upward |
| [USB-02 — USB coupon cover](stl/coupons/02_USB_cover_coupon.stl) | 28 × 19.2 × 2.4 | Outer rear surface; countersinks downward |

Covers 04 and USB-02 print with the **flat outer side down and countersinks toward the bed**. The inner recess opens upward; its flat surface limits rearward board and tray movement. There are no protrusions inside the recess or external USB pocket.

Settings: **0.4 mm nozzle, 0.2 mm layers, 3 walls, 15% infill, 5 top and bottom layers, supports off**. Use PETG Basic and the same plate/material settings that produced the successful coupon. Add a brim only to the small eyes if needed. Inspect latch slots, rails, pins and the USB opening in the slicer. The mascot is a closed solid in CAD; slicer infill creates its internal structure, so solid printing is unnecessary.

## Measurements and fasteners { #_3 }

| Actual component | Used in the model |
| --- | --- |
| Three LEDs | Body **Ø4.75**, flange **Ø5.4 × 1.2 mm**; **Ø5.1 mm** mounting holes selected using the coupon independently of the LED diameter |
| ESP32-C3 SuperMini | Board **22.4 × 18 × 0.7 mm**, overall length including USB **23.9 mm**, USB projection beyond the board **1.5 mm** |
| USB-C | Body **8.9 × 3.1 mm**; center Z = **8.05 mm** with the measured +1.2 mm correction and new 2 mm floor; rear end Y = **27.65 mm**, nominally **0.25 mm** outside the cover |
| Self-tapping screws | **5 identical DIN 7982 ST2.2 × 6.5 mm**, countersunk Phillips, 304 stainless steel: 3 for the cover, 2 for the LED retainer |

The countersunk screw's **6.5 mm length includes the head**. Head seat: **80°** angle, **4.6 mm** outer countersink diameter, **2.5 mm** clearance hole and **1.9 mm** pilot. Standard dimensions are given in the [Böllhoff DIN 7982 table](https://eshop.boellhoff.de/out/media/pdf/DIN_7982_Edelstahl_A2___en.pdf); the angle difference between DIN 7982 and ISO 7050 is covered in [REYHER, page 11](https://www.reyher.de/fileadmin/user_upload/Downloadbereich/Technische_Informationen_2016_11_EN_KS_klein.pdf#page=11). Tighten by hand until clamped, without extra force after the head seats.

Coupon dated 17.09.2026: in rows C and R, **1.8 mm** holes held the screws well and the heads sat flush; **1.9 and 2.0 mm** also held. **1.9 mm** was selected. All three LEDs fitted **5.1 mm**; **5.3 and 5.5 mm** were too loose. The earlier test validates the pilot diameter; the reduced **Ø4.8 mm** LED-retainer bosses and the new assembly have not yet been print-tested.

The only additional items are glue for the eyes/optional arm attachment and, if needed, **0.25 mm** soft shims behind the LED flanges. Do not glue the electronics or tray. Board installation requires no ties or extra screws.

## Assembly and servicing { #_4 }

1. Remove strings and blobs from edges, holes and latch slots. On the new tray, check the component-free board edges, absence of bending and clearance beneath solder joints. There is **1 mm** between the tray floor and board; the board dimensions come from your measurements.
2. Insert the LEDs through the tower's open rear: **red at the top, yellow in the middle, green at the bottom**. The flanges meet the rear surface of the front wall. Orient each LED's pair of leads vertically, along Z, so they pass through the retainer's open slot.
3. Position the retainer with its projections toward the flanges and countersinks to the rear, then secure it with two screws. Its main plate occupies Y = **5.8…8.8 mm**, and its front supports are at Y = **3.85 mm**. There is **0.25 mm** axial clearance behind the flange. If needed, remove play with a thin soft shim without covering the leads. The LEDs remain replaceable.
4. **Outside the tower**, slide the board's front edge, opposite USB, under the tray's fixed lips, raising the USB edge by about **15°**. Lower the rear edge until both latches engage. Press on bare board material, not components. **Before closing the cover, the board can move backward:** the latches retain it vertically, while the cover's inner surface limits rearward travel. Hold the board against its front stop and insert the loaded tray from the rear into the rails **all the way to the front stop**. The rails capture the side tabs, and local stops prevent the latches from opening.
5. Follow the [hardware instructions](../../docs/hardware-setup.md#wiring-and-resistor-choice) and [LED check](../../docs/hardware-setup.md#check-the-wiring-before-connecting-clawlight) for wiring and electrical testing. Leave a loose loop of flexible wire so the tray can slide out; route it without trapping it against the cover, rails or screw bosses. BOOT and RESET must remain accessible with the cover open.
6. Close the cover with three screws; the USB coupon uses only the lower two. The tray's front stop and cover's inner surface carry force through the bare board ends, and the cover keeps the tray from sliding out. It must close without pressing components or bending the board. The cover's outer surface is at **Y = 27.4 mm**, and the nominal USB end at **Y = 27.65 mm**. Longitudinal board play permits **0…0.5 mm** of connector projection; it must not be recessed with the tray fully installed. Check plugging and unplugging your cable: the plug must seat fully while the board and tray stay in place.
7. Align the arm's diamond pins with the tower sockets, including the enlargements for their root heels. Keep the tower still and move the mascot **straight right toward the tower, only along +X, by 1.8 mm**. No forward/backward motion along Y is required. To separate them, pull straight back in the opposite direction, along −X. **The mascot's four feet and the tower base must stand on the same plane at the same time.** If permanent attachment is needed, apply a very small amount of glue only to the pins/arm-to-shell contact. Keep glue off the removable cover and its joint at **Y = 25 mm** (the cover occupies Y = **25…27.4 mm**). Glue the two identical eyes into the shallow front pockets.

To remove the board, unplug USB, open the cover and fully slide out the tray without pulling the wires, holding the board to prevent it from moving backward. Only outside the enclosure, carefully spread the latches, raise the USB edge and release the front edge from under the lips. Inside the tower, the latches are intentionally blocked by local stops.

The cover is one printed part with a flat outer surface and an inner recess without protruding ribs. Ties, a glued USB panel and separate board mounts are unnecessary. Inside the tower, the arm-socket reinforcements rest on 45° braces.

## Files and configuration { #_5 }

- [clawlight.py](clawlight.py) — parametric geometry of the seven assembly parts and export of six unique STL files; [hand_joint.py](hand_joint.py) — joint pins and sockets.
- [clawlight.step](clawlight.step) — assembled printed parts only; [parts.json](parts.json) — STL dimensions and volumes after generation.
- [appearance.step](appearance.step) and [exploded.step](exploded.step) — visual and exploded assemblies with illustrative electronics; sources: [appearance.py](appearance.py) and [exploded.py](exploded.py).
- [usb_coupon.py](usb_coupon.py) — USB coupon cut from the production tower and cover at a height of **19.2 mm**; [usb_coupon.step](usb_coupon.step) — two parts in print orientation. [USB-01 — base](stl/coupons/01_USB_base_coupon.stl) and [USB-02 — cover](stl/coupons/02_USB_cover_coupon.stl) use production tray 06, the actual board and two standard screws. Electronics and a duplicate tray are not included in these files.

The main parameters are at the beginning of the source files:

| Group | Parameters and meaning |
| --- | --- |
| Dimensions | `DEPTH=27.4`, `MASCOT_SCALE=0.8`, `TOWER_HEIGHT=58`, `BASE_HEIGHT=22.4`, `LED_PITCH=9` |
| LED fits | `LED_DIAMETERS`, `LED_FLANGES`, `LED_FLANGE_THICKNESS` — measurements; `LED_BORE_DIAMETERS=(5.1,5.1,5.1)` — independent holes; `LED_BORE_LEAD_IN=0.6` — chamfer |
| Eyes | `EYE_TONGUE_CORNER_CHAMFER=0.50`, `EYE_TONGUE_ENTRY_CHAMFER=0.25` before 0.8 scaling; final tongue chamfers 0.4 / 0.2 mm; print face down |
| LED retainer | `RETAINER_EDGE_CHAMFER=0.20`, `RETAINER_SLOT_CHAMFER=0.15`, `RETAINER_RELIEF_CHAMFER=0.15`; axial gap `LED_RETAINER_GAP=0.25` |
| Board and tray | `PCB_WIDTH`, `PCB_LENGTH`, `PCB_THICKNESS`, `PCB_LENGTH_WITH_USB`; `PCB_FRONT=3.75`, `TRAY_FRONT_EXTENSION=1.05`, `PCB_FRONT_STOP_EXTENSION=1.05`, `TRAY_FRONT_CLEARANCE=0`; `PCB_FLOOR_GAP=1`, `BASE_FLOOR_THICKNESS=2`, `TRAY_FLOOR_Z=BASE_FLOOR_THICKNESS` |
| Rails | `BASE_SIDE_WALL=2` gives 0.6 mm clearance at the outer tabs; `TRAY_RAIL_SIDE_CLEARANCE=0.35`, `TRAY_RAIL_VERTICAL_CLEARANCE=0.45`, `TRAY_KEY_EXTENSION=0.8`; `TRAY_RAIL_BACK=REAR=25` removes protrusions beyond the joint; `TRAY_KEY_CHAMFER=0.1` preserves the rear tabs' bearing edges; local `CLIP_KEEPER_CLEARANCE=0.2` |
| Latches | `CLIP_BEAM_THICKNESS=0.8`, `CLIP_BEAM_LENGTH=12`, `CLIP_HOOK_OVERLAP=0.8`; `CLIP_HOOK_REAR_INSET=2.2`, `CLIP_HOOK_LENGTH=0.55`; gap above the board `PCB_TOP_CLEARANCE=0.15` |
| USB and cover | `USB_WIDTH=8.9`, `USB_HEIGHT=3.1`, `USB_SEATING_Z_OFFSET=1.2`, `USB_CLEARANCE=0.2`, `USB_ENTRY_CHAMFER=0.2`; `USB_REAR=27.65`, `USB_CENTER_Z=8.05`; `COVER_INNER_SKIN=1`, `COVER=2.4`, `COVER_INNER_CHAMFER=0.2`; rear stops set by `PCB_REAR_STOP_FRONT`, board play by `PCB_END_CLEARANCE=0.25` |
| Self-tapping screws | `SCREW_PILOT_DIAMETER=1.9`; head/length parameters determine countersinks and pilot depths; bosses `COVER_BOSS_DIAMETER=5.6`, `LED_BOSS_DIAMETER=4.8` |
| Arm joint | In `hand_joint.py`: `PIN_DIAGONAL=3`, `SOCKET_DIAGONAL=3.3`, `PIN_PROJECTION=1.8`, `SOCKET_DEPTH=2`; centers at Y = 7/20, Z = 30 mm; +X axis |

Component heights of **0.6 mm underneath / 4 mm above** and the illustrated **8.5 mm** LED length from flange to tip remain design assumptions. The actual board must not rest on components or solder joints. Your cable housing must be checked physically. Mascot scaling does not apply to hardware or verified holes.

To rebuild, install [requirements.txt](requirements.txt), then run `python models/clawlight/clawlight.py` and `python models/clawlight/check.py` from the `clawlight` repository root. Rebuild archives with `python models/clawlight/package_prints.py`. Additional STEP views are exported by their `gen_step()`; [dimensions.py](dimensions.py) creates a dimensioned PNG/SVG from the actual geometry.

## Checks and viewing { #_6 }

The [CAD check](validation.json) for the current **27.4 mm** depth version passed **19 checks**. All **8 STL files have watertight meshes**, one connected component each and rest at Z = 0. Checks cover absence of intersections between printed parts, tray and cover movement, agreement of USB coupons with production parts, and **0…0.5 mm** USB projection over the board's longitudinal play. The new inner chamfers and preservation of the narrow USB opening behind the chamfer were checked. Additional checks cover the continuous flat **2 mm** floor, absence of ribs in the cover recess and capture of the tray's rear tabs in the base even at the rearmost position.

[Slicer report](slicer-validation.json): **8 of 8 STL files passed** for A1 Mini / PETG Basic / 0.4 mm nozzle / 0.2 mm layers. Both covers were sliced again after adding chamfers; results for the other six STL files were reused after exact SHA-256 matches. Hashes of all eight current STL files were checked against the report. No supports or filament changes were generated; there are no floating-region or mesh-repair warnings. Tray 06 sliced without internal diagnostic messages.

The **USB-02** coupon cover retains one nonfatal internal `ZFiller` message. Inspection of actual toolpaths across **12 layers and 12 components** found no missing whole layers or components, but does not prove complete local infill. Before printing, inspect USB-02 walls and infill in your slicer; check physical fit with the USB coupon first.

| Set | PETG | Estimated time |
| --- | --- | --- |
| Assembly: 6 unique STL files, 7 parts including two eyes | **38.13 g** | **2 h 34 min** |
| USB test: 06 + USB-01 + USB-02 | **8.42 g** | **1 h** |
| Full set: 8 unique STL files, 9 parts including two eyes | **45.34 g** | **3 h 22 min** |

Estimates add up separate print jobs for every part, including printer preparation; combining parts on one plate changes the time. The verification profile uses Textured PEI and Bambu PETG Basic. For actual printing, keep your plate and settings that produced the successful coupon.

**The arm's fit in the 3.3 mm socket was physically tested on earlier coupons; the final assembly still needs fitting.** The USB coupon checks the board, rails, cover closure and actual cable. Fit the LEDs, wires and full assembly before permanently gluing the arm joint.

- [Eye rear-tongue chamfers](preview/eye_chamfers_20260918T085846Z.png) — the eye geometry is unchanged.
- [LED-retainer chamfers at the front](preview/retainer_front_20260917T203403Z.png) and [rear](preview/retainer_rear_20260917T203403Z.png) — the retainer geometry is unchanged.

New views of the version with the **flat 2 mm floor and cover chamfers**, generated from rebuilt STEP files and reviewed after CAD checks:

- [Open base with tray](preview/cover_chamfers_open_20260918T102410Z.png), [inside of the cover](preview/cover_chamfers_cover_inside_20260918T102410Z.png).
- [USB close-up](preview/cover_chamfers_usb_20260918T102410Z.png) — the light-colored block shows the connector's check envelope, not a plug in the printed part.
- [Overall view](preview/cover_chamfers_finished_20260918T102410Z.png), [exploded assembly](preview/cover_chamfers_exploded_20260918T102410Z.png), [updated USB coupon](preview/cover_chamfers_coupons_20260918T102410Z.png).
- [Current dimensioned drawing](preview/dimensions.png), also [SVG](preview/dimensions.svg).

Other older base views in `preview/` show the previous geometry.

For local viewing, open the [assembly](clawlight.step), [view with electronics](appearance.step) or [exploded assembly](exploded.step) in a CAD viewer. These relative links also work in the extracted print package; STEP and STL dimensions use millimeters.
