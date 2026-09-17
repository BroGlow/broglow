# Claw Light — single-extruder printing and assembly

**Fit status: provisional.** These files use explicit, unmeasured PCB, USB and LED envelopes. They are not a verified fit for your purchased parts. Measure the hardware, print the fit coupon, and inspect the sliced layers before committing to the complete enclosure. CAD checks do not establish physical fit, cable access or printer tolerances.

The assembled printed parts occupy approximately **93 × 43 × 76 mm** (width × depth × height), including the projecting eyes but excluding LEDs and USB cable. The STL sizes below are their **print orientations**, not assembled dimensions. All STL exports already sit on bed Z = 0; import in millimetres and do not auto-orient them.

## Сначала — тест посадки

1. Напечатайте **все три детали купона**: [основание](stl/00_fit_coupon.stl), [планку C](stl/00b_coupon_cover_cap.stl) и [планку R](stl/00c_coupon_retainer_cap.stl). Используйте тот же пластик и настройки, что для чёрного корпуса: сопло 0,4 мм, слой 0,2 мм, 3 стенки, 15% заполнения, 5 верхних/нижних слоёв, поддержки выключены. Сохраните ориентацию STL: у C зенковки вверх, у R — вниз.
2. После печати **переверните R зенковками вверх**. Установите C на ряд C, R на ряд R; отверстия планок должны совпасть со стойками. Ряд C повторяет крышку толщиной 2,4 мм, R — крепёжную пластину удерживателя толщиной 3 мм. Глубины глухих отверстий соответствуют каждому креплению.
3. Вашим саморезом **DIN 7982 ST2.2 × 6,5 мм с потайной головкой** проверьте отверстия **1,7 / 1,8 / 1,9 / 2,0 мм** слева направо. Вкручивайте вручную **только через соответствующую планку**: без неё остриё может упереться в дно. Подходящий диаметр позволяет головке сесть и прижать планку без трещин, прокручивания и чрезмерного усилия. Не затягивайте дальше после прижима. Проверьте оба ряда; значение 1,8 мм в модели пока предварительное.
4. Примерьте каждый светодиод в подписанные отверстия **5,1 / 5,3 / 5,5 мм**: цилиндр входит без усилия, фланец остаётся сзади. Прямоугольный карман предназначен для пробной посадки одного напечатанного глаза.
5. Сообщите, какой диаметр отверстия под саморез подошёл в **C и R**, и какое отверстие лучше для **красного, жёлтого и зелёного** светодиодов. Если результаты рядов различаются, нужны отдельные размеры отверстий в модели до печати корпуса. Физическая посадка ещё не проверена; купон проверяет её на вашем пластике и крепеже.

## Files and print order

Print the three-piece fit coupon first, then print the orange and black jobs separately. There are **10 STLs: seven assembly parts and three coupon parts**. No AMS, filament changes during a print, or multicolour slicing are required. Print each file once; the eyes are two separate files.

| Job / file | Bed footprint × height, mm | Exported orientation |
| --- | --- | --- |
| 1. [Fit coupon base](stl/00_fit_coupon.stl) | 68 × 50 × 8.8 | Flat base down; posts and labels up |
| 1. [Coupon cover cap C](stl/00b_coupon_cover_cap.stl) | 54 × 8 × 2.4 | Countersink mouths up |
| 1. [Coupon retainer cap R](stl/00c_coupon_retainer_cap.stl) | 54 × 8 × 3 | Countersink mouths down; flip after printing for the test |
| 2. Orange: [mascot, four feet and arm](stl/01_mascot_orange.stl) | 70 × 55 × 40 | Back down; face up |
| 3. Black: [left eye](stl/02_eye_left_black.stl) and [right eye](stl/03_eye_right_black.stl) | 5 × 10 × 4.15 each | Visible faces down; locating tongues up |
| 3. Black: [tower](stl/04_tower_black.stl) | 28 × 76 × 37.6 | Front down; wiring cavity open upward |
| 3. Black: [rear cover](stl/05_rear_cover_black.stl) | 28 × 76 × 2.4 | Inner face down; countersink mouths up |
| 3. Black: [LED retainer](stl/06_LED_retainer_black.stl) | 9.55 × 39.35 × 5.15 | Rear/countersink mouths down; flange pads up |
| 3. Black: [PCB tray](stl/07_PCB_tray_black.stl) | 21.2 × 26.95 × 5.8 | Base down; PCB supports up |

