[English](PRINT_GUIDE.en.md) | [Русский](PRINT_GUIDE.md)

<a name="claw-light"></a>

# Claw Light — compact version for single-extruder printing

Download **`clawlight-enclosure-vVERSION.zip` and its `.sha256` from this project's
GitHub Release assets** for ready-to-print files. GitHub's automatic **Source code
(zip)** is an editable source snapshot; generate its STL/STEP files using the
procedure below. STL, STEP/STP and ZIP outputs are ignored and are not stored in Git.

Extract the complete enclosure ZIP before following file links. Open
`models/clawlight/PRINT_GUIDE.en.md` inside it: `stl/`, `stl/coupons/`, the four
`.step` files and `preview/` are relative to that guide. CAD source, validation
reports and the linked hardware instructions are included. The compact STL ZIP
places main parts at its root and coupons in `coupons/`; use the full enclosure
ZIP when you need this guide, STEP assemblies or editable sources.

**Status: models finalized and accepted by the user on 2026-09-19. The printed coupon with 0.30 mm upper-rail overlap has a good PCB fit.** The accepted geometry retains the **19.20 × 1.10 mm** sliding channel. If updating the earlier wide version, **reprint only tower 03**; no further coupon iteration is needed for this board. The flat LED retainer, rear cover, mascot and eyes are unchanged. The assembly still uses **four screws: three for the cover and one for the retainer**.

The enclosure is designed for **A1 Mini, PETG Basic, a 0.4 mm nozzle and 0.2 mm layers**. The mascot and eyes use **65% scale**; electronics, screws and tested hardware fits retain their actual dimensions. The assembly measures approximately **64.5 × 29.35 × 45.5 mm**, including the eyes but excluding projecting LEDs and the cable. Enclosure depth remains **27.4 mm**; the base is **24 mm** wide and **15.2 mm** tall.

**Already physically checked:** earlier coupons selected **1.9 mm** screw pilots, **5.1 mm** LED bores and **3.3 mm** hand sockets. Those fit dimensions remain, but the new retainer, tab pocket and LED flange seats need a printed fit check. The latest coupon with wider upper lips was physically tested and accepted as a good fit on **2026-09-19**; PCB height and USB position are unchanged. The earlier **0.7 mm** USB recess has not been remeasured; CAD clearances do not establish the actual connector, solder or component fit.

<a name="_1"></a>

## What changed and why

The reduced tower and mascot make the existing LEDs larger relative to the body. LED spacing is **6.5 mm**, with centers at **Z = 27, 33.5 and 40 mm**. The **5.1 mm** bores retain the tested fit; their **0.4 mm** entry chamfers leave **0.6 mm** between adjacent mouths.

The base has two integral **C-shaped PCB guides** connected to the flat **2 mm** floor, front wall and side walls. Their upper roofs grow into the side walls along **45°** triangular braces, rising **1.3 mm** above the rail top at the wall. Two short C-shaped holders return around the PCB corners at the far end, opposite USB. The board slides straight in from the open rear, with the USB connector facing the cover. There is no separate tray to move inside the base.

| Feature | Current geometry |
| --- | --- |
| SuperMini PCB | **18 × 22.4 × 0.7 mm**; bottom **Z = 2.8 mm**, top **Z = 3.5 mm** |
| Guide fit | **0.60 mm** clearance at each side: **19.20 mm** channel; **1.10 mm** slot height leaves **0.40 mm** above the PCB |
| Edge support and retention | Lower bearing reaches **0.80 mm** under each PCB edge; upper lips overlap the centered PCB by **0.30 mm** per edge, leaving **17.40 mm** open above it. The **19.20 × 1.10 mm** sliding channel below the lips is unchanged; the cover locates USB |
| Guide entries | **0.25 mm** rear lead-in chamfers on the slot mouths only and **0.10 mm** long-edge bevels; wall/floor roots remain continuous to the cover joint **Y = 25 mm** |
| Far-end corner lips | Each covers **2.0 mm** across one corner and reaches **0.60 mm** over the PCB end; **14 mm** stays open centrally. Lip undersides are at **Z = 3.9 mm**, with **0.15 mm** entry chamfers and **45°** front-wall braces |
| End stops | Edge-only front stops at **Y = 3.5 mm** and the existing cover stops at **Y = 26.4 mm** limit board travel |
| USB and cover | Opening **9.15 × 3.20 mm**, center **Z = 5.25 mm**; **0.125 mm X / 0.05 mm Z** clearance per side around the **8.9 × 3.1 mm** socket envelope; **0.2 mm** entry chamfers on both sides and **0.15 mm** corner bevels |
| Enclosure and mascot | Tower **14 × 45.5 mm**, base **24 × 15.2 mm** in front view; enclosure depth **27.4 mm**. Mascot face **32.5 × 26 mm**, total height **35.75 mm**, depth **20.5 mm** |

