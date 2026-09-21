"""Package the current print inventory; run after broglow.py and check.py."""

import argparse
import hashlib
import json
import posixpath
import re
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

MODEL = Path(__file__).resolve().parent
ROOT = MODEL.parent.parent
REFERENCE_FILES = [
    "Cargo.toml",
    "scripts/package-enclosure.py",
    "scripts/package-macos.py",
    "site/guide/en/hardware.md",
    "site/guide/ru/hardware.md",
    "site/guide/en/firmware.md",
    "site/guide/ru/firmware.md",
    "firmware/esp32-c3-supermini/platformio.ini",
    "firmware/esp32-c3-supermini/src/main.cpp",
    "firmware/esp32-c3-supermini/src/led-test.cpp",
    "firmware/esp32-c3-supermini/tests/check.cpp",
    "firmware/esp32-c3-supermini/tests/check.py",
    *(
        f"assets/icons/broglow-{color}.png"
        for color in ("none", "green", "yellow", "red")
    ),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify existing archives without writing files.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=ROOT / "dist/enclosure",
        help="Directory for generated ZIP files (default: dist/enclosure).",
    )
    args = parser.parse_args()
    output = args.output_dir.resolve()
    if not args.check:
        output.mkdir(parents=True, exist_ok=True)
    metadata = json.loads((MODEL / "parts.json").read_text())
    validation = json.loads((MODEL / "validation.json").read_text())
    slices = json.loads((MODEL / "slicer-validation.json").read_text())
    files = sorted((MODEL / "stl").rglob("*.stl"))
    assert validation["status"] == "passed"
    assert files and len(files) == len(metadata), "Missing or duplicate printable parts"
    assert {f.relative_to(MODEL).as_posix() for f in files} == {
        p["file"] for p in metadata.values()
    }
    assert len(slices["parts"]) == len(files)
    assert {p["file"] for p in slices["parts"]} == {
        p["file"] for p in metadata.values()
    }
    assert (
        slices["result"]["files_checked"]
        == slices["result"]["successful"]
        == len(files)
    )
    for field in (
        "geometry_warnings",
        "files_with_support_toolpaths",
        "repair_warnings",
        "floating_or_unsupported_region_warnings",
        "nonfatal_slicer_diagnostic_count",
    ):
        assert slices["result"][field] == 0, f"Slicer evidence reports {field}"
    for name in ("broglow", "appearance", "exploded", "usb_coupon"):
        step = MODEL / f"{name}.step"
        assert (
            step.is_file() and step.stat().st_size
        ), f"Missing required STEP view: {step}"
    for part in slices["parts"]:
        assert part["status"] == "success", f"Failed slicer evidence: {part['file']}"
        assert not any(
            part[field]
            for field in (
                "geometry_warnings",
                "support_toolpaths",
                "repair_warnings",
                "floating_or_unsupported_region_warnings",
                "slicer_log_diagnostics",
            )
        ), f"Slicer evidence reports warnings: {part['file']}"
        assert (
            hashlib.sha256((MODEL / part["file"]).read_bytes()).hexdigest()
            == part["sha256"]
        ), f"Stale slicer evidence: {part['file']}"
    guide = "\n".join(
        (MODEL / name).read_text() for name in ("PRINT_GUIDE.en.md", "PRINT_GUIDE.md")
    )
    for link in re.findall(r"\]\(([^)]+)\)", guide):
        if "://" not in link and not link.startswith("#"):
            assert (MODEL / link.split("#")[0]).exists(), link
    readme = """# BroGlow — enclosure

A1 Mini / PETG Basic / 0.4 mm nozzle / 0.2 mm layers.
Use exported orientations, 100% scale, 3 walls, 15% infill,
5 top/bottom layers and no supports. Disable automatic orientation.

01_mascot_orange.stl
02_eye_black_x2.stl       — one eye in the file; print TWO copies
03_tower_black.stl
04_rear_cover_black.stl
05_LED_retainer_black.stl
coupons/01_USB_base_coupon.stl
coupons/02_USB_cover_coupon.stl

Main assembly: five unique STLs, six physical pieces including two eyes.
The complete inventory is seven unique STLs, eight pieces including the two coupons.
USB fit test: the two coupon parts, actual PCB/cable and two DIN 7982 ST2.2 x 6.5 screws.

For assembly, use FOUR DIN 7982 ST2.2 x 6.5 screws: three cover, one retainer.
Tighten by hand. Print the cover and retainer flat outer face down, countersinks
facing the bed; print both eyes face down, tongues upward.

Dry-fit the USB coupons first. The 14 mm crop checks PCB/USB fit only; check the
LED retainer, upper tab pocket and wiring in the full tower.
Slide the PCB straight from the open rear, USB facing backward. Keep it centered:
modeled components clear at +/-0.15 mm, not the full +/-0.60 mm bare-board travel.
Check GPIO solder, edge components and bare 2.7 x 1.0 mm patches at both far corners.
Align USB before closing; do not force the board or use screws to pull it into place.
Check cable seating and clearance above the table.

Insert LEDs red/yellow/green from top to bottom. Pass the red LED around the upper
screw boss on the mascot side, then align it with its hole. Keep leads clear of
flange seats and on the retainer's open side, away from the mascot.
Insert retainer 05 from the rear 1.3 mm below its final position; slide it up
1.3 mm into the roof pocket and fasten the lower screw. Leave the cover off for
wiring and electrical checks. Keep wires clear of the guides, tab pocket and
screws, with enough slack to withdraw the PCB; do not pinch wires or load components.

Move the mascot's hand pins straight toward the tower along +X by 1.8 mm. All four
feet and the tower base must stand on the same plane. Dry-fit before gluing the
hand and eyes; keep glue away from the cover, electronics and PCB guides.

Disconnect power before servicing. To remove the PCB, remove the cover and slide
it straight back without pulling wires. To remove the retainer, support the LEDs,
remove its screw, slide down 1.3 mm and withdraw rearward, keeping leads clear.

The full enclosure package includes the assembly/wiring guides, STEP, source and
check reports. App and firmware downloads are separate.
"""
    readme_ru = """<a name="broglow-enclosure"></a>

# BroGlow — корпус

A1 Mini / PETG Basic / сопло 0,4 мм / слой 0,2 мм.
Сохраните экспортированные ориентации, масштаб 100%, 3 стенки, 15% заполнения,
5 верхних/нижних слоёв, без поддержек. Отключите автоматическую ориентацию.

01_mascot_orange.stl
02_eye_black_x2.stl       — в файле один глаз; напечатайте ДВЕ копии
03_tower_black.stl
04_rear_cover_black.stl
05_LED_retainer_black.stl
coupons/01_USB_base_coupon.stl
coupons/02_USB_cover_coupon.stl

Основная сборка: пять уникальных STL, шесть деталей с двумя глазами.
Весь комплект: семь уникальных STL, восемь деталей вместе с двумя пробниками.
USB-примерка: две детали пробника, настоящая плата/кабель и два DIN 7982 ST2.2 x 6,5.

Для сборки нужны ЧЕТЫРЕ DIN 7982 ST2.2 x 6,5: три для крышки, один для держателя.
Затягивайте вручную. Крышку и держатель печатайте плоской наружной стороной вниз,
зенковками к столу; оба глаза — лицом вниз, язычками вверх.

Сначала примерьте USB-пробники. Срез высотой 14 мм проверяет только плату и USB;
держатель LED, верхний карман язычка и проводку проверяйте в полной башне.
Вставляйте плату прямо сзади, USB назад. Держите её по центру: модель компонентов
проходит при +/-0,15 мм, но не на всём ходе голой платы +/-0,60 мм.
Проверьте пайку GPIO, компоненты у краёв и свободные участки 2,7 x 1,0 мм
в обоих дальних углах. Совместите USB до закрытия; не вдавливайте плату
и не подтягивайте её саморезами. Проверьте посадку кабеля и зазор над столом.

Вставьте LED сверху вниз: красный, жёлтый, зелёный. Красный проведите мимо верхней
стойки самореза со стороны маскота, затем совместите с отверстием. Уложите выводы
вне опор фланцев, с открытой стороны держателя, дальше от маскота.
Подведите держатель 05 сзади на 1,3 мм ниже окончательного положения, сдвиньте
вверх на 1,3 мм в карман крыши и закрепите нижним саморезом. Оставьте крышку
снятой для проводки и электрических проверок. Уложите провода вне направляющих,
кармана язычка и саморезов, с запасом для извлечения платы; не зажимайте провода
и не нагружайте компоненты.

Вставьте штифты руки прямо к башне по +X на 1,8 мм. Все четыре ноги маскота
и основание башни должны стоять на одной плоскости. До склеивания руки и глаз
соберите насухо; не допускайте клея на крышку, электронику и направляющие платы.

Перед обслуживанием отключите питание. Для снятия платы снимите крышку и выдвиньте
плату прямо назад, не натягивая провода. Для снятия держателя придержите LED,
открутите саморез, сдвиньте держатель вниз на 1,3 мм и извлеките назад,
не задевая выводы.

Полный комплект корпуса содержит инструкции по сборке/проводке, STEP, исходники
и отчёты. Приложение и прошивка скачиваются отдельно.
"""
    packs = {
        "broglow-compact-STL.zip": [
            (f, f.relative_to(MODEL / "stl").as_posix()) for f in files
        ],
        "broglow-USB-fit-test.zip": [
            (f, f.relative_to(MODEL / "stl").as_posix())
            for f in files
            if f.parent.name == "coupons"
        ],
    }
    full = [(ROOT / name, name) for name in REFERENCE_FILES]
    if (ROOT / "BUILD.json").is_file():
        full.append((ROOT / "BUILD.json", "BUILD.json"))
    for f in sorted(MODEL.rglob("*")):
        if not f.is_file() or any(
            x.startswith(".") or x == "__pycache__" for x in f.relative_to(MODEL).parts
        ):
            continue
        if f.suffix not in (
            ".py",
            ".step",
            ".stl",
            ".md",
            ".txt",
            ".png",
            ".svg",
        ) and f.name not in ("parts.json", "validation.json", "slicer-validation.json"):
            continue
        relative = f.relative_to(MODEL)
        if relative.parts[0] == "preview" and relative.as_posix() not in (
            "preview/dimensions.png",
            "preview/dimensions.svg",
        ):
            continue
        full.append((f, f.relative_to(ROOT).as_posix()))
    packs["broglow-print-pack.zip"] = full
    full_names = {target for _, target in full}
    for source, target in full:
        if source.suffix != ".md":
            continue
        for link in re.findall(r"\]\(([^)]+)\)", source.read_text()):
            if "://" in link or link.startswith("#"):
                continue
            resolved = posixpath.normpath(
                posixpath.join(posixpath.dirname(target), link.split("#")[0])
            )
            assert (
                resolved in full_names
            ), f"Missing linked package file: {target} -> {link}"
    for name, entries in packs.items():
        entries = [(ROOT / "LICENSE", "LICENSE"), *entries]
        path = output / name
        if name == "broglow-USB-fit-test.zip":
            text = """# USB fit test — enclosure

Print coupons/01_USB_base_coupon.stl and coupons/02_USB_cover_coupon.stl.
Coupon width 24 mm, height 14 mm. PCB guides and USB retain the low-board geometry.
This crop omits the LED retainer and upper tab pocket. It cannot validate LED retention;
test those parts in the full tower. Use the current matching coupon files.
There is no separate tray.
Use the actual PCB/cable and two DIN 7982 ST2.2 x 6.5 screws.
A1 Mini / PETG Basic / 0.4 mm nozzle / 0.2 mm layers; 3 walls, 15% infill,
5 top/bottom layers. Keep exported orientations, 100% scale, no supports.

Slide the horizontal PCB straight into the integral C-guides from the open rear,
USB facing backward, until the far edge reaches the front stops. Close the cover
without bending the board; its stops capture the PCB. Leave wiring slack for removal.
Check insertion/removal, actual GPIO solder and edge-component clearance, lid closure,
USB alignment and full cable seating. Do not force the PCB into the slots.
Keep the PCB centered: modeled components clear at +/-0.15 mm, not the full
+/-0.60 mm bare-board play. Check bare 2.7 x 1.0 mm patches at both far corners,
actual underside solder and cable clearance above the table. Align USB before
closing; do not pull the board into place with screws. Tighten screws by hand.
Inspect cover-coupon walls and infill in the slicer before printing.
A successful coupon test does not replace final wiring and assembly checks.
"""
            text_ru = """<a name="usb-fit-test-enclosure"></a>

# Примерка USB — корпус

Напечатайте coupons/01_USB_base_coupon.stl и coupons/02_USB_cover_coupon.stl.
Ширина пробника 24 мм, высота 14 мм. Направляющие и USB сохраняют низкое положение платы.
В этом срезе нет держателя LED и верхнего кармана язычка; он не проверяет удержание LED.
Проверьте эти детали на полной башне. Используйте текущие согласованные файлы пробника.
Отдельного лотка нет.
Используйте настоящую плату/кабель и два DIN 7982 ST2.2 x 6,5.
A1 Mini / PETG Basic / сопло 0,4 мм / слой 0,2 мм; 3 стенки, 15% заполнения,
5 верхних/нижних слоёв. Сохраните ориентации, масштаб 100%, без поддержек.

Вставьте горизонтальную плату прямо сзади в С-образные направляющие, USB назад,
до передних упоров. Закройте крышку без изгиба платы; её упоры удерживают плату.
Оставьте запас провода для извлечения. Проверьте установку/извлечение,
реальные GPIO-пайку и компоненты у краёв, закрытие крышки, совпадение USB
и полную посадку кабеля. Не вдавливайте плату в пазы.
Держите плату по центру: модель компонентов проходит при +/-0,15 мм, но не на всём
боковом ходе голой платы +/-0,60 мм. Проверьте свободные участки 2,7 x 1,0 мм
в обоих дальних углах, настоящую пайку снизу и зазор кабеля над столом.
Совместите USB до закрытия; не подтягивайте плату саморезами. Затягивайте вручную.
Перед печатью просмотрите стенки и заполнение крышки в слайсере.
Удачная примерка пробника не заменяет проверку проводки и окончательной сборки.
"""
        else:
            text = readme
            text_ru = readme_ru
        language_links = "[English](START_HERE.md) | [Русский](START_HERE.ru.md)\n\n"
        text = language_links + text
        text_ru = language_links + text_ru
        if name == "broglow-print-pack.zip":
            text += "\n[Full print guide](models/broglow/PRINT_GUIDE.en.md)\n"
            text_ru += (
                "\n[Полное руководство по печати](models/broglow/PRINT_GUIDE.md)\n"
            )
        manuals = {"START_HERE.md": text, "START_HERE.ru.md": text_ru}
        entry_names = {*manuals, *(target for _, target in entries)}
        for filename, content in manuals.items():
            for link in re.findall(r"\]\(([^)]+)\)", content):
                if "://" not in link and not link.startswith("#"):
                    assert (
                        link.split("#")[0] in entry_names
                    ), f"Missing entry-manual link: {name}/{filename} -> {link}"
        if not args.check:
            with ZipFile(path, "w", ZIP_DEFLATED) as archive:
                for filename, content in manuals.items():
                    archive.writestr(filename, content)
                for f, target in entries:
                    archive.write(f, target)
        with ZipFile(path) as archive:
            assert archive.testzip() is None, f"Corrupt archive: {path}"
            expected = [*manuals, *(target for _, target in entries)]
            assert sorted(archive.namelist()) == sorted(
                expected
            ), f"Stale archive inventory: {path}"
            for filename, content in manuals.items():
                assert (
                    archive.read(filename) == content.encode()
                ), f"Stale printing instructions: {path}/{filename}"
            for f, target in entries:
                assert (
                    hashlib.sha256(archive.read(target)).digest()
                    == hashlib.sha256(f.read_bytes()).digest()
                ), f"Stale archive content: {path.name}/{target}"
        print("Verified:", path)
    if not args.check:
        for name in (
            "broglow-hand-joint-test.zip",
            "broglow-fit-coupon-DIN7982-2.2x6.5.zip",
        ):
            (output / name).unlink(missing_ok=True)


if __name__ == "__main__":
    main()