Reference slicer settings: **0.4 mm nozzle, 0.2 mm layers, 3 walls, 15% infill, 5 top and 5 bottom layers, supports OFF**. Use the temperature and cooling settings for your actual filament. Print the coupon in the same filament and settings as the black enclosure. Add a brim to the small eyes if bed adhesion requires it. Keep the prepared orientations: they expose the tower cavity and avoid printing the mascot standing on its feet. The tower's tie tunnels introduce only 1.3 mm short internal shelf-return bridges. Inspect those, the arm recess and small holes in the layer preview for your slicer's bridging and thin-feature handling.

The mascot is a closed solid in CAD; slicer infill creates its internal structure. Do not print at 100% infill unless deliberately choosing that extra material and weight.

## Chamfered rectangular finish

This revision uses **flat chamfers only**, preserving the rectangular character. The mascot outline has 0.8 mm bevels, its face 0.6 mm, and the exposed foot/arm fronts 0.4 mm. The tower and cover share 0.6 mm corner bevels, with 0.4 mm outer face/bed chamfers. Eye corners use 0.35 mm bevels and their visible bed-facing perimeter 0.25 mm. LED bores have 0.3 mm front lead-ins; the USB opening has 0.8 mm clipped corners and a 0.3 mm rear mouth chamfer. Smaller bevels ease the removable retainer and tray.

The cover mating plane, LED flange seats, PCB bearing pads and eye locating tongues retain their fitting dimensions. Bed-facing chamfers slope at 45° instead of introducing shallow rounded overhangs. The source exposes the finishing dimensions as `*_CHAMFER` parameters. The domed LED reference shapes represent the purchased LEDs; they are not printed enclosure fillets.

The hand recess now has **full-depth 45° ramps at both ends**, replacing the horizontal ledges. A 2 mm end chamfer on the recess cutter keeps every internal recess surface at 45° or steeper to the bed in the supplied back-down print orientation. Keep that orientation and supports off; the middle of the recess retains its original depth.

## Coupon layout and hardware fit

The base has engraved sizes. The LED holes are at Y = 8 mm; the **C** screw row is at Y = 25 mm and the **R** row at Y = 40 mm. Both screw rows have **1.7, 1.8, 1.9 and 2.0 mm** blind pilots at X = 7, 20, 33 and 46 mm. Each row reproduces its production boss diameter and blind pilot depth. Test the actual screw through the matching cap, with its countersink facing the screw head; never drive the screw directly into a bare test post. Cap R intentionally prints upside down to reproduce the retainer's head-seat orientation.

The enclosure defaults to 5.3 mm LED holes for provisional 5.0 mm barrels, and 1.8 mm screw pilots. The rectangular pocket uses the same parameters as the production eye pocket. Print one eye alongside the coupon and check it enters without splitting the pocket. Remove first-layer flare before judging fits. Report the best pilot for **each of C and R**, plus the best LED hole for **each lamp**; the current model uses one shared pilot diameter for all five screws.

Measure all three LED barrels, flanges and flange thicknesses; the PCB outline and thickness; components above and below the PCB; the USB socket location; and the cable overmould. Confirm the tray's four small bearing pads and two ties land on clear PCB areas. The design does not assume PCB mounting holes. Check there is room for your resistor bodies, insulated joints and lead bends throughout the open tower. BOOT and RESET must remain accessible with the cover removed.

## Additional assembly items

- Five **DIN 7982 ST2.2 × 6.5 mm countersunk Phillips self-tapping screws, stainless 304**: three for the rear cover and two for the LED retainer. The same screw size is used throughout.
- Two **2.5 mm wide cable ties, no thicker than 1 mm**, for the PCB tray.
- A small amount of plastic-compatible glue for the two eyes and the arm-to-tower joint only.
- Optional thin, soft **0.25 mm shims** behind the LED flanges if axial play needs removing.