The PCB is **1.8 mm lower** than the tray-based version. Its underside is **0.8 mm** above the 2 mm floor, leaving **0.2 mm** below the assumed **0.6 mm** solder/component envelope. This brings the USB cable closer to the table; whether its molding touches the table depends on the actual cable.

The guides contact narrow PCB edge strips. The new corner lips assume **2.7 × 1.0 mm bare patches at both far-end corners**. Check those patches, actual GPIO solder pads, joints and components before insertion; the board must slide without bending or being forced. The cover captures the PCB in the front/back direction when closed. With it removed, the board can slide out for service.

The cover has an unchanged flat outer face, a lower PCB recess, with **0.2 mm** inner edge chamfers and **0.4 mm** outer perimeter chamfer. The recess leaves a **1 mm** skin and **2.4 mm** material around the screws; its top is **Z = 7.75 mm**.

The three cover-screw bosses retain **6.36 mm** axial sections, **5.56 mm** pilot depth and **0.8 mm** closed ends. The lower bosses have **0.30 mm** underside relief to **Z = 8.0 mm**, leaving at least **1.55 mm** pilot wall. Outer/pilot diameters **5.6 / 1.9 mm** leave **1.85 mm** radial walls outside that lower relief. **45°** side-wall ramps support front-face-down printing. A separate lower screw holds the LED retainer; its upper end is located by a rigid tab in a pocket. Check screw seating on the printed coupon and tighten by hand.

The LED plastic measures **8.5 mm from tip to bottom, including the 1.2 mm flange**. The barrel ahead of the flange is therefore **7.3 mm**; the previous 8.5 mm barrel assumption was incorrect. The flange front seat is at **Y = 3.8 mm**, its rear face at **Y = 5.0 mm**, and the retainer support at **Y = 5.25 mm**, leaving **0.25 mm** axial play. Nominal LED projection is **3.25…3.5 mm**. The tower’s **Ø6.3 mm** collars provide flat front flange seats; the rear bore edge is unbeveled to preserve **0.15 mm** radial flange bearing. The separate retainer has a continuous **3.2 mm** wide, **3 mm** thick spine with three short **0.75 mm** raised flange pads. Its rigid upper tab engages the roof pocket by **1 mm**, with **0.25 mm** pocket clearance and **1.15 mm** outer roof skin. The lower screw is at **X − 0.5 mm, Z = 20.5 mm** relative to the tower center. The cover screws remain at **X ± 8.5 mm, Z = 10.5 mm** and **X + 4 mm, Z = 41.2 mm**, relative to tower center **X = 33 mm**. The mascot remains **6.9 mm forward of the closed cover**.

**Hand and tower:** two diamond-shaped pins are printed as part of the hand and fit into tower sockets. There are no separate pins. The pin axes are **straight and exactly perpendicular to the hand's end face**: the mascot moves toward the tower only **to the right, along +X, by 1.8 mm**. The pin's front diamond has a full diagonal of **3 mm**; the socket uses the coupon-tested **3.3 mm** diagonal and is **2 mm** deep. Pin size and **1.8 mm** projection are unchanged; the joint datum is now **X = 26 mm**, with centers at **Y = 6 / 16 mm, Z = 24 mm**. An elongated haunch with a 45° slope at each pin root supports printing the mascot on its back; the slope does not change the insertion direction. The socket has a matching enlargement. The pins position the parts; for permanent attachment, apply a little glue after the final fit test. The tower still stands on its own base.

