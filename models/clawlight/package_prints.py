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
REFERENCE_FILES = [
    'Cargo.toml', 'scripts/package-enclosure.py', 'scripts/package-macos.py',
    'docs/hardware-setup.md', 'docs/hardware-setup.ru.md',
    'docs/firmware-reference.md', 'docs/firmware-reference.ru.md',
    'firmware/esp32-c3-supermini/platformio.ini',
    'firmware/esp32-c3-supermini/src/main.cpp',
    'firmware/esp32-c3-supermini/src/led-test.cpp',
    'firmware/esp32-c3-supermini/tests/check.cpp',
    'firmware/esp32-c3-supermini/tests/check.py',
    *(f'assets/icons/clawd-{color}.png' for color in ('none', 'green', 'yellow', 'red')),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='Verify existing archives without writing files.')
    parser.add_argument('--output-dir', type=Path, default=ROOT/'dist/enclosure',
                        help='Directory for generated ZIP files (default: dist/enclosure).')
    args = parser.parse_args()
    output = args.output_dir.resolve()
    if not args.check:
        output.mkdir(parents=True, exist_ok=True)
    metadata = json.loads((MODEL/'parts.json').read_text())
    validation = json.loads((MODEL/'validation.json').read_text())
    slices = json.loads((MODEL/'slicer-validation.json').read_text())
    files = sorted((MODEL/'stl').rglob('*.stl'))
    assert validation['status'] == 'passed'
    assert files and len(files) == len(metadata), 'Missing or duplicate printable parts'
    assert {f.relative_to(MODEL).as_posix() for f in files} == {p['file'] for p in metadata.values()}
    assert len(slices['parts']) == len(files)
    assert {p['file'] for p in slices['parts']} == {p['file'] for p in metadata.values()}
    assert slices['result']['files_checked'] == slices['result']['successful'] == len(files)
    for field in ('geometry_warnings', 'files_with_support_toolpaths', 'repair_warnings',
                  'floating_or_unsupported_region_warnings', 'nonfatal_slicer_diagnostic_count'):
        assert slices['result'][field] == 0, f'Slicer evidence reports {field}'
    for name in ('clawlight', 'appearance', 'exploded', 'usb_coupon'):
        step = MODEL/f'{name}.step'
        assert step.is_file() and step.stat().st_size, f'Missing required STEP view: {step}'
    for part in slices['parts']:
        assert part['status'] == 'success', f"Failed slicer evidence: {part['file']}"
        assert not any(part[field] for field in ('geometry_warnings', 'support_toolpaths',
                       'repair_warnings', 'floating_or_unsupported_region_warnings', 'slicer_log_diagnostics')), \
            f"Slicer evidence reports warnings: {part['file']}"
        assert hashlib.sha256((MODEL/part['file']).read_bytes()).hexdigest() == part['sha256'], \
            f"Stale slicer evidence: {part['file']}"
    guide = '\n'.join((MODEL/name).read_text() for name in ('PRINT_GUIDE.en.md', 'PRINT_GUIDE.md'))
    for link in re.findall(r'\]\(([^)]+)\)', guide):
        if '://' not in link and not link.startswith('#'):
            assert (MODEL/link.split('#')[0]).exists(), link
    readme = '''# Claw Light — enclosure release candidate

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

This revision adds 45-degree side-wall braces to the PCB guides and short retaining
lips at the two far-end corners opposite USB. Reprint only tower 03; use the new USB
base coupon for a first fit test. Cover 04, flat LED retainer 05, mascot and eyes are unchanged.
The LED retainer still uses a rigid upper tab in a roof pocket and one lower screw.
The full assembly uses FOUR DIN 7982 ST2.2 x 6.5 screws: three cover, one retainer.
The rear cover locates the PCB independently of the LED retainer. No separate tray is used.
Print retainer 05 flat outer rear face down, countersink toward the bed.
Tower: 14 mm upper width, 45.5 mm height; base 24 mm wide and 15.2 mm tall.
LED pitch 6.5 mm, centers Z=27/33.5/40 mm; bores remain 5.1 mm with 0.4 mm entries.
The 14 mm-high USB coupon checks only PCB/USB fit and the lower cover screws.
It omits the LED retainer and upper pocket: test their fit with the full tower.
Use the current matching coupon pair.
The PCB slides straight from the open rear into fixed C-guides, USB facing backward.
Each guide has 0.10 mm side clearance, a 0.80 mm slot for the 0.70 mm PCB,
a 0.60 mm lower bearing and a 0.30 mm upper overlap. Only the rear slot mouths
are chamfered 0.25 mm; long edges 0.10 mm. Wall/floor roots remain continuous to the seam. Guides meet the flat 2 mm floor and front wall and
stop at the cover seam. Their 45-degree top braces rise 1.3 mm at the side walls.
Each far-end corner lip covers 2.5 mm across the PCB and reaches 0.60 mm over its end,
leaving 13 mm open centrally. Lip entries have 0.15 mm chamfers and 45-degree wall braces.
The model assumes bare 2.7 x 1.0 mm patches at both far corners; check the actual board.
Edge-only front stops and existing cover stops locate the PCB.
Check actual edge components, GPIO solder joints and wire clearance; do not force the board.
Leave wiring slack; remove the cover and slide the board straight out for service.
Keep wires clear of the LED retainer, tab pocket and screw. Actual insulated-wire
sizes and bundle fit are unmeasured: check the real wiring before closing the cover.
The new integral fit needs a printed test; CAD clearances are nominal.

PCB bottom/top remain Z=2.8/3.5 mm. The USB opening remains 9.15 x 3.20 mm,
center Z=5.25 mm, entry chamfers 0.2 mm and corner bevels 0.15 mm.
The 0.8 mm floor gap allows the nominal 0.6 mm underside envelope plus 0.2 mm clearance.
The cable sits closer to the table; table contact depends on its molding.
Enclosure depth stays 27.4 mm. An earlier 0.7 mm physical USB recess was not
remeasured; do not assume modeled projection establishes the actual cable fit.
Mascot scale is 0.65: face 32.5 x 26 mm, total height 35.75 mm, depth 20.5 mm.
Eyes scale with the mascot. Hand-pin sizes remain 3 mm / 1.8 mm projection;
their new datum is X=26 mm, Y=6/16 mm, Z=24 mm, with 3.3 mm sockets.
Cover bosses retain 6.36 mm full-diameter sections, 5.56 mm pilots, 0.8 mm closed
ends and 45-degree side-wall ramps. Diameters 5.6/1.9 mm leave 1.85 mm radial walls.
Tighten the three cover screws and the one retainer screw by hand.
Measured LED plastic length is 8.5 mm TIP TO BOTTOM INCLUDING the 1.2 mm flange;
barrel length ahead of the flange is 7.3 mm. Nominal projection is 3.25–3.5 mm.
The flange seat is Y=3.8 mm, flange rear Y=5.0 mm, retainer support Y=5.25 mm,
leaving 0.25 mm axial play. Route leads clear of the retainer contact areas.
Insert the red LED past the upper screw boss slightly toward the mascot side,
then align it with its hole and push forward.
Keep LED leads on the open side of the retainer, away from the mascot. Insert the
retainer from the rear 1.3 mm below its final position, slide it up 1.3 mm into the
roof pocket, and secure the lower end with one screw. To remove it, unscrew, slide
down 1.3 mm, then withdraw rearward without pulling the leads.
Support the LEDs before releasing the retainer for service; removing the rear cover
alone leaves the LEDs retained.

Earlier tested dimensions remain: LED bore 5.1 mm, screw pilot 1.9 mm, hand socket 3.3 mm.
Completed LED/screw and hand-joint coupons are no longer included.
Hand pins position the parts; use glue for permanent attachment after a dry fit.
Eye rear tongue: 0.325 mm corner and 0.1625 mm insertion-edge chamfers.
Print both eyes front-face down, mounting tongues upward.
Inspect USB cover-coupon walls and infill before printing; current checks are in the slicer report.
The full package includes both print guides, STEP files, source and check reports.
The Mac app and prebuilt firmware are separate release downloads.
'''
    readme_ru = '''<a name="claw-light-enclosure-release-candidate"></a>

# Claw Light — релиз-кандидат корпуса

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

В этой версии направляющие платы усилены укосами 45 градусов к боковым стенкам
и короткими уголками на дальнем торце, противоположном USB. Перепечатайте только башню 03;
для первой примерки используйте новое основание USB-пробника. Крышка 04, плоский
держатель LED 05, маскот и глаза не изменены. Держатель по-прежнему крепится
жёстким верхним язычком в кармане крыши и одним нижним саморезом.
Для полной сборки нужны ЧЕТЫРЕ DIN 7982 ST2.2 x 6,5: три для крышки, один для держателя.
Крышка фиксирует плату независимо от держателя LED. Отдельного лотка нет.
Держатель 05 печатайте плоской наружной задней поверхностью вниз, зенковкой к столу.
Башня: ширина стойки 14 мм, высота 45,5 мм; основание шириной 24 мм и высотой 15,2 мм.
Шаг LED 6,5 мм, центры Z=27/33,5/40 мм; отверстия 5,1 мм, входные фаски 0,4 мм.
USB-пробник высотой 14 мм проверяет только плату/USB и нижние саморезы крышки.
Держателя LED и верхнего кармана в нём нет: проверяйте их на полной башне.
Используйте текущую согласованную пару пробников.
Плата вставляется прямо сзади в неподвижные С-образные направляющие, USB назад.
Боковой зазор — по 0,10 мм; паз высотой 0,80 мм для платы 0,70 мм;
нижняя опора заходит под край на 0,60 мм, верхняя полка перекрывает 0,30 мм.
Входные фаски 0,25 мм только на устьях пазов сзади, продольные 0,10 мм.
Основания у стенок и дна непрерывны до стыка крышки. Направляющие соединены с плоским
дном 2 мм и передней стенкой, заканчиваются у стыка крышки. Верхние укосы
45 градусов поднимаются на 1,3 мм у боковых стенок. Каждый дальний уголок занимает
2,5 мм по ширине платы и заходит над её торцом на 0,60 мм; по центру свободны 13 мм.
Уголки имеют входные фаски 0,15 мм и укосы к стенкам 45 градусов. Модель предполагает
свободные участки 2,7 x 1,0 мм в обоих дальних углах; проверьте настоящую плату.
Передние упоры у краёв и прежние упоры крышки ограничивают ход платы.
Проверьте реальные компоненты, пайку GPIO и провода; не вдавливайте плату.
Оставьте запас проводов: для обслуживания снимите крышку и выдвиньте плату прямо назад.
Уложите провода вне держателя LED, кармана язычка и самореза. Размеры изоляции
и посадка пучка не измерены: перед закрытием проверьте реальную проводку.
Новая посадка требует печатной примерки; расчётные зазоры номинальные.

Низ/верх платы сохранены на Z=2,8/3,5 мм. USB-проём 9,15 x 3,20 мм,
центр Z=5,25 мм, входные фаски 0,2 мм, угловые 0,15 мм.
Зазор до дна 0,8 мм включает расчётный габарит снизу 0,6 мм и свободные 0,2 мм.
Кабель ближе к столу; касание стола зависит от формы его оболочки.
Глубина корпуса 27,4 мм. Прежнее утопление USB на 0,7 мм отдельно не перемерено;
расчётный выступ не подтверждает реальную посадку кабеля.
Масштаб маскота 0,65: лицо 32,5 x 26 мм, общая высота 35,75 мм, глубина 20,5 мм.
Глаза уменьшены вместе с маскотом. Размер штифтов 3 мм и выступ 1,8 мм сохранены;
новая плоскость X=26 мм, центры Y=6/16 мм, Z=24 мм; гнёзда 3,3 мм.
Полные участки стоек крышки — 6,36 мм, пилоты 5,56 мм, закрытое дно 0,8 мм,
скосы 45 градусов от боковых стенок. Диаметры 5,6/1,9 мм дают радиальную стенку 1,85 мм.
Затягивайте три самореза крышки и один саморез держателя вручную.
Измеренная длина пластика LED — 8,5 мм ОТ ВЕРШИНЫ ДО НИЗА ВКЛЮЧАЯ фланец 1,2 мм;
корпус перед фланцем — 7,3 мм. Расчётный выступ — 3,25–3,5 мм.
Опора фланца Y=3,8 мм, задняя грань Y=5,0 мм, опора держателя Y=5,25 мм,
осевой люфт 0,25 мм. Уложите выводы вне опорных поверхностей держателя.
Красный LED проведите мимо верхней стойки самореза чуть со стороны маскота,
затем совместите с отверстием и подайте вперёд.
Уложите выводы с открытой стороны держателя, дальше от маскота. Подведите держатель
сзади на 1,3 мм ниже окончательного положения, сдвиньте вверх на 1,3 мм в карман
крыши и закрепите низ одним саморезом. Для снятия открутите саморез, сдвиньте вниз
на 1,3 мм и извлеките назад, не натягивая выводы.
При снятии держателя придерживайте LED; снятие одной задней крышки
не освобождает LED.

Проверенные размеры сохранены: отверстия LED 5,1 мм, пилоты 1,9 мм, гнёзда руки 3,3 мм.
Испытанные пробники LED/саморезов и соединения руки больше не включены.
Штифты руки позиционируют детали; склеивайте постоянное соединение после сухой примерки.
Фаски язычка глаза: углы 0,325 мм, входная кромка 0,1625 мм.
Печатайте два глаза лицом вниз, монтажными язычками вверх.
Перед печатью просмотрите стенки и заполнение крышки USB-пробника; текущие проверки — в отчёте слайсера.
Полный комплект содержит оба руководства, STEP, исходники и отчёты.
Приложение Mac и готовая прошивка скачиваются отдельно из релиза.
'''
    packs = {
        'clawlight-compact-STL.zip': [(f, f.relative_to(MODEL/'stl').as_posix()) for f in files],
        'clawlight-USB-fit-test.zip': [(f, f.relative_to(MODEL/'stl').as_posix()) for f in files
                                     if f.parent.name == 'coupons'],
    }
    full = [(ROOT/name, name) for name in REFERENCE_FILES]
    if (ROOT/'BUILD.json').is_file():
        full.append((ROOT/'BUILD.json', 'BUILD.json'))
    for f in sorted(MODEL.rglob('*')):
        if not f.is_file() or any(x.startswith('.') or x == '__pycache__' for x in f.relative_to(MODEL).parts):
            continue
        if f.suffix not in ('.py', '.step', '.stl', '.md', '.txt', '.png', '.svg') and \
                f.name not in ('parts.json', 'validation.json', 'slicer-validation.json'):
            continue
        if (f.parent.name == 'preview' and f.name not in ('dimensions.png', 'dimensions.svg')
                and f.relative_to(MODEL).as_posix() not in guide):
            continue
        full.append((f, f.relative_to(ROOT).as_posix()))
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
        path = output/name
        if name == 'clawlight-USB-fit-test.zip':
            text = '''# USB fit test — enclosure release candidate

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
The new fixed-guide fit has not been physically validated.
Guides: 0.10 mm side clearance each, slot 0.80 mm for a 0.70 mm PCB,
0.60 mm lower bearing, 0.30 mm upper overlap, 0.25 mm slot-mouth entry chamfers
and 0.10 mm edge bevels; wall/floor roots remain continuous to the cover seam.
Board bottom/top remain Z=2.8/3.5 mm. Guides meet the 2 mm floor and end at the cover seam.
New 45-degree roof braces rise 1.3 mm at the side walls. Each far-end corner lip
covers 2.5 mm across the PCB and reaches 0.60 mm over its end, with 0.15 mm entries;
the middle 13 mm stays open. Check the assumed bare 2.7 x 1.0 mm patches at both corners.
Use the new base coupon; its cover is unchanged.
The nominal underside envelope clears the floor by 0.2 mm; check actual solder.
The cable is closer to the table, but contact depends on its molding.
USB opening stays 9.15 x 3.20 mm at Z=5.25 mm, entry chamfers 0.2 mm, corners 0.15 mm.
Depth stays 27.4 mm; an earlier 0.7 mm USB recess has not been remeasured.
Check screws in the retained short bosses (6.36 mm section, 5.56 mm pilot,
0.8 mm closed end, diameters 5.6/1.9 mm, 45-degree side-wall ramps).
Inspect cover-coupon walls and infill in the slicer before printing.
A successful coupon test does not replace final wiring and assembly checks.
'''
            text_ru = '''<a name="usb-fit-test-enclosure-release-candidate"></a>

# Примерка USB — релиз-кандидат корпуса

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
Посадка новых неподвижных направляющих ещё не проверена физически.
Направляющие: боковой зазор по 0,10 мм, паз 0,80 мм для платы 0,70 мм,
нижняя опора 0,60 мм, верхнее перекрытие 0,30 мм, входные фаски 0,25 мм только
на устьях пазов, продольные 0,10 мм; основания у стенок и дна непрерывны до стыка крышки.
Низ/верх платы сохранены на Z=2,8/3,5 мм; направляющие соединены с дном 2 мм и заканчиваются у стыка крышки.
Новые верхние укосы 45 градусов поднимаются на 1,3 мм у боковых стенок. Каждый дальний
уголок занимает 2,5 мм по ширине и заходит над торцом платы на 0,60 мм, с фасками 0,15 мм;
по центру свободны 13 мм. Проверьте предполагаемые свободные участки 2,7 x 1,0 мм
в обоих дальних углах. Используйте новое основание пробника; его крышка не изменена.
Расчётный габарит снизу имеет зазор до дна 0,2 мм; проверьте реальную пайку.
Кабель ближе к столу, но касание зависит от формы оболочки.
USB-проём 9,15 x 3,20 мм на Z=5,25 мм, входные фаски 0,2 мм, угловые 0,15 мм.
Глубина 27,4 мм; прежнее утопление USB на 0,7 мм отдельно не перемерено.
Проверьте саморезы в сохранённых коротких стойках: участок 6,36 мм, пилот 5,56 мм,
дно 0,8 мм, диаметры 5,6/1,9 мм, скосы от стенок 45 градусов.
Перед печатью просмотрите стенки и заполнение крышки в слайсере.
Удачная примерка пробника не заменяет проверку проводки и окончательной сборки.
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
            (output/name).unlink(missing_ok=True)


if __name__ == '__main__':
    main()
