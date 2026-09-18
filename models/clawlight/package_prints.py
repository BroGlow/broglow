"""Package the current print inventory; run after clawlight.py and check.py."""
import argparse
import hashlib
import json
import posixpath
import re
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

MODEL = Path(__file__).resolve().parent
ROOT = MODEL.parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='Verify existing archives without writing files.')
    args = parser.parse_args()
    metadata = json.loads((MODEL/'parts.json').read_text())
    validation = json.loads((MODEL/'validation.json').read_text())
    slices = json.loads((MODEL/'slicer-validation.json').read_text())
    files = sorted((MODEL/'stl').rglob('*.stl'))
    assert validation['status'] == 'passed'
    assert len(files) == len(metadata) == 8
    assert {str(f.relative_to(MODEL)) for f in files} == {p['file'] for p in metadata.values()}
    assert {p['file'] for p in slices['parts']} == {p['file'] for p in metadata.values()}
    for part in slices['parts']:
        assert hashlib.sha256((MODEL/part['file']).read_bytes()).hexdigest() == part['sha256'], \
            f"Stale slicer evidence: {part['file']}"
    guide = '\n'.join((MODEL/name).read_text() for name in ('PRINT_GUIDE.en.md', 'PRINT_GUIDE.md'))
    for link in re.findall(r'\]\(([^)]+)\)', guide):
        if '://' not in link and not link.startswith('#'):
            assert (MODEL/link.split('#')[0]).exists(), link
    readme = '''# Claw Light — enclosure release candidate

A1 Mini / PETG Basic / 0.4 mm nozzle / 0.2 mm layers.
Use the exported orientations, 100% scale, 3 walls, 15% infill,
5 top/bottom layers and no supports. Disable automatic orientation.

01_mascot_orange.stl
02_eye_black_x2.stl       — one eye in the file; print TWO copies
03_tower_black.stl
04_rear_cover_black.stl
05_LED_retainer_black.stl
06_PCB_tray_black.stl
coupons/01_USB_base_coupon.stl
coupons/02_USB_cover_coupon.stl

Print one of each except the eye (x2). Main assembly: seven physical parts.
For the USB fit check, print both coupons plus tray 06.
Use your actual PCB/cable and two DIN 7982 ST2.2 x 6.5 screws for the test.
The tested tray can then be used in the final assembly.

Completed LED/screw and hand-joint coupons have been removed. Their tested
sizes remain: LED bore 5.1 mm, screw pilot 1.9 mm, hand socket 3.3 mm.
Hand pins provide positioning; use glue for permanent attachment.
Eye rear tongue: 0.4 mm corner chamfers and 0.2 mm insertion-edge chamfer.
Print the eye front-face down, mounting tongue upward; print two copies.

Flat-floor revision: 2.0 mm floor without the central recess; no cover tabs.
Tray and USB window both lowered 0.8 mm; the measured +1.2 mm USB correction
relative to the PCB is retained. Window 9.3 x 3.5 mm, entry chamfers 0.2 mm
on both sides; inner perimeter, pocket rim and screw entries chamfered 0.2 mm,
body depth 27.4 mm, no projecting base rail fingers.
Reprint the base, cover and tray together; modeled USB projection is 0..0.5 mm.
Inspect USB cover coupon walls/infill: its successful slice logged an internal
ZFiller diagnostic. No entire missing layers or components were found.
Physical fit needs testing with the revised USB fixture and tray.
The full package includes PRINT_GUIDE.en.md and PRINT_GUIDE.md, STEP files, source and check reports.
The Mac app and prebuilt firmware are separate release downloads.
'''
    readme_ru = '''<a name="claw-light-enclosure-release-candidate"></a>

# Claw Light — релиз-кандидат корпуса

A1 Mini / PETG Basic / сопло 0,4 мм / слой 0,2 мм.
Используйте экспортированные ориентации, масштаб 100%, 3 стенки, 15% заполнения,
5 верхних/нижних слоёв, без поддержек. Отключите автоматическую ориентацию.

01_mascot_orange.stl
02_eye_black_x2.stl       — в файле один глаз; напечатайте ДВЕ копии
03_tower_black.stl
04_rear_cover_black.stl
05_LED_retainer_black.stl
06_PCB_tray_black.stl
coupons/01_USB_base_coupon.stl
coupons/02_USB_cover_coupon.stl

Напечатайте по одному экземпляру, кроме глаза (x2). Основная сборка: семь деталей.
Для проверки USB напечатайте оба пробника и лоток 06.
Используйте настоящую плату/кабель и два самореза DIN 7982 ST2.2 x 6,5.
Проверенный лоток затем можно установить в окончательную сборку.

Проверенные пробники LED/саморезов и соединения руки удалены. Их проверенные
размеры сохранены: отверстие LED 5,1 мм, пилот самореза 1,9 мм, гнездо руки 3,3 мм.
Штифты руки позиционируют детали; для постоянного крепления используйте клей.
Задний язычок глаза: фаски углов 0,4 мм и входной кромки 0,2 мм.
Печатайте глаз лицом вниз, монтажным язычком вверх; нужны две копии.

Версия с плоским дном: дно 2,0 мм без центральной выборки; на крышке нет выступов.
Лоток и окно USB опущены на 0,8 мм; измеренная поправка USB +1,2 мм
относительно платы сохранена. Окно 9,3 x 3,5 мм, входные фаски 0,2 мм
с обеих сторон; внутренний периметр, край выборки и входы винтовых отверстий
имеют фаски 0,2 мм; глубина корпуса 27,4 мм, выступающих пальцев направляющих нет.
Перепечатайте основание, крышку и лоток вместе; выступ USB в модели — 0..0,5 мм.
Проверьте стенки/заполнение крышки USB-пробника: успешная нарезка выдала внутреннее
сообщение ZFiller. Целые пропущенные слои или компоненты не обнаружены.
Физическую посадку нужно проверить с изменённым USB-пробником и лотком.
Полный комплект содержит PRINT_GUIDE.en.md и PRINT_GUIDE.md, STEP, исходники и отчёты проверок.
Приложение Mac и готовая прошивка скачиваются отдельно из релиза.
'''
    packs = {
        'clawlight-compact-STL.zip': [(f, str(f.relative_to(MODEL/'stl'))) for f in files],
        'clawlight-USB-fit-test.zip': [(f, str(f.relative_to(MODEL/'stl'))) for f in files
                                     if f.parent.name == 'coupons' or f.stem == '06_PCB_tray_black'],
    }
    reference_files = [
        'docs/hardware-setup.md', 'docs/hardware-setup.ru.md',
        'docs/firmware-reference.md', 'docs/firmware-reference.ru.md',
        'firmware/esp32-c3-supermini/platformio.ini',
        'firmware/esp32-c3-supermini/src/main.cpp',
        'firmware/esp32-c3-supermini/src/led-test.cpp',
        'firmware/esp32-c3-supermini/tests/check.cpp',
        'firmware/esp32-c3-supermini/tests/check.py',
        *(f'assets/icons/clawd-{color}.png' for color in ('none', 'green', 'yellow', 'red')),
    ]
    full = [(ROOT/name, name) for name in reference_files]
    for f in sorted(MODEL.rglob('*')):
        if not f.is_file() or any(x.startswith('.') or x == '__pycache__' for x in f.relative_to(MODEL).parts):
            continue
        if f.suffix not in ('.py', '.step', '.stl', '.md', '.txt', '.png', '.svg') and \
                f.name not in ('parts.json', 'validation.json', 'slicer-validation.json'):
            continue
        if f.parent.name == 'preview' and str(f.relative_to(MODEL)) not in guide:
            continue
        full.append((f, str(f.relative_to(ROOT))))
    packs['clawlight-print-pack.zip'] = full
    full_names = {target for _, target in full}
    for source, target in full:
        if source.suffix != '.md':
            continue
        for link in re.findall(r'\]\(([^)]+)\)', source.read_text()):
            if '://' in link or link.startswith('#'):
                continue
            resolved = posixpath.normpath(posixpath.join(posixpath.dirname(target), link.split('#')[0]))
            assert resolved in full_names, f'Missing linked package file: {target} -> {link}'
    for name, entries in packs.items():
        entries = [(ROOT/'LICENSE', 'LICENSE'), *entries]
        path = MODEL.parent/name
        if name == 'clawlight-USB-fit-test.zip':
            text = '''# USB fit test — enclosure release candidate

Print one each: 06_PCB_tray_black.stl, coupons/01_USB_base_coupon.stl,
and coupons/02_USB_cover_coupon.stl. The tray is reused in the final assembly.
A1 Mini / PETG Basic / 0.4 mm nozzle / 0.2 mm layers; 3 walls, 15% infill,
5 top/bottom layers. Keep exported orientations, 100% scale, no supports.
Use the actual PCB/cable and two DIN 7982 ST2.2 x 6.5 screws.
Check tray removal, lid closure, USB alignment and full cable insertion.
Revision: flat 2.0 mm floor, no cover tabs, depth 27.4 mm. Tray and USB window
both lowered 0.8 mm; measured +1.2 mm correction relative to PCB retained.
Cover inner edges and both USB entries now have 0.2 mm chamfers.
Use the new tray with both new coupon parts; do not mix with the earlier set.
Inspect cover-coupon walls/infill before printing: its successful slice logged
one internal ZFiller diagnostic, retained in the full report.
'''
            text_ru = '''<a name="usb-fit-test-enclosure-release-candidate"></a>

# Примерка USB — релиз-кандидат корпуса

Напечатайте по одному: 06_PCB_tray_black.stl, coupons/01_USB_base_coupon.stl
и coupons/02_USB_cover_coupon.stl. Лоток затем используется в окончательной сборке.
A1 Mini / PETG Basic / сопло 0,4 мм / слой 0,2 мм; 3 стенки, 15% заполнения,
5 верхних/нижних слоёв. Сохраните экспортированные ориентации, масштаб 100%, без поддержек.
Используйте настоящую плату/кабель и два самореза DIN 7982 ST2.2 x 6,5.
Проверьте извлечение лотка, закрытие крышки, совпадение USB и полную посадку кабеля.
Версия: плоское дно 2,0 мм, без выступов крышки, глубина 27,4 мм. Лоток и окно USB
опущены на 0,8 мм; измеренная поправка +1,2 мм относительно платы сохранена.
На внутренних кромках крышки и обоих входах USB теперь фаски 0,2 мм.
Используйте новый лоток с обеими новыми частями пробника; не смешивайте их с прежним комплектом.
Перед печатью проверьте стенки/заполнение крышки пробника: при успешной нарезке выдано
одно внутреннее сообщение ZFiller, сохранённое в полном отчёте.
'''
        else:
            text = readme
            text_ru = readme_ru
        language_links = '[English](START_HERE.md) | [Русский](START_HERE.ru.md)\n\n'
        text = language_links + text
        text_ru = language_links + text_ru
        if name == 'clawlight-print-pack.zip':
            text += '\n[Full print guide](models/clawlight/PRINT_GUIDE.en.md)\n'
            text_ru += '\n[Полное руководство по печати](models/clawlight/PRINT_GUIDE.md)\n'
        manuals = {'START_HERE.md': text, 'START_HERE.ru.md': text_ru}
        entry_names = {*manuals, *(target for _, target in entries)}
        for filename, content in manuals.items():
            for link in re.findall(r'\]\(([^)]+)\)', content):
                if '://' not in link and not link.startswith('#'):
                    assert link.split('#')[0] in entry_names, f'Missing entry-manual link: {name}/{filename} -> {link}'
        if not args.check:
            with ZipFile(path, 'w', ZIP_DEFLATED) as archive:
                for filename, content in manuals.items():
                    archive.writestr(filename, content)
                for f, target in entries:
                    archive.write(f, target)
        with ZipFile(path) as archive:
            assert archive.testzip() is None, f'Corrupt archive: {path}'
            expected = [*manuals, *(target for _, target in entries)]
            assert sorted(archive.namelist()) == sorted(expected), f'Stale archive inventory: {path}'
            for filename, content in manuals.items():
                assert archive.read(filename) == content.encode(), f'Stale printing instructions: {path}/{filename}'
            for f, target in entries:
                assert hashlib.sha256(archive.read(target)).digest() == hashlib.sha256(f.read_bytes()).digest(), \
                    f'Stale archive content: {path.name}/{target}'
        print('Verified:', path)
    if not args.check:
        for name in ('clawlight-hand-joint-test.zip', 'clawlight-fit-coupon-DIN7982-2.2x6.5.zip'):
            (MODEL.parent/name).unlink(missing_ok=True)


if __name__ == '__main__':
    main()