The main STLs are in `stl/`; the two remaining USB coupons are in `stl/coupons/`. `02_eye_black_x2.stl` contains **one eye**: add **two copies** in the slicer. Main parts are numbered 01–05; coupons have their own numbering in their folder. The eye's rear tongue retains straight chamfers of **0.325 mm on the corners** and **0.1625 mm on the entry end** after scaling. The smaller eye and its matching pocket must be printed together; the eye still prints face down.

<a name="_2"></a>

## What to print next

**For the preceding keyed-retainer version, reprint only tower 03.** The rear cover 04, flat retainer 05, mascot 01 and both eyes 02 remain compatible. First check the reinforced guides with the new **USB-01 base coupon** and the existing **USB-02 cover**. Confirm that the two far-end corner patches are bare and the board slides under the lips without force. The 14 mm coupon checks the PCB and USB, but omits the LED retainer and upper tab pocket.

1. For a first print, make [USB-01 — USB coupon base](stl/coupons/01_USB_base_coupon.stl) and [USB-02 — its cover](stl/coupons/02_USB_cover_coupon.stl). The coupon reproduces the lower **14 mm** of the working base and cover, including full-size integral guides. Slide in the actual PCB, close it with two DIN 7982 ST2.2 × 6.5 screws, and check free insertion/removal, edge and solder clearance, closure without PCB bending, USB alignment and full cable seating.
2. After a successful coupon fit, print **tower 03, cover 04 and retainer 05**. Check LEDs, wiring and closure; then print **mascot 01** and **two eyes from file 02**. Dry-fit before gluing.

The completed LED/screw and hand-joint coupons are no longer included. Their tested **5.1 mm** LED bores, **1.9 mm** pilots and **3.3 mm** hand sockets remain.

<a name="stl"></a>

## STLs and orientation

There are **7 STLs: 5 unique assembly parts and 2 coupon parts**. The assembly needs **6 printed pieces**: two copies of eye 02 and one of each other part. All files use millimeters, are already oriented and rest on Z = 0. **Do not enable automatic orientation.** Print the orange and black parts in separate jobs; neither AMS nor color changes within a job are needed.

| Part | Dimensions in print orientation, mm | Face on the bed |
| --- | --- | --- |
| [01 — orange mascot with hand and pins](stl/01_mascot_orange.stl) | 47.3 × 35.75 × 20.5 | Back; face upward |
| [02 — eye, **2 copies**](stl/02_eye_black_x2.stl) | 3.25 × 6.5 × 2.7 each | Visible face; tongue upward |
| [03 — tower](stl/03_tower_black.stl) | 24 × 45.5 × 25 | Front face; cavity upward |
| [04 — rear cover](stl/04_rear_cover_black.stl) | 24 × 45.5 × 2.4 | **Outer rear face; countersinks downward** |
| [05 — flat LED retainer](stl/05_LED_retainer_black.stl) | 7.1 × 26.6 × 3.75 | **Flat outer rear face; countersink downward** |
| [USB-01 — USB coupon base](stl/coupons/01_USB_base_coupon.stl) | 24 × 14 × 25 | Front face; cavity upward |
| [USB-02 — USB coupon cover](stl/coupons/02_USB_cover_coupon.stl) | 24 × 14 × 2.4 | Outer rear face; countersinks downward |

Covers 04 and USB-02 print **with the flat outer face downward and countersinks facing the bed**. The internal recess opens upward; its flat surface limits rearward PCB travel. Print retainer 05 flat on its outer rear face, with the countersink toward the bed. Keep the tab, pocket and lead clearances free of strings. The USB coupon omits the LED-retention features.

Settings: **0.4 mm nozzle, 0.2 mm layers, 3 walls, 15% infill, 5 top and bottom layers, supports disabled**. Use PETG Basic and the same plate/material settings that produced the successful coupon. If needed, add a brim only to the small eyes. Inspect the PCB slots, rails, pins and USB opening in the slicer. The mascot is a closed solid in CAD; slicer infill creates its internal structure, so a solid print is unnecessary.