For these countersunk screws, **6.5 mm is the overall length including the head**. The model uses an **80° head seat, 4.6 mm mouth, 2.5 mm clearance bore and provisional 1.8 mm pilot**. The DIN 7982 reference specifies a 4.0–4.3 mm head diameter for ST2.2 and a 5.7–7.3 mm tolerance range for the nominal 6.5 mm length; the model's blind depths allow for that range and head seating. The 80° DIN / 90° ISO distinction is documented in [REYHER technical information, page 11](https://www.reyher.de/fileadmin/user_upload/Downloadbereich/Technische_Informationen_2016_11_EN_KS_klein.pdf#page=11). See the [Böllhoff DIN 7982 A2 product sheet](https://eshop.boellhoff.de/out/media/pdf/DIN_7982_Edelstahl_A2___en.pdf). Measure or test the purchased screws: matching a standard is not a physical fit test. Tighten by hand only until the plate is clamped; do not force the head flush or continue after seating.

## Assembly

1. Clean first-layer flare and loose strands from mating surfaces, LED holes and screw holes. Dry-fit the eyes, cover and tray before applying any glue.
2. Place the LEDs through the tower front from inside: **red at the top, yellow in the middle, green at the bottom**. Each flange bears against the inside front wall. Orient each LED's two leads as a **vertical pair along assembly Z**, matching the retainer's open slots. These slots open sideways so the retainer can later be removed without threading soldered leads through closed holes.
3. Fit the LED retainer with its raised pads toward the LED flanges and its countersinks toward the open rear. Secure it with two DIN 7982 ST2.2 × 6.5 mm screws at the relocated mounts below the green lamp and between the yellow/red lamps. The 3 mm structural plate sits at assembly Y = 5.8…8.8 mm; its pads extend forward to Y = 3.65 mm, preserving 0.25 mm nominal flange clearance. If needed, fit thin soft shims that do not obstruct the lead slots; do not crush a flange or clamp a lead with a screw. Keep LED packages replaceable and unglued.
4. Fit the PCB to the tray with its USB-C socket toward the rear. Route the two cable ties through the paired tray slots and around bare PCB areas. Tighten only enough to hold the board; do not press ties onto components or buttons. Insert the tray onto the pedestal's two shelves until it reaches the front stop, aligning the ties with the 1.3 mm deep shelf tunnels. Keep each tie's locking head clear of the shelf tunnels and cover. The tray and PCB must remain removable.
5. Wire and test the electronics using the existing [hardware wiring and resistor instructions](../../docs/hardware-setup.md#wiring-and-resistor-choice) and [wiring check](../../docs/hardware-setup.md#check-the-wiring-before-connecting-clawlight). Route insulated leads and resistors clear of the tray, screw bosses and cover contact surfaces. Leave approximately 30 mm of flexible service slack between the board and lamps so the tray can slide out; fold it loosely into the tower. Check USB insertion with the actual cable, the PCB supported, and no load or pinching on the wires.
6. Fit the rear cover with its countersinks facing outward and secure it with three DIN 7982 ST2.2 × 6.5 mm screws. It must seat without forcing the tray, wires or components. Confirm the cable overmould clears the opening and both buttons remain serviceable after removing the cover.
7. Glue the eyes into their shallow front pockets using a small amount on their locating tongues. Set the mascot's **four feet and the tower pedestal together on one flat desk surface**, dry-fit the arm contact, then glue the arm to the tower shell in that position. Apply arm glue only between **5 and 35 mm behind the tower's front face (Y = 5…35)**. **Do not glue the rear-cover contact at Y = 37.6…40**, even though the arm reaches that far back. The tower stands on its own pedestal. Keep glue away from the cover seam, screws and electronics so the cover can still be removed.

## Editable dimensions and rebuilding

The editable source is [clawlight.py](clawlight.py); the [assembled STEP](clawlight.step) contains **only the seven printed parts** in assembly coordinates. [parts.json](parts.json) records all ten print-oriented bounding boxes and solid volumes. The [coupon layout source](fit_coupon.py) defines the three coupon pieces in print orientation; its review export is [fit_coupon.step](fit_coupon.step). For visual review, [appearance.step](appearance.step) and [exploded.step](exploded.step) include illustrative, unmeasured electronics; these are not extra printable parts. Their view sources are [appearance.py](appearance.py) and [exploded.py](exploded.py).

The source currently assumes:

| Parameter(s) | Provisional default / purpose |
| --- | --- |
| `LED_DIAMETERS`, `LED_FLANGES` | Three 5.0 mm barrels and three 6.0 mm flanges; order is green, yellow, red |
| `LED_FLANGE_THICKNESS`, `LED_BARREL_LENGTH` | 1.0 mm flange thickness; 8.5 mm flange-front-to-tip length |
| `LED_PITCH`, `LED_BORE_CLEARANCE` | 12 mm centre pitch; 0.30 mm diametral barrel clearance |
| `LED_RETAINER_GAP`, `LEAD_SLOT` | 0.25 mm axial play; 3.6 mm open slot for a vertically oriented lead pair |
| `PCB_WIDTH`, `PCB_LENGTH`, `PCB_THICKNESS` | 18 × 22.5 × 1.6 mm PCB |
| `PCB_UNDERSIDE`, `PCB_TOP_HEIGHT` | 2 mm below and 4 mm above the PCB component envelopes; 0.4 mm spare between the assumed underside envelope and tray floor |
| `PCB_REAR`, `PCB_BOTTOM` | Rear PCB edge Y = 35.2 mm; lower PCB surface Z = 9.0 mm |
| `USB_WIDTH`, `USB_HEIGHT`, `USB_CENTER_Z`, `USB_REAR` | 9 × 3.5 mm shell; Z = 12.1 mm centre, derived from PCB height; rear face Y = 37.6 mm |
| `USB_OPENING_WIDTH`, `USB_OPENING_HEIGHT` | 14 × 10 mm cover opening; check the real cable overmould |
| `FIT` | 0.25 mm clearance per side |
| `SCREW_DIAMETER`, `SCREW_LENGTH`, `SCREW_LENGTH_TOLERANCE` | DIN 7982 ST2.2 × 6.5 mm; ±0.8 mm overall length allowance |
| `SCREW_HEAD_DIAMETER_MIN`, `SCREW_HEAD_DIAMETER_MAX`, `SCREW_HEAD_ANGLE_DEG` | 4.0–4.3 mm head; 80° included angle |
| `SCREW_PILOT_DIAMETER`, `SCREW_CLEARANCE`, `COUNTERSINK_DIAMETER` | Provisional 1.8 mm pilot; 2.5 mm clearance bore; 4.6 mm head seat mouth |
| `COVER_PILOT_DEPTH`, `RETAINER_PILOT_DEPTH` | Derived blind depths, approximately 5.56 and 4.96 mm; coupon rows reproduce them |
| `RETAINER_MOUNT_Y`, `RETAINER_BACK_Y`, `RETAINER_Y` | 3 mm plate at Y = 5.8…8.8; front flange pads at Y = 3.65 mm |
| `LED_SCREWS` | Retainer screw centres (X, Z) = (45.8, 38) and (45.8, 62) mm |
| `COUPON_PILOT_DIAMETERS`, `COUPON_LED_DIAMETERS` | Pilot samples 1.7 / 1.8 / 1.9 / 2.0 mm; LED samples 5.1 / 5.3 / 5.5 mm |
| `EYE_CLEARANCE` | 0.30 mm pocket clearance per side around the eye locating tongue |
| `WALL`, `COVER`, `DEPTH` | 2.4 mm nominal wall; 2.4 mm cover; 40 mm assembled depth |

`EYE_TONGUE_*`, `EYE_POCKET_DEPTH` and `EYE_CLEARANCE` control the eye and both production/coupon pockets together. `coupon_parts()` supplies the final cap print poses: the raw `coupon_cap(...)` builder creates both caps with their countersinks up, then the R export is flipped. Use the exported STL poses. Tie positions are defined in both `pcb_tray()` and the corresponding shelf tunnels in `tower()`; move them together if required by the actual PCB. LED flange thickness is shared by the three lamps; unlike barrel and flange diameter, it is not an independent value per lamp.

This layout has fixed pedestal, upright and fastener coordinates. Larger boards or LED flanges may require changing the surrounding geometry, not only one parameter; the source checks reject a tray wider than the available pedestal or a flange diameter of 8.5 mm or more. Recheck the entire fit and assembly after any dimension change.

From an environment with **build123d 0.11.1**, run `python models/clawlight/clawlight.py` from the `clawlight` repository root to regenerate all ten STLs, the printed-assembly STEP and parts metadata. Re-export auxiliary views from their `gen_step()` functions after changes. Re-run the accompanying checks and inspect the updated sliced parts before printing. Physical fit remains unverified until tested with your actual hardware.

## Checks and previews

[check.py](check.py) passed for this revision: all **10 STL meshes are watertight, single-component and positioned on Z = 0**; the seven assembly solids have no mutual volume interference. Checks cover provisional electronics, ties, wire and screwdriver access, withdrawal paths, common desk plane, and both coupon cap rows. At the largest modeled screw length and deepest head seating, the tip retains 0.30 mm clearance to the blind pilot floor; the retainer boss retains approximately 0.84 mm of front material. Results are in [validation.json](validation.json). Install [requirements.txt](requirements.txt), regenerate, then run `python models/clawlight/check.py` from the `clawlight` repository root to repeat the checks.

All **10 regenerated STLs** sliced successfully in **Bambu Studio 02.07.01.62**, using a reference **A1 / 0.4 mm / Generic PLA** profile: 0.2 mm layers, 3 walls, 15% infill, supports off and one extruder. The run reported **zero geometry warnings, support toolpaths or filament changes**. [slicer-validation.json](slicer-validation.json) records the checked STL hashes and effective settings. Estimated consumption is **84.2 g / 4 h 23 min**, including the three coupon parts and ten separate print startups; grouping same-colour parts changes that estimate. This is reference slicing, not G-code or a profile for your unconfirmed printer. **No physical print or hardware-fit test has been performed.**

- [Three-piece coupon](preview/coupon_20260916T173845Z.png) and [top view with hole labels](preview/coupon_top_20260916T173845Z.png).
- [Cover countersinks](preview/cover_screws_20260916T173845Z.png) and [LED-retainer countersinks](preview/retainer_screws_20260916T173845Z.png).
- [Assembled preview](preview/finished_20260916T173845Z.png), with illustrative LEDs.
- [Printed assembly](preview/assembly_20260916T173845Z.png) and [front view](preview/front_20260916T173845Z.png).
- [Rear service access](preview/service_20260916T173845Z.png), cover hidden.
- [Exploded assembly](preview/exploded_20260916T173845Z.png).
- [Hand recess, front close-up](preview/hand_front_20260916T173845Z.png) and [rear close-up](preview/hand_rear_20260916T173845Z.png), showing both 45° ramps.
- [Dimensioned drawing](preview/dimensions.png), also available as [SVG](preview/dimensions.svg).

The local CAD viewer is **stopped**. To use the links below, start the existing viewer manually with this command; it stops automatically after 12 hours. Neutral shading makes the black parts' internal features visible; choose orange and black filament for the actual object.

```sh
npm --prefix "$HOME/.agents/skills/cad-viewer/scripts/viewer" run start -- --host 127.0.0.1 --dir "/Users/mahenzon/Documents/ChatGPT/bro-agent-light/clawlight/models/clawlight" --shutdown-after 12h
```

| File | Live review |
| --- | --- |
| clawlight.step | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=clawlight.step) |
| appearance.step | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=appearance.step) |
| exploded.step | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=exploded.step) |
| stl/00_fit_coupon.stl | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=stl%2F00_fit_coupon.stl) |
| fit_coupon.step | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=fit_coupon.step) |
| stl/00b_coupon_cover_cap.stl | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=stl%2F00b_coupon_cover_cap.stl) |
| stl/00c_coupon_retainer_cap.stl | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=stl%2F00c_coupon_retainer_cap.stl) |
| stl/01_mascot_orange.stl | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=stl%2F01_mascot_orange.stl) |
| stl/02_eye_left_black.stl | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=stl%2F02_eye_left_black.stl) |
| stl/03_eye_right_black.stl | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=stl%2F03_eye_right_black.stl) |
| stl/04_tower_black.stl | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=stl%2F04_tower_black.stl) |
| stl/05_rear_cover_black.stl | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=stl%2F05_rear_cover_black.stl) |
| stl/06_LED_retainer_black.stl | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=stl%2F06_LED_retainer_black.stl) |
| stl/07_PCB_tray_black.stl | [Open in 3D](http://127.0.0.1:4178/?dir=%2FUsers%2Fmahenzon%2FDocuments%2FChatGPT%2Fbro-agent-light%2Fclawlight%2Fmodels%2Fclawlight&file=stl%2F07_PCB_tray_black.stl) |