<a name="_3"></a>

## Measurements and fasteners

| Actual component | Value used in the model |
| --- | --- |
| Three LEDs | Total plastic length **8.5 mm**, including flange **Ø5.4 × 1.2 mm**; barrel **7.3 mm**, body **Ø4.75**; **Ø5.1 mm** fitting holes were selected from the coupon independently of LED diameter |
| ESP32-C3 SuperMini | PCB **22.4 × 18 × 0.7 mm**, total length including USB **23.9 mm**, USB projection beyond PCB **1.5 mm** |
| USB-C | Body **8.9 × 3.1 mm**; opening center Z = **5.25 mm**, correction `USB_SEATING_Z_OFFSET=0.2`; the calculated envelope ends at Y = **27.65 mm**, an earlier **0.7 mm** physical recess has not been remeasured |
| Screws | **4 identical DIN 7982 ST2.2 × 6.5 mm** countersunk Phillips self-tapping screws, 304 stainless steel: three for the cover, one for the LED retainer |

The countersunk screw's **6.5 mm length includes its head**. Head seat: **80°** angle, **4.6 mm** outer countersink diameter, **2.5 mm** clearance hole and **1.9 mm** pilot. Standard dimensions are in the [Böllhoff DIN 7982 table](https://eshop.boellhoff.de/out/media/pdf/DIN_7982_Edelstahl_A2___en.pdf); the difference between DIN 7982 and ISO 7050 angles is described in [REYHER, page 11](https://www.reyher.de/fileadmin/user_upload/Downloadbereich/Technische_Informationen_2016_11_EN_KS_klein.pdf#page=11). Tighten by hand until clamped; do not apply extra force after the head seats.

Coupon dated 17.09.2026: in rows C and R, **1.8 mm** holes held the screws well with flush heads; **1.9 and 2.0 mm** also held. **1.9 mm** was selected. All three LEDs fitted **5.1 mm**; **5.3 and 5.5 mm** were too loose. The earlier test confirms the pilot diameter; the new LED retention and complete assembly still need a printed fit check.

The only additional supplies are glue for the eyes/optional permanent hand attachment and, if needed, soft **0.25 mm** shims behind the LED flanges. Do not glue the electronics or PCB guides. PCB installation needs no cable ties or additional screws.

<a name="_4"></a>

## Assembly and servicing

1. Remove strings and blobs from guide entries, PCB slots and holes. Check the actual component-free edge strips and GPIO solder joints against the **0.80 mm** lower bearing and two **2.0 mm** far-end lips. Do not force a tight board into the slots.
2. Insert the LEDs through the tower’s open rear: **red at the top, yellow in the middle and green at the bottom**. For the upper red LED, pass the upper screw boss slightly toward the mascot side, then align with the hole and push forward. Seat each flange against its front support. Arrange the leads clear of the flange contact areas; do not load the leads to hold an LED in position.
3. Keep the LED leads on the open side of the retainer, away from the mascot, so the retainer can slide vertically. Approach from the rear with retainer 05 **1.3 mm below its final position**, then slide it **up 1.3 mm** to engage the rigid upper tab in the roof pocket. Secure the lower end with one screw. The retainer does not require a flexible clip. There is **0.25 mm** nominal axial clearance; if needed, use a thin soft shim behind the flange without obstructing leads. Keep the cover off during wiring and electrical checks.
4. With the rear cover removed, hold the PCB horizontal with USB facing the open rear. Slide its two long edges straight into the integral C-guides and under the far-end corner lips until the far edge meets the front stops. No tilt, flexible clips or separate tray are needed. Slide the PCB centered beneath the lips. The component model clears at **±0.15 mm** sideways offset, while the bare-board channel allows **±0.60 mm**; full sideways travel can contact components and is not a safe insertion allowance. Center USB in the cover before closing; do not use the screws to force alignment. Upper lips limit lifting with the existing **0.40 mm** vertical play; this is not a friction clamp.
5. Wire and electrically test the lamp using the [hardware guide](../../docs/hardware-setup.md#wiring-and-resistor-choice) and [LED check](../../docs/hardware-setup.md#check-the-wiring-before-connecting-clawlight). Leave enough flexible wire slack to slide the PCB out; keep wires clear of guides, the cover and screw bosses. Keep wires clear of the retainer, tab pocket and screw; check the actual bundle before closing the cover. BOOT and RESET must remain accessible with the cover open.
6. Close the cover with three screws; the USB coupon uses only the lower two. Ensure no leads or wires are pinched and each LED projects evenly before tightening. The fixed front stops and cover stops take loads through the PCB end edges and limit its longitudinal travel. The cover must close without pressing components or bending the board. The cover outer face is **Y = 27.4 mm** and the nominal USB end is **Y = 27.65 mm**; the modeled **0…0.5 mm** projection has not been established by physical measurement. Check that the cable seats fully and the PCB remains located during connection and removal.
7. Align the hand's diamond pins with the tower sockets, including the enlargements for the pin-root haunches. Holding the tower still, move the mascot **straight right toward the tower, only along +X, by 1.8 mm**. No forward/backward movement along Y is needed. To separate them, pull straight in the opposite direction, along −X. **The mascot's four feet and the tower base must all stand on the same plane.** For permanent attachment, apply very little glue only to the pins/hand-to-shell contact. Keep glue away from the removable cover and its joint at **Y = 25 mm**; the cover spans Y = **25…27.4 mm**. Glue the two identical eyes into the shallow front pockets.

To remove the board, disconnect USB, remove the rear cover, and slide the PCB straight backward out of the guides without pulling its wires. No clips need releasing. Check that solder joints clear the guide entries during removal.

The flat cover locates the PCB independently of the separate LED retainer. The LEDs remain retained when the cover is removed. For LED service, support the LEDs before removing the retainer’s lower screw. Slide the retainer down **1.3 mm** to release its upper tab, then withdraw it rearward; keep leads clear throughout. Cable ties, a glued-in USB panel and separate PCB mounts are unnecessary. Inside the tower, the hand-socket reinforcements rest on 45° haunches.

<a name="_5"></a>

## Files and configuration

- [clawlight.py](clawlight.py) — parametric geometry for six assembly pieces and export of five unique STLs; [hand_joint.py](hand_joint.py) — the joint's pins and sockets.
- [clawlight.step](clawlight.step) — only the printable parts in assembly; [parts.json](parts.json) — STL dimensions and volumes after generation.
- [appearance.step](appearance.step) and [exploded.step](exploded.step) — appearance and exploded assemblies with illustrative electronics; sources [appearance.py](appearance.py) and [exploded.py](exploded.py).
- [usb_coupon.py](usb_coupon.py) — USB coupon made by trimming the working tower and cover to **14 mm** high; [usb_coupon.step](usb_coupon.step) — two parts in print orientation. [USB-01 — base](stl/coupons/01_USB_base_coupon.stl) and [USB-02 — cover](stl/coupons/02_USB_cover_coupon.stl) are used with the actual PCB and two standard screws. Electronics are not included in these files.

The main parameters are near the start of the source files:

| Group | Parameters and meaning |
| --- | --- |
| Overall dimensions | `DEPTH=27.4`, `MASCOT_DEPTH=20.5`, `MASCOT_SCALE=0.65`, `TOWER_X=33`, `TOWER_HEIGHT=45.5`, `UPRIGHT_WIDTH=14`, `BASE_WIDTH=24`, `BASE_HEIGHT=15.2`, `LED_PITCH=6.5`, `LED_Z=(27,33.5,40)` |
| LED fits | `LED_DIAMETERS`, `LED_FLANGES`, `LED_FLANGE_THICKNESS` — measurements; `LED_BORE_DIAMETERS=(5.1,5.1,5.1)` — independent holes; `LED_BORE_LEAD_IN=0.4` — chamfer |
| LED retention | `LED_TOTAL_LENGTH=8.5` includes `LED_FLANGE_THICKNESS=1.2`; derived `LED_BARREL_LENGTH=7.3`, `LED_SEAT_Y=3.8`; `LED_AXIAL_CLEARANCE=0.25`, `LED_SUPPORT_Y=5.25`, `LED_COLLAR_DIAMETER=6.3` |
| Retainer | `RETAINER_THICKNESS=3`, `RETAINER_KEY_ENGAGEMENT=1`, `RETAINER_KEY_CLEARANCE=0.25`, `RETAINER_RELEASE_DROP=1.3`, `RETAINER_CHAMFER=0.15`; spine width 3.2 mm, short pad reach 0.75 mm; one `LED_SCREWS` location at `(TOWER_X-0.5,20.5)` |
| Eyes | `EYE_TONGUE_CORNER_CHAMFER=0.50`, `EYE_TONGUE_ENTRY_CHAMFER=0.25` before 0.65 scaling; finished tongue chamfers 0.325 / 0.1625 mm; print face down |
| PCB | `PCB_WIDTH=18`, `PCB_LENGTH=22.4`, `PCB_THICKNESS=0.7`, `PCB_LENGTH_WITH_USB=23.9`; `PCB_FRONT=3.75`, `PCB_BOTTOM=2.8`, `PCB_TOP=3.5`; `BASE_FLOOR_THICKNESS=2` |
| Integral guides | `PCB_SIDE_CLEARANCE=0.60`, `PCB_TOP_CLEARANCE=0.40`; `PCB_RAIL_LOWER_OVERLAP=0.80`, `PCB_RAIL_UPPER_OVERLAP=0.30`, `PCB_RAIL_ROOF_THICKNESS=1`; `PCB_RAIL_ENTRY_CHAMFER=0.25`, `PCB_RAIL_EDGE_CHAMFER=0.10`; `PCB_FRONT_STOP_Y=3.5`, rear limit `REAR=25`; 45° roof braces rise to the side walls |
| Far-end corners | `PCB_CORNER_LENGTH=2.0`, `PCB_CORNER_OVERLAP=0.60`, `PCB_CORNER_ENTRY_CHAMFER=0.15`; assumed bare patches use `PCB_CORNER_CLEAR_WIDTH=2.7`, `PCB_CORNER_CLEAR_DEPTH=1.0` |
| USB and cover | `USB_WIDTH=8.9`, `USB_HEIGHT=3.1`, `USB_SEATING_Z_OFFSET=0.2`, `USB_CLEARANCE_X=0.125`, `USB_CLEARANCE_Z=0.05`, `USB_CORNER_CHAMFER=0.15`, `USB_ENTRY_CHAMFER=0.2`; `USB_REAR=27.65`, `USB_CENTER_Z=5.25`; `COVER_INNER_SKIN=1`, `COVER=2.4`, `COVER_INNER_CHAMFER=0.2`; rear stops are defined by `PCB_REAR_STOP_FRONT`, PCB play by `PCB_END_CLEARANCE=0.25` |
| Screws | `SCREW_PILOT_DIAMETER=1.9`; head/length parameters define countersinks and pilot depths; cover bosses `COVER_BOSS_DIAMETER=5.6`; `COVER_PILOT_DEPTH≈5.56`, `SCREW_TIP_WALL=0.8`, `COVER_BOSS_FRONT=REAR-COVER_PILOT_DEPTH-SCREW_TIP_WALL` define the short **6.36 mm** axial boss section; lower undersides are relieved to `PCB_INSERTION_CEILING=8.0` |
| Hand joint | In `hand_joint.py`: `PIN_DIAGONAL=3`, `SOCKET_DIAGONAL=3.3`, `PIN_PROJECTION=1.8`, `SOCKET_DEPTH=2`; `JOINT_X=26`; centers at Y = 6/16, Z = 24 mm; +X axis |

Component heights of **0.6 mm below / 4 mm above** the PCB remain modeling assumptions. LED plastic length **8.5 mm** is the user’s measurement including the flange. The integrated guides leave the board center open. The component model assumes **2.7 × 1.0 mm bare patches at the two non-USB corners**; only those corner regions are relieved, with the central end-component envelope preserved. Check those patches and actual GPIO solder joints and components at the supported edges on your PCB. Neither the board nor the cover should load components or solder. Your cable housing needs a physical check. Mascot scaling does not apply to hardware or tested holes.

<a name="generate-from-source"></a>

## Generate from source

Use **macOS 15 on Apple Silicon with Python 3.14**, the verified generation
platform, on a fresh source checkout or the extracted full enclosure package.
Start in the directory containing `Cargo.toml`. No Codex, agent skill,
PlatformIO, connected board or running app is required. Install the pinned
[requirements.txt](requirements.txt) and run the same command used by release CI:

```bash
python3.14 -m venv .venv-cad
.venv-cad/bin/python -m pip install -r models/clawlight/requirements.txt
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

- `dist/enclosure/source/models/clawlight/stl/` and `stl/coupons/`: printable meshes.
- `dist/enclosure/source/models/clawlight/`: `clawlight.step`, `appearance.step`,
  `exploded.step`, `usb_coupon.step`, `parts.json` and `validation.json`.
- `dist/enclosure/source/models/clawlight/preview/dimensions.png` and `.svg`:
  dimensions generated by [dimensions.py](dimensions.py).
- `dist/enclosure/clawlight-enclosure-vVERSION.zip` and `.zip.sha256`: the full
  ready-to-print release package; `VERSION` comes from `Cargo.toml`.
- `dist/enclosure/clawlight-print-pack.zip`, `clawlight-compact-STL.zip` and
  `clawlight-USB-fit-test.zip`: the full, STL-only and coupon convenience packages.

Use `--output-dir PATH` to choose another ignored output directory. Generation
copies sources into a fresh staging tree, so old local STL files cannot leak into
a package and reviewed reports in the checkout stay unchanged. Existing exports
beside the editable CAD source remain available for your own inspection.

Packaging compares every generated STL hash with [slicer-validation.json](slicer-validation.json)
and checks its recorded slicing results. A mismatch fails the build. Investigate
the pinned toolchain and obtain real slicing evidence before changing reviewed
hashes; CAD checks are not slicing. CI verifies existing matching slicer evidence,
not a new slicer run. Physical fit still requires the checks described above.

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

[CAD validation](validation.json) passes **19 check groups**: all **7 STLs** are watertight single components, the **6 assembly pieces** have no interfering volume across **15 pairs**, and the modeled PCB insertion, LED capture, four screw seats and retainer service path are clear. The side braces have continuous wall contact through the rear seam; the two far-end lips retain **0.20 mm** of full capture land at the worst rearward PCB position after chamfers. All **24** continuous insertion envelopes pass at X = **0 / ±0.15 mm** and both vertical slot limits, with the two explicitly assumed bare corner patches. Independent probes confirm that each long upper rail stops its PCB edge from lifting and blocks a 2° USB-end-up tilt; the far-end holders cannot mask a missing rail. Full **±0.60 mm** sideways component clearance is not claimed. With the cover and lower screw removed, the retainer can move down **1.3 mm** and then rearward without intersecting the checked printed parts, hardware or lead envelopes. Each short pad bears on **4.67 mm²** of flange; the upper tab engages by **1 mm**, with **1.15 mm** roof skin. The checked central wire passage remains **2.6 × 6 mm** with **0.25 mm** minimum printed-part clearance. Actual insulated-wire sizes, assembly fit and retention strength still require a printed check.

[Slicer validation](slicer-validation.json) records the current STL hashes and reference-profile checks. The extended-rail coupon passed the user’s physical fit test on **2026-09-19**, and the model set is finalized. Keep the successful coupon’s material, plate and orientation. Slicing and coupon fit do not establish complete wired-assembly fit or retention strength.

Current design: extended upper-rail coupon. This coupon’s physical fit was accepted on **2026-09-19**.

Previous design: wide coupon that slides in but lifts too easily. Superseded by the accepted extended-rail version.

The earlier revision introduced the reinforced tower and far-end corner holders; the flat LED retainer and rear cover remain unchanged.

For local viewing, open the [assembly](clawlight.step), [view with electronics](appearance.step), or [exploded assembly](exploded.step) in a CAD viewer. These relative links also work in the unpacked print package; STEP and STL dimensions are in millimeters.
