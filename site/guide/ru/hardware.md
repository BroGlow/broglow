---
title: Техническое руководство
---

<a name="broglow-setup-and-operation"></a>

# BroGlow: настройка и работа

BroGlow передаёт статусы локальных агентов лампам ESP32-C3 SuperMini по USB или,
на macOS, по Bluetooth LE. Один Mac управляет несколькими лампами; нужно питание
USB, без Wi-Fi. Команды исходников выполняйте из корня репозитория, команды пакета —
из распакованного каталога. Необязательный корпус — отдельный комплект для печати,
релиз **1.0.0**. Перед заявлениями о поддержке оборудования смотрите
[ограничения проверок](#repeatable-checks-and-results).

<a name="choose-a-setup-path"></a>

## Выберите способ настройки

| Задача | С чего начать |
| --- | --- |
| Поручить настройку Codex или другому агенту | [Инструкция для агента](#agent-runbook) |
| Установить без компиляции Rust | [Установка готового пакета](#install-a-prebuilt-package) |
| Собрать на своём Mac | [Сборка из исходников](#build-from-source) |
| Подключить уже прошитую лампу | [Настройка с помощью USB](#usb-assisted-setup) |
| Найти значок или проверить службу | [Значок в строке меню и проверка службы](#menu-bar-icon-and-service-validation) |
| Показывать статусы локальных задач настольного приложения Codex | [Подключение локальных сеансов Codex](#connect-local-codex-sessions) |
| Записать готовую прошивку на новую C3 | [Запись готовой прошивки C3](#flash-prebuilt-c3-firmware) |
| Проверить подключение LED отдельно от Bluetooth и управления сеансами | [Отдельная прошивка для проверки LED](#standalone-led-test-firmware) |

<a name="agent-runbook"></a>

### Инструкция для агента

Откройте пакет или исходники в агенте и попросите:

> Установи BroGlow по site/guide/ru/hardware.md. Сохрани мои настройки и посторонние
> хуки, оставь одну службу в строке меню и проверь её состояние. Подключи мою уже
> прошитую лампу C3, если она подключена; спроси перед прошивкой неизвестной платы.

1. Проверьте архитектуру Mac. `BUILD.json` + `bin/broglow` — готовый пакет;
   `Cargo.toml` + `src/` — исходники. Выберите соответствующий способ установки.
2. Проверьте службу и [сделайте снимок](#install-or-update-the-mac-service) перед
   изменениями. Для подключения используйте уже работающую установку.
3. Определите плату и подключите одну лампу по USB: на macOS настройка начнётся автоматически.
   Для повтора используйте **Lamps → Add lamp** или `broglow ble pair`.
   Рабочая прошивка не требует повторной записи или сброса сопряжений.
4. Разрешение Bluetooth и [доверие хукам Codex](#connect-local-codex-sessions)
   оставьте пользователю. Сохраните порядок и настройки посторонних хуков;
   не подделывайте хеши доверия и не перезаписывайте конфигурацию Codex.
5. Проверьте одну подписанную службу из пакета приложения, CLI в PATH и свежее
   подтверждение лампы. Укажите недостающее согласие, плату или визуальную проверку.
   Одной сборки или сохранённого ID недостаточно.

Для разработки также читайте `AGENTS.ru.md` и `CLAUDE.ru.md` в исходниках.

<a name="wiring-and-resistor-choice"></a>

## Подключение и выбор резисторов

Перед подключением проводов отключите USB. Для каждого LED нужен свой резистор:
**330 Ω для красного/жёлтого, 220 Ω для зелёного**, мощностью 1/8 Вт или 1/4 Вт.

<!-- --8<-- [start:lamp-wiring] -->
```text
GPIO0 ── 330 Ω ── анод красного LED (+)  катод (−) ── GND
GPIO1 ── 330 Ω ── анод жёлтого LED (+)   катод (−) ── GND
GPIO3 ── 220 Ω ── анод зелёного LED (+)  катод (−) ── GND
```
<!-- --8<-- [end:lamp-wiring] -->

Катод обычно имеет короткую ножку и плоский край; обрезанные LED проверьте
мультиметром в режиме диода. Катоды соединяются с общей GND, **не с общим резистором**.
Плата питается от USB-C; LED — от **3,3 В GPIO**, не контакта 5 В. Используйте
отдельные индикаторные LED, не RGB-светодиод или адресную ленту.

| LED | Контакт платы | Команда | Значение |
| --- | --- | --- | --- |
| Красный / верхний | `0` / GPIO0 | `R` | Требуется ваше действие |
| Жёлтый / средний | `1` / GPIO1 | `Y` | Ожидание |
| Зелёный / нижний | `3` / GPIO3 | `G` | Работа |
| Нет цвета сеанса | — | `N` | Выключены при свежем статусе, кроме запуска, подтверждения соединения, предпросмотра яркости и Identify |

**0, 1, 3 и G (GND) находятся с одной стороны**; это маркировка, не порядковые
позиции. Контакт 2 не используйте. GPIO0/1/3 поддерживают ШИМ и не затрагивают
контакты загрузки 2/8/9, USB 18/19, BOOT 9 и синий LED 8. Внешний кварц 32,768 кГц
на GPIO0/1 здесь не используется. [Назначение контактов Espressif](https://documentation.espressif.com/esp32-c3_datasheet_en.html),
[справочник GPIO](https://docs.espressif.com/projects/esp-idf/en/v5.4/esp32c3/api-reference/peripherals/gpio.html).

`I = (3.3 V − Vf) / R`: при `Vf = 2.0 V` резистор 330 Ω даёт **3,9 мА / 5 мВт**;
470 Ω — 2,8 мА, 220 Ω — 5,9 мА. Яркость зависит от прямого напряжения LED.
Для других LED начните с 330 Ω на каждом и подберите номиналы в пределах
допустимого тока. Никогда не убирайте резистор.

<a name="build-and-identify-the-board"></a>

## Сборка и определение платы

Установите [uv](https://docs.astral.sh/uv/getting-started/installation/):
`pyproject.toml` и `uv.lock` предоставляют PlatformIO Core **6.2.0**, esptool
**5.4.0** и pyserial. Rust остаётся в `~/.cargo/bin/`. Сохраняйте
`espressif32 @ 7.1.3` / Arduino-ESP32 **2.0.17**, включая библиотеку BLE.
Приложение закрепляет **btleplug 0.13.1** для передачи ошибок записи CoreBluetooth;
одобрение сопряжения macOS всё равно обязательно.

```bash
# Начните в каталоге исходников BroGlow этого проекта.
export PATH="$HOME/.cargo/bin:$PATH"

# Только сборка; ни одна команда не обращается к плате.
uv run flash --build-only
cargo build --locked

# Список последовательных USB-устройств; он не определяет точную модель платы.
uv run platformio device list
```

Перед записью или командами последовательного порта осмотрите **ESP32-C3 SuperMini**
и сравните порты до/после подключения кабелем передачи данных. При необходимости
отключите похожие платы. VID Espressif `0x303A` общий для разных чипов: не выбирайте
по VID, имени из примера или порядку списка. Если плата не найдена, остановитесь.
Замените заглушку фактическим путём:

```bash
BOARD_PORT='/dev/cu.REPLACE_WITH_IDENTIFIED_BOARD'
```

Профиль `esp32-c3-devkitm-1` задаёт C3, **4 МБ флеш-памяти**, DIO / 40 МГц
и встроенный USB, а не разводку SuperMini. Определив плату, проверьте чип и память:

```bash
~/.platformio/penv/bin/python ~/.platformio/packages/tool-esptoolpy/esptool.py \
  --chip esp32c3 --port "$BOARD_PORT" chip_id
~/.platformio/penv/bin/python ~/.platformio/packages/tool-esptoolpy/esptool.py \
  --chip esp32c3 --port "$BOARD_PORT" flash_id
```

Чтение перезапускает плату в загрузчик. Требуются **ESP32-C3 / 4 МБ**;
расхождения устраните до записи. [Профиль PlatformIO](https://docs.platformio.org/en/latest/boards/espressif32/esp32-c3-devkitm-1.html).

<a name="flash-and-recover-usb-upload"></a>

## Запись прошивки и восстановление USB-загрузки

Штатный путь одинаков для исходников и распакованного пакета Mac:

```bash
uv run flash
uv run flash --backup
uv run flash --skip-info
uv run flash --port PORT
```

Команда выбирает только одну однозначную последовательную USB-плату, проверяет
ESP32-C3 и ровно 4 МБ памяти, спрашивает подтверждение перед записью и после
перезапуска требует точный ответ BroGlow на `INFO`. `--skip-info` пропускает только
проверку старой прошивки. Копии создаются по запросу: `--backup` читает все 4 МБ
в приватный файл `.recovery/`, печатает SHA-256 и предупреждает о возможных ключах
Bluetooth. По умолчанию копии нет. Для `--yes` также обязателен явный `--port`.
Команда приостанавливает установленное управление лампой до первого обращения
к последовательному порту и ждёт три секунды остановки обработчика. Ранее
включённое управление на выбранном транспорте восстанавливается после проверки,
отмены или ошибки; уже отключённая лампа остаётся отключённой. Установка по
документированному пути `~/.cargo/bin/broglow` находится и без настройки PATH.

В исходниках команда собирает и записывает через PlatformIO. В пакете она проверяет
`flash-manifest.json` и четыре готовых образа перед вызовом esptool. Полное стирание
не выполняется, а `firmware.bin` никогда не записывается по нулевому адресу.

Для расширенной ручной записи из исходников сначала определите плату. Если BroGlow
установлен, отключите управление:

```bash
broglow ble off
```

Подождите три секунды, закройте `broglow led` и мониторы порта, затем запишите:

```bash
~/.platformio/penv/bin/pio run -d firmware/esp32-c3-supermini \
  -t upload --upload-port "$BOARD_PORT"
```

Если соединение не удаётся: удерживайте **BOOT**, нажмите/отпустите **RESET**,
отпустите **BOOT**; обновите список портов и `BOARD_PORT`. Повторите запись;
нажмите RESET, если приложение не запустилось. Проверьте кабель передачи данных.
[Инструкция Espressif](https://docs.espressif.com/projects/esp-idf/en/stable/esp32c3/get-started/establish-serial-connection.html).

Результаты — в `firmware/esp32-c3-supermini/.pio/build/esp32-c3-supermini/`.
PlatformIO записывает загрузчик, таблицу разделов и приложение по штатным адресам.
**Не записывайте образ приложения `firmware.bin` по нулевому адресу.** Обновляйте
этой USB-загрузкой или esptool ниже; обновления через прикладной serial/BLE нет.

После изменения `LED_RED_PIN`, `LED_YELLOW_PIN`, `LED_GREEN_PIN` или
`STATUS_TIMEOUT_MS` в [platformio.ini](../../../firmware/esp32-c3-supermini/platformio.ini)
пересоберите прошивку. Параметры встроенного LED — в [индикации](#pairing-feedback-and-diagnostics-without-external-leds).

<a name="flash-prebuilt-c3-firmware"></a>

### Запись готовой прошивки C3

Пропустите для рабочей прошитой лампы. Проверенный пакет содержит четыре образа
и `firmware/esp32-c3-supermini/flash-manifest.json`; компилятор не нужен.
После проверки контрольных сумм выполните `uv run flash` из корня пакета.
Ручная процедура esptool ниже нужна для расширенного восстановления. Сначала
[определите плату](#build-and-identify-the-board). Если esptool отсутствует, используйте Python 3
([установщик macOS](https://www.python.org/downloads/macos/)):

```bash
python3 -m venv "$HOME/.local/share/broglow-flash-tools"
ESPTOOL_PY="$HOME/.local/share/broglow-flash-tools/bin/python"
"$ESPTOOL_PY" -m pip install 'esptool==5.4.0'
"$ESPTOOL_PY" -m serial.tools.list_ports -v
# Задайте BOARD_PORT по физически определённой плате, не по примеру из инструкции.
"$ESPTOOL_PY" -m esptool --chip esp32c3 --port "$BOARD_PORT" chip-id
"$ESPTOOL_PY" -m esptool --chip esp32c3 --port "$BOARD_PORT" flash-id
```

Требуются **ESP32-C3 / 4 МБ** и проверенный `PACKAGE_DIR`. Отключите управление
лампой, подождите три секунды и закройте клиентов порта. Для необязательной
полной копии вручную:

```bash
RECOVERY_ROOT="$PACKAGE_DIR/.recovery"
mkdir -p -m 700 "$RECOVERY_ROOT"
chmod 700 "$RECOVERY_ROOT"
FLASH_BACKUP="$RECOVERY_ROOT/c3-before-flash-$(date +%Y%m%d-%H%M%S).bin"
(umask 077; "$ESPTOOL_PY" -m esptool --chip esp32c3 --port "$BOARD_PORT" \
  read-flash 0x0 0x400000 "$FLASH_BACKUP")
shasum -a 256 "$FLASH_BACKUP"
```

Копия может содержать ключи Bluetooth: храните её приватно. Сверьте адреса с манифестом
и запишите образы закреплённым [esptool 5.4.0](https://pypi.org/project/esptool/5.4.0/):

```bash
FIRMWARE_DIR="$PACKAGE_DIR/firmware/esp32-c3-supermini"
cat "$FIRMWARE_DIR/flash-manifest.json"
"$ESPTOOL_PY" -m esptool --chip esp32c3 --port "$BOARD_PORT" \
  --baud 460800 write-flash --flash-mode dio --flash-freq 40m --flash-size 4MB \
  0x0 "$FIRMWARE_DIR/bootloader.bin" \
  0x8000 "$FIRMWARE_DIR/partitions.bin" \
  0xe000 "$FIRMWARE_DIR/boot_app0.bin" \
  0x10000 "$FIRMWARE_DIR/firmware.bin"
```

Проверьте успешную запись/хеш и запуск приложения, затем [сопрягите через USB](#usb-assisted-setup).
Запись сохраняет NVS `0x9000..0xe000` только при совпадающей разметке; для иной
прошивки нужна полная копия. Не используйте `erase_flash` при обычном обновлении
и не записывайте образ приложения в ноль. При необходимости используйте BOOT/RESET выше.

<a name="full-flash-recovery-image"></a>

### Образ полной флеш-памяти для восстановления

Храните полный дамп приватно вместе с хешем и идентификатором платы: в нём есть
прошивка и ключи Bluetooth. Остановите клиентов порта и восстанавливайте только
на ту же плату; это заменит прошивку и сопряжения. Полный дамп **4 МБ** начинается
с нуля, в отличие от образа приложения `firmware.bin`. Не распространяйте чужую копию.

<a name="standalone-led-test-firmware"></a>

## Отдельная прошивка для проверки LED

Соберите `led-test`, чтобы проверить провода отдельно от Bluetooth, сеансов и ШИМ.
Он циклически включает **красный GPIO0, жёлтый GPIO1, зелёный GPIO3** на полном
цифровом уровне по секунде; синий встроенный LED вспыхивает на 100 мс при смене.
Каждому LED нужен свой резистор и катод к общей GND. Пакет содержит исходники
теста, но не готовый тестовый образ.

[Заново определите плату/порт](#build-and-identify-the-board), проверьте **C3 / 4 МБ**,
закройте мониторы порта и выполните из исходников:

```bash
broglow ble off
# Подождите, пока работающий обработчик лампы освободит устройство.
sleep 3
~/.platformio/penv/bin/pio run -d firmware/esp32-c3-supermini -e led-test
~/.platformio/penv/bin/pio run -d firmware/esp32-c3-supermini -e led-test \
  -t upload --upload-port "$BOARD_PORT"
~/.platformio/penv/bin/pio device monitor --port "$BOARD_PORT" --baud 115200
```

Дождитесь проверенной записи. Сравните `led-test color=red gpio=0 duration_ms=1000`
и `pin_levels red=1 yellow=0 green=0` со свечением; далее yellow/1 и green/3.
Высоким должен быть только названный контакт. Логический уровень не измеряет
напряжение, ток или исправность LED. Если света нет, отключите питание и проверьте
полярность, резистор, контакты и GND; тест сам по себе не определяет неисправную деталь.

Тест **не поддерживает BLE, `INFO`, сопряжение, `?`, serial-команды цвета и плавный
старт основной прошивки**. Ctrl-C закрывает монитор, не цикл; отключение питания
останавливает, сброс запускает снова. Оставьте запрошенную тестовую плату в этом
режиме до запроса обычной работы. Только тогда закройте монитор, заново определите
порт и верните основную прошивку:

```bash
~/.platformio/penv/bin/pio run -d firmware/esp32-c3-supermini \
  -e esp32-c3-supermini -t upload --upload-port "$BOARD_PORT"
```

Для новой/сменной платы затем выполните [USB-сопряжение](#usb-assisted-setup);
сохраняйте прежние ID ламп, если не попросили забыть. Образы находятся в
`.pio/build/led-test/` и `.pio/build/esp32-c3-supermini/` каталога прошивки.

<a name="check-the-wiring-before-connecting-broglow"></a>

## Проверка подключения перед соединением с BroGlow

Нужны **основная прошивка**, определённый `BOARD_PORT`, отключённое управление
лампой и закрытые мониторы порта. Отключите BLE: подключённый клиент управляет LED.
Используйте Python PlatformIO или [окружение для готовой прошивки](#flash-prebuilt-c3-firmware):

```bash
# Установка из исходников:
SERIAL_PY="$HOME/.platformio/penv/bin/python"
# Для готового пакета вместо этого: SERIAL_PY="$HOME/.local/share/broglow-flash-tools/bin/python"
"$SERIAL_PY" - "$BOARD_PORT" <<'PY'
import sys
import time
import re
import serial

with serial.Serial(sys.argv[1], 115200, timeout=0.5, write_timeout=2) as port:
    port.rts = False
    port.dtr = True
    time.sleep(0.3)
    port.reset_input_buffer()

    def request(command, prefix):
        port.write(command)
        port.flush()
        until = time.monotonic() + 3
        while time.monotonic() < until:
            reply = port.readline().decode(errors="replace").strip()
            if reply.startswith(prefix):
                return reply
        raise RuntimeError("Нет ответа прошивки; перепроверьте определённый порт")

    identity = request(b"INFO\n", "broglow_pair ")
    if not re.fullmatch(r"broglow_pair identity=[0-9a-f]{12} service=7d2ea28a-f7bd-485a-bd9d-92ad6ecfe93e", identity):
        raise RuntimeError("Неожиданный идентификатор прошивки; остановитесь и определите плату")
    print(identity, flush=True)
    diagnostic = request(b"?\n", "state=")
    if " connected=0 " not in f" {diagnostic} ":
        raise RuntimeError("Лампой ещё управляет BLE; отключите его перед тестом")
    try:
        time.sleep(3)  # Дождитесь завершения плавной стартовой индикации.
        for _ in range(10):
            for command, label in [(b"R\n", "красный"), (b"Y\n", "жёлтый"), (b"G\n", "зелёный")]:
                print(label, flush=True)
                port.write(command)
                port.flush()
                time.sleep(1)
    finally:
        port.write(b"N\n")
        port.flush()
        print("все выключены", flush=True)
PY
```

Ожидаются красный, жёлтый и зелёный по отдельности на секунду, десять циклов,
затем `N`/выключение; прерывание тоже пытается отправить `N`. После трёхсекундного
плавного запуска свежие serial-команды заменяют анимацию ожидания/сопряжения.
Для тёмных/тусклых LED отключите питание и проверьте полярность, GPIO, резистор
и GND. Если темны все, проверьте также прошивку, порт, яркость и отключение BLE.
Перезапуск питания при отключённом BLE возвращает 100% яркости.

Serial принимает полные строки `R\n`, `Y\n`, `G\n`, `N\n` на **115200 бод**, включая
CRLF; пустые, неизвестные, многосимвольные и слишком длинные строки игнорируются.
Для проверки тайм-аута отправьте один `R\n`, оставьте порт открытым без записи:
через **десять секунд** состояние должно стать `N`, затем вернётся индикация
сопряжения/ожидания отключённой платы. Поля `state`, `age_ms`, `mode` отличают её
от устаревшего цвета сеанса.

<a name="local-recovery-files"></a>

## Локальные файлы восстановления

Храните снимки и приватные аппаратные результаты в `.recovery/` в корне исходников
или пакета с правами **0700**; Git и публичные пакеты исключают каталог. Дампы
могут содержать ключи Bluetooth. Один снимок перед изменением установки используйте
для повторов и храните до проверки. Сборка/документация снимка не требуют.
Сохраняйте неизвестную прошивку, если владелец явно не разрешил потерю данных платы;
файлы восстановления не нужны для сборки, установки и запуска релиза.

<a name="install-or-update-the-mac-service"></a>

## Установка или обновление службы на Mac

Выполняйте от вошедшего пользователя Mac, **без sudo**. Сначала проверьте установку;
отсутствие команды/службы на новом Mac нормально:

```bash
command -v broglow
launchctl print "gui/$(id -u)/io.github.broglow.menubar"
pgrep -fl 'broglow.*menubar'
```

Перед изменениями сохраните настройки/хуки, CLI и службу из корня исходников/пакета.
Храните `.recovery/` при удалении пакета, пока нужен откат:

```bash
RECOVERY_ROOT="$PWD/.recovery"
mkdir -p -m 700 "$RECOVERY_ROOT"
chmod 700 "$RECOVERY_ROOT"
BACKUP_DIR="$RECOVERY_ROOT/install-$(date +%Y%m%d-%H%M%S)"
mkdir -m 700 "$BACKUP_DIR"
for item in \
  "$HOME/.claude/settings.json" \
  "$HOME/.claude/broglow/config.json" \
  "$HOME/.cargo/bin/broglow" \
  "${CODEX_HOME:-$HOME/.codex}/hooks.json" \
  "$HOME/.config/opencode/plugins/broglow.js" \
  "${COPILOT_HOME:-$HOME/.copilot}/hooks/broglow.json" \
  "$HOME/Library/LaunchAgents/io.github.broglow.menubar.plist"
do
  if [ -f "$item" ]; then
    mkdir -p "$BACKUP_DIR$(dirname "$item")"
    cp -p "$item" "$BACKUP_DIR$item"
  fi
done
if [ -d "$HOME/Applications/BroGlow.app" ]; then
  cp -R "$HOME/Applications/BroGlow.app" "$BACKUP_DIR/BroGlow.app"
fi
```

Оба способа используют `~/.cargo/bin/broglow`, чтобы хуки работали после удаления
пакета; для готовой сборки Cargo не нужен. `broglow install` обновляет приложение,
хуки и существующую службу входа, сохраняя посторонние записи. Сохраните порядок
и одобрение хуков Codex, используйте [проверку пользователем](#connect-local-codex-sessions)
и не перезаписывайте `config.toml`. Перезапустите сеансы opencode для нового плагина.
Если PATH находит другую копию, используйте `~/.cargo/bin/broglow install` явно.

Служба `io.github.broglow.menubar`, зарегистрированная в
`~/Library/LaunchAgents/io.github.broglow.menubar.plist`, запускает:

```text
~/Applications/BroGlow.app/Contents/MacOS/broglow menubar
```

Обновляйте эту службу; не запускайте рядом `cargo run -- menubar`. Блокировки процесса
и BLE предотвращают дубликаты строки меню и одновременный поиск/управление лампой.
Из копий восстанавливайте только затронутые настройки BroGlow, сохраняя посторонние.

<a name="install-a-prebuilt-package"></a>

### Установка готового пакета

Скачайте полный Mac-пакет проекта и `.tar.gz.sha256`: `aarch64-apple-darwin`
для Apple Silicon, `x86_64-apple-darwin` для Intel. Архив приложения в трее и CLI для Windows — не пакет Mac/C3.
Для установки приложения не нужны Rust, Cargo, Node.js или PlatformIO.

```bash
# Замените фактическим путём к полученному архиву.
PACKAGE_ARCHIVE='/path/to/broglow-vVERSION-aarch64-apple-darwin.tar.gz'
cd "$(dirname "$PACKAGE_ARCHIVE")"
shasum -a 256 -c "$(basename "$PACKAGE_ARCHIVE").sha256"
mkdir -p "$HOME/Downloads"
tar -xzf "$PACKAGE_ARCHIVE" -C "$HOME/Downloads"
PACKAGE_DIR="$HOME/Downloads/$(basename "$PACKAGE_ARCHIVE" .tar.gz)"
cd "$PACKAGE_DIR"
shasum -a 256 -c SHA256SUMS
uname -m
file bin/broglow
cat BUILD.json
./bin/broglow --version
./bin/broglow ble pair --help
```

Нужны совпадающие хеши/архитектура, ожидаемая версия и поддержка `ble pair`.
`BUILD.json` содержит минимальную macOS и ревизию исходников. Хеши проверяют
целостность, не издателя: получайте пакет из доверенного источника. Локальная
сборка не нотариально заверена Developer ID. Если заблокирована, проверьте источник
и используйте для этого приложения [Open Anyway](https://support.apple.com/en-us/102445);
не отключайте Gatekeeper глобально. После [снимка](#install-or-update-the-mac-service) установите:

```bash
mkdir -p "$HOME/.cargo/bin"
install -m 755 "$PACKAGE_DIR/bin/broglow" "$HOME/.cargo/bin/broglow"
cmp "$PACKAGE_DIR/bin/broglow" "$HOME/.cargo/bin/broglow"
"$HOME/.cargo/bin/broglow" install
export PATH="$HOME/.cargo/bin:$PATH"
```

Установщик локально подписывает постоянный пакет приложения. Завершите
[настройкой PATH](#run-broglow-from-any-directory-path), [проверкой службы](#menu-bar-icon-and-service-validation)
и [USB-сопряжением](#usb-assisted-setup). Включённые [образы C3](#flash-prebuilt-c3-firmware)
нужны только новым/непрошитым платам.

<a name="build-from-source"></a>

### Сборка из исходников

Нужна полная копия исходников с `Cargo.lock`, `src/ble.rs` и
`firmware/esp32-c3-supermini/platformio.ini`; архив приложения ею не является.
Сохраняйте локальные изменения. Определяйте сборку по ревизии/`BUILD.json`,
не только номеру версии.

Установите только недостающие инструменты:

- Инструменты Apple: `xcode-select -p`; при отсутствии `xcode-select --install`.
- [Rust stable](https://doc.rust-lang.org/book/ch01-01-installation.html);
  проверки добавляются через `rustup component add rustfmt clippy`.
- [PlatformIO Core](https://docs.platformio.org/en/latest/core/installation/methods/installer-script.html)
  для прошивки (обычно `~/.platformio/penv/bin/pio`).
- [Node.js](https://nodejs.org/en/download) для JS-тестов;
  [Python 3](https://www.python.org/downloads/macos/) для проверок прошивки/упаковки.

Из корня исходников соберите и проверьте без записи на плату:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
node --test tests/js/*.test.mjs
cargo build --release --locked

# Прошивка лампы: только сборка и проверки, без записи на плату.
~/.platformio/penv/bin/python -m pip install clang-format==23.1.1
~/.platformio/penv/bin/clang-format --dry-run --Werror \
  firmware/esp32-c3-supermini/src/*.cpp firmware/esp32-c3-supermini/tests/*.cpp
python3 firmware/esp32-c3-supermini/tests/check.py
~/.platformio/penv/bin/pio run -d firmware/esp32-c3-supermini
```

Первая сборка скачивает закреплённые зависимости; `--offline` подходит только
при полном кеше Cargo. После [снимка](#install-or-update-the-mac-service) установите:

```bash
cargo install --path . --locked
"$HOME/.cargo/bin/broglow" install
```

После изменения кода приложения выполняйте обе команды: компиляция оставляет
старую службу запущенной. Прошивке нужна явная запись на плату. Завершите
[проверкой службы](#menu-bar-icon-and-service-validation), [PATH](#run-broglow-from-any-directory-path)
и, если запрошено, [настройкой лампы](#usb-assisted-setup).

<a name="run-broglow-from-any-directory-path"></a>

### Запуск broglow из любого каталога (PATH)

При обоих способах CLI пользователя находится в `~/.cargo/bin/broglow`.
Исходники нужны только для пересборки. Проверьте PATH вне репозитория:

<!-- --8<-- [start:path-check] -->
```bash
cd /tmp
command -v broglow
type -a broglow
broglow --help
```
<!-- --8<-- [end:path-check] -->

Первым должен быть ваш `~/.cargo/bin/broglow`. Если отсутствует или перекрыт
другой копией, добавьте **один раз** в `~/.zshrc`, откройте новый терминал и повторите:

<!-- --8<-- [start:path-setup] -->
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```
<!-- --8<-- [end:path-setup] -->

Если `~/.cargo/env` Rust уже настроил PATH, менять ничего не нужно. Сохраняйте
настройки оболочки; другие оболочки используют свои файлы запуска. Разовый export
не влияет на будущие терминалы. `broglow install` регистрирует хуки/службу,
не PATH. Скрипты/задания могут пропускать настройки оболочки: используйте
`"$HOME/.cargo/bin/broglow"`. LaunchAgent уже использует абсолютный путь пакета.

<a name="make-a-local-prebuilt-package"></a>

### Создание локального готового пакета

После проверок и пересборки изменённых исходников приложения/прошивки:

```bash
python3 scripts/package-macos.py
```

Команда упаковывает готовые результаты в игнорируемый `dist/`, проверяет адреса
образов и границы разделов, записывает хеши/происхождение. Передавайте `.tar.gz`
вместе с `.sha256`. Внутри CLI, четыре образа C3/манифест, инструкция, указатели
для агентов и метаданные. Команда не собирает, не устанавливает, не прошивает
и не публикует. Релизы по тегам — в `site/guide/ru/releasing.md` исходников.

<a name="menu-bar-icon-and-service-validation"></a>

## Значок в строке меню и проверка службы

После установки нажмите **⌘ Space**, найдите **BroGlow** и нажмите Return.
Значок приложения со светофором открывает управление сеансами, в том числе если
BroGlow уже запущен. Также можно открыть `~/Applications/BroGlow.app` в Finder
или выполнить:

```bash
open "$HOME/Applications/BroGlow.app"
```

BroGlow запускается при входе и остаётся в строке меню. Команда `broglow`
в Terminal открывает необязательную терминальную панель TUI.

Найдите **значок светофора справа в строке меню macOS** (подсказка `broglow`).
Значка Dock/окна не ожидается. Левый щелчок открывает сеансы; внизу
**Lamp / Lamps 1/2 / Lamp off** открывает **Lamps** с транспортом и статусами.
Разверните строку для управления; откройте **Connection details** для ID и диагностики.

Ссылки Dashboard скрыты по умолчанию. Включите **Settings → Appearance → Show
Dashboard button**, либо:

```bash
broglow dashboard-button show
broglow dashboard-button hide
```

Это сохраняет `show_dashboard_button` и применяется сразу. При скрытых ссылках
сбой перехода к сеансу также не открывает Terminal. Прямой `broglow` открывает
TUI; **Quit** остаётся в меню правого щелчка.

Под названием каждого сеанса перед проектом показаны иконка и имя его агента
(Claude, Codex, opencode или Copilot). Второй значок показывает, где он запущен:
окно — приложение агента, `>_` — терминал, `</>` — IDE; при наведении видно точное
место, например **Claude app**, **Ghostty** или **VS Code**. Отключите **Settings → Appearance → Show
agent names**, чтобы оставить только иконку; имя появится при наведении. Это
сохраняет `show_agent_names`. TUI следует той же настройке в столбце AGENT и
показывает название или короткую метку (`cc` Claude, `cx` Codex, `oc` opencode,
`co` Copilot), а место запуска называет в столбце HOST (**app** — приложение агента).
Меню трея Linux показывает метку вместо иконки.

| Значок | Состояние сеансов |
| --- | --- |
| ![Серый](../../../assets/icons/broglow-none.png) Серый | Нет активных сеансов |
| ![Зелёный](../../../assets/icons/broglow-green.png) Зелёный | Работа с учётом Yellow mode |
| ![Жёлтый](../../../assets/icons/broglow-yellow.png) Жёлтый | Ожидание с учётом Yellow mode |
| ![Красный](../../../assets/icons/broglow-red.png) Красный | Требуется ввод |

Цвет — не исправность службы/Bluetooth; текст использования по умолчанию выключен.
Если значок скрыт, покажите строку меню, выйдите из полноэкранного режима или
раскройте менеджер строки меню. [Справка Apple](https://support.apple.com/guide/mac-help/whats-in-the-menu-bar-mchlp1446/mac).

```bash
launchctl print "gui/$(id -u)/io.github.broglow.menubar"
ps -axo pid=,command= | awk '$2 ~ /(^|\/)broglow$/ && $3 == "menubar" {print}'
/usr/bin/plutil -lint "$HOME/Applications/BroGlow.app/Contents/Info.plist"
/usr/bin/codesign --verify --strict "$HOME/Applications/BroGlow.app"
"$HOME/.cargo/bin/broglow" ble status
```

Нужны launchctl `state = running` и ровно один `broglow menubar` по пути
`~/Applications/BroGlow.app/Contents/MacOS/broglow` с тем же PID.
Plist/подпись подтверждают регистрацию/целостность, не разрешение Bluetooth.
Подпись меняет исполняемый файл приложения: не сравнивайте его исходный хеш с CLI.
`ble status` показывает свежие записи: `disabled` до настройки и `connected: 1/2`
с одной удалённой лампой нормальны. Свечение LED нужно проверить визуально.

Для загруженной, но остановленной службы:

```bash
launchctl kickstart -k "gui/$(id -u)/io.github.broglow.menubar"
```

Повторите проверки; отсутствующий пакет/задание восстановите выбранной установкой.
При завершении читайте `~/.claude/broglow/menubar.log` и `menubar.err`.
Не запускайте вторую службу для проверки. Первый запуск TUI может установить хуки/автозапуск.

<a name="connect-local-codex-sessions"></a>

## Подключение локальных сеансов Codex

Codex передаёт локальные события через `$CODEX_HOME/hooks.json` (обычно
`~/.codex/hooks.json`). Установка регистрирует шесть хуков; **пользователь должен
разрешить точные определения**, прежде чем Codex запустит новые/изменённые хуки,
не управляемые политикой.

1. Установите BroGlow; повтор не нужен, если шесть записей `codex-hook` уже есть.
2. Откройте CLI Codex с пользователем и `CODEX_HOME` настольного приложения.
   На Mac разработки это `/Applications/ChatGPT.app/Contents/Resources/codex`;
   иначе используйте фактический CLI приложения/установки с той же конфигурацией.
3. В `/hooks` проверьте/разрешите BroGlow `SessionStart`, `UserPromptSubmit`,
   `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Stop`. Сохраните посторонние
   хуки. Агентам нельзя подделывать хеши, обходить доверие или перезаписывать
   `config.toml`. Изменённые определения могут потребовать новой проверки.
   Если явно отключены, пользователь может включить `hooks = true` в `[features]`,
   сохранив управляемую политику.
4. Выйдите из CLI, отправьте запрос в новой **локальной задаче приложения**
   и проверьте BroGlow. Если хуки не подхватились, завершите активную работу,
   перезапустите приложение и повторите.

Реальная задача должна стать **Working**, затем **Idle/Paused** после ответа;
проверьте **Needs you/Needs help** при обычном запросе разрешения. Другие сеансы
и Yellow mode влияют на общий цвет. `ble status` доказывает доставку лампе,
не хуки Codex; ручные тестовые события не проверяют настольную интеграцию.
Старым задачам боковой панели нужно новое событие; обычный вопрос не обязательно
является запросом разрешения. Чаты ChatGPT и облачные/удалённые задачи не питают
локальные хуки этого Mac.

Имена/переименования берутся из локального индекса Codex в течение **пяти секунд**,
пока список открыт, не меняя статус/активность. **×** (TUI **x**) убирает сеанс
только из BroGlow; новое событие вернёт его, просмотр — нет.
**Clear paused** рядом с заголовком Paused так же убирает все жёлтые сеансы,
сохраняя зелёные работающие и красные сеансы, которым нужна помощь.
Щелчок/Enter фокусирует сохранённый терминал CLI, иначе открывает
`codex://threads/<session-id>` в установленном приложении по bundle ID,
даже если оно называется ChatGPT.app.

[Доверие хукам OpenAI](https://developers.openai.com/codex/hooks#review-and-trust-hooks),
[поддерживаемые контексты](https://developers.openai.com/plugins/guides/submit-claude-plugin#review-what-openai-supports).

<a name="bluetooth-identity-and-permission"></a>

## Идентификатор Bluetooth и разрешения

Включите Bluetooth и разрешите **BroGlow** в **System Settings → Privacy & Security
→ Bluetooth**. Пакет приложения задаёт `NSBluetoothAlwaysUsageDescription` и постоянный
ID `io.github.broglow.menubar`; одного plist LaunchAgent недостаточно.
`ble scan` перезапускается из этого пакета с общей идентичностью.

Локальная ad-hoc-подпись использует designated requirement только по идентификатору:
он сохраняется между сборками, но **не удостоверяет издателя**. При распространении
постоянно задавайте реальное `BROGLOW_SIGNING_IDENTITY` со стандартным требованием
этого подписанта. Смена подписи может потребовать повторного разрешения.

Подпись или сканирование с разрешением Terminal не доказывают доступ фоновой службы.
Проверьте запись установленной службой при питании лампы только от адаптера,
затем перезапуск службы/вход и восстановление после отказа в доступе.

[Разрешения btleplug](https://github.com/deviceplug/btleplug#macos),
[описание использования Apple](https://developer.apple.com/documentation/bundleresources/information-property-list/nsbluetoothalwaysusagedescription),
[подпись/TCC](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements),
[designated requirements](https://developer.apple.com/library/archive/technotes/tn2206/).

<a name="pair-and-remember-the-home-and-office-lamps"></a>

## Сопряжение и сохранение домашней и офисной ламп

Лампы объявляют `BroGlow C3` с четырёхзначным шестнадцатеричным суффиксом платы;
Mac сохраняет UUID CoreBluetooth. Лампа принимает один Mac одновременно;
Mac может обновлять несколько ламп.

<a name="usb-assisted-setup"></a>

### Настройка с помощью USB

Подключите одну прошитую плату USB-кабелем передачи данных, закройте мониторы порта.
Настольное приложение macOS начинает настройку автоматически в течение **пяти секунд**,
в том числе если плата подключена при запуске приложения. Для ручного повтора
выберите **Lamps → Add lamp** или выполните:

```bash
broglow ble pair
broglow ble status
```

Не отключайте её во время **Adding…**, сразу одобрите сопряжение macOS. Команда
определяет прошивку, открывает сопряжение и сохраняет совпавший BLE ID;
**Connected** требует последующей зашифрованной записи обработчика. Затем питание
можно оставить от Mac или адаптера; управление остаётся BLE. При нескольких
платах/неоднозначности сначала определите нужный порт:

```bash
broglow ble pair --port "$BOARD_PORT"
```

Нужны точная идентификация `INFO`, UUID службы BroGlow и полный шестибайтовый
базовый MAC в данных производителя (`0xffff`); имени/USB VID недостаточно.
Автопоиск допускает встроенный Espressif `303A:1001`, объединяет `tty`/`cu`
и требует одного кандидата. Неоднозначность оставляет настройки/радио нетронутыми;
параллельные настройки отклоняются. Только проверенный ответ BroGlow `INFO` разрешает
`PAIR`; приложение никогда не прошивает плату автоматически.

Автонастройка делает одну попытку на подключение USB. При ошибке прежние настройки
ламп восстанавливаются; повторите через **Add lamp** либо отключите USB минимум на
**пять секунд** и подключите снова. Проверенные USB-идентификаторы сохраняются в
`usb_lamps`. Ранее добавленные лампы сохраняют выбранный транспорт и состояние
включения; забытые лампы добавляются только вручную через **Add lamp**.
Ход настройки и ошибки видны в **Lamps**. Поведение USB на Windows/Linux сохраняется;
автоматическая настройка Bluetooth доступна только на macOS.

Настройка приостанавливает управление, ждёт до **восьми секунд** освобождения
обработчика, выполняет `INFO`/`PAIR` на **115200 бод**, освобождает serial и ищет
**восемь секунд**. Занятый канал вызывает ошибку, не перехват. При неудаче прежние
настройки ламп восстанавливаются с сохранением параллельных посторонних изменений;
закрытие панели не останавливает настройку/восстановление. USB использует тот же
Just Works и требует согласия macOS.

Повторите для каждой лампы: ID добавляются, сопряжения сохраняются. Если `INFO`
не поддерживается, используйте [готовую](#flash-prebuilt-c3-firmware) или
[собранную](#flash-and-recover-usb-upload) прошивку на определённой плате в рамках запроса.

<a name="pair-without-a-usb-data-cable"></a>

### Сопряжение без USB-кабеля передачи данных

BOOT — **GPIO9, активный LOW**. Используйте после обычного запуска, предварительно
отпустив один раз. Удержание при включении/RESET запускает загрузчик ROM.

| Действие после обычной загрузки | Результат |
| --- | --- |
| Нет сохранённых сопряжений | Автоматическое окно сопряжения 60 секунд |
| Удерживать BOOT три секунды, отпустить | Окно 60 секунд; сопряжения сохраняются |
| Продолжать удержание до десяти секунд, отпустить | Запрос удаления всех сопряжений платы, отключение, окно 60 секунд |
| USB `broglow ble pair` | То же окно после идентификации; сопряжения сохраняются |
| Окно истекло | Новые сопряжения отклоняются; сохранённые Mac могут подключаться |

**LE Secure Connections Just Works** использует зашифрованные записи GATT
и постоянные сопряжения, но **без защиты от MITM**, PIN или сравнения чисел.
Подключайте нужную лампу в её физическое окно. UUID выбирает устройство;
ключи платы/macOS образуют сопряжение. [API безопасности Espressif](https://docs.espressif.com/projects/esp-idf/en/v4.4.7/esp32c3/api-reference/bluetooth/esp_gap_ble.html).

Питайте лампу от USB-адаптера. Перед поиском освободите обработчик/радио:

```bash
broglow ble off
# Подождите не менее трёх секунд, затем откройте сопряжение кнопкой BOOT, как описано выше.
broglow ble scan
```

Поиск длится **восемь секунд**. Питайте только нужную лампу или сравните поиск
при выключенной/включённой. Скопируйте точный UUID CoreBluetooth этого Mac,
заменив заглушки:

```bash
broglow ble add <HOME_ID_FROM_SCAN>
broglow ble status
broglow ble list
```

`add` включает все сохранённые лампы. Сразу одобрите Bluetooth и системное сопряжение:
наблюдаемый тайм-аут запроса — **около 30 секунд**, хотя окно платы и ожидание
записи — **60 секунд**. При истечении откройте окно заново. Первая зашифрованная
запись — `N`; после подтверждения обработчик читает/отправляет текущий общий статус.
Вспышка **250 мс** после первой записи подтверждает каждое соединение.
Объявление или низкоуровневое соединение не равны подтверждённой записи.

Для офисной лампы повторите off → ожидание три секунды → BOOT → scan → add.
Оба ID сохраняются: `Lamps 1/2` нормален при одной удалённой лампе, `BLE 2/2`
ожидается с обеими рядом. Записи/свежесть независимы: одна не блокирует и не маскирует другую.

`~/.claude/broglow/config.json` хранит `ble_peripherals` и `led_transport: "ble"`.
Используйте
**Lamps → строка → Forget lamp** или:

```bash
broglow ble remove <ID_TO_FORGET>
```

Forget работает при отсутствии/отключении, сохраняет до удаления строки и оставляет
список при ошибке записи. Переподключение прекращается, сопряжения обеих сторон
сохраняются. Удаление последнего ID отключает BLE; `ble off` сохраняет ID,
`ble usb` сохраняет их и выбирает USB. Верните лампу USB-настройкой или ID из поиска.

<a name="replace-a-mac-or-recover-mismatched-bonds"></a>

### Замена Mac или восстановление несовпадающих сопряжений

Отключите обработчик старого Mac. Установите BroGlow на новом, откройте
трёхсекундным удержанием окно платы, найдите и добавьте ID. UUID CoreBluetooth
зависят от компьютера; не копируйте ID старого Mac.

При повторных ошибках удерживайте BOOT **десять секунд после запуска**, чтобы
стереть сопряжения платы. Удалите старую запись Bluetooth в macOS через Forget/Remove,
если она есть, затем найдите/добавьте в новом окне. `ble remove` меняет только
BroGlow, не ключи macOS. Подтвердите восстановление успешной записью и полями
`authenticated`/`bonds`. Стирание асинхронно: `bond_reset=requested` не означает завершение.

<a name="pairing-feedback-and-diagnostics-without-external-leds"></a>

### Индикация сопряжения и диагностика без внешних LED

При запуске все управляемые LED дважды плавно загораются и гаснут. Во время
сопряжения три цвета загораются по очереди; несопряжённые лампы мигают; сопряжённая
отключённая лампа даёт тусклый импульс каждые 30 секунд. Первая зашифрованная запись
вызывает одну короткую подтверждающую вспышку. Затем приоритет получает свежий
статус сеанса. Встроенный LED обозначает зелёный, жёлтый и красный одним, двумя
и тремя импульсами соответственно.

`?` через определённый последовательный USB-порт сообщает состояние, коэффициенты
заполнения ШИМ, сопряжение и сохранённые ключи, а также возраст heartbeat.
Это запрошенные выходные сигналы, а не подтверждение видимого свечения.
Подробные интервалы индикации, `INFO`/`PAIR` и диагностические команды приведены
в [справочнике прошивки](firmware.md#pairing-feedback-and-diagnostics-without-external-leds).

<a name="everyday-operation-and-status"></a>

## Повседневная работа и статусы

Переключайте транспорт через TUI `l` или **Lamps → Enable/Disable lamp**.
Для включения USB нужна плата; BLE переключает все сохранённые лампы без USB.
`ble off` сохраняет ID; `ble add <SAVED_ID>` включает без дубликатов.

В **Lamps** разверните строку (щелчок или Enter/Space) для управления, **Name** и
**Forget lamp**; откройте **Connection details** для **Device ID** и **Diagnostic**.
Локальные имена — до **40 символов Unicode**; пустое
возвращает `Lamp 1`, `Lamp 2`, … . Сохраняйте при отсутствии/отключении, не меняя
объявляемое имя/сопряжения; CLI: `broglow ble rename <SAVED_ID> "Office"` (`""` очищает).
**Troubleshooting** содержит сведения службы/настройки; Back/Escape возвращает сеансы.
В TUI `d` переключает подробности, стрелки или `j`/`k` прокручивают, Escape возвращает.

**Identify** запускает только на выбранной подключённой Bluetooth-лампе **три прохода
красный → жёлтый → зелёный**: по одному светодиоду на **100%**, **100 мс на LED**
(всего **0,9 секунды**), затем возвращает последний статус и сохранённую яркость,
даже при сохранённых 0%. Он не включает лампу и не сохраняет настройки;
отсутствующие/приостановленные/устаревшие лампы недоступны. Для этой анимации обновите
плату до соответствующей прошивки: старая молча игнорирует новый режим, даже если
подтверждает запись. Обновление приложения не прошивает плату; следуйте штатной
процедуре копии/записи для определённой платы.

| Статус | Значение / действие |
| --- | --- |
| `disabled` | Включите в TUI/Lamps |
| `searching` | Проверьте сохранённые ID и питание |
| `connecting` | Соединение/поиск служб |
| `connected` | Хотя бы одна лампа недавно приняла запись; проверьте счётчик/статусы ламп |
| `error` | Прочитайте подробности разрешений, Bluetooth, тайм-аута или GATT |
| `unavailable` | Статус обработчика отсутствует/устарел; проверьте службу |

Зелёной точке нужна запись обработчика, не объявление/USB. `1/2` нормально,
если одна лампа далеко; подтверждение не доказывает свечение LED.

Красный побеждает при запросе ввода. **Yellow mode**: `any_inactive` выбирает
жёлтый при любом ожидающем сеансе; `active_wins` остаётся зелёным, пока другой
работает. Нет живых сеансов — свежий `N`/темнота после вспышки соединения.
Смена режима не требует переподключения.

Лампы получают текущий статус при соединении/изменении и каждые **две секунды**.
Переподключение не повторяет старый цвет. Отключение пытается отправить `N`,
разрывает соединение, останавливает поиск/повторы; **десятисекундный** тайм-аут
очищает недоставленный старый статус. `N` не различает отключённый и отсутствующий
Mac: отключённая сопряжённая лампа вернётся к импульсу ожидания, без гарантии темноты.

<a name="brightness-control"></a>

### Управление яркостью

В **Lamps** разверните лампу и задайте **Light brightness: 0–100%**. **All lamps**
устанавливает всем сохранённым общий уровень, очищает индивидуальные и задаёт
умолчание новым. По умолчанию 100%; прежние общие уровни сохраняются. Mac хранит
`led_brightness` и необязательные `lamp_brightness` по ID в
`~/.claude/broglow/config.json`; отсутствующие получают их после переподключения.

Ползунки показывают три цвета на выбранных подключённых лампах, возвращая статус
через **две секунды** после последнего предпросмотра. Удержание указателя продлевает
показ; клавиатура тоже показывает. Отпускание/отмена, выход из Lamps или закрытие
панели останавливают продление. Только подтверждённое изменение сохраняет;
закрытие/отмена сами по себе — нет. Предпросмотр не включает лампу и не пишет
настройки. Сохранение работает при отсутствии/отключении; Forget удаляет имя
и индивидуальную яркость лампы.

```bash
broglow ble brightness 40 --id YOUR-SAVED-UUID  # одна сохранённая лампа
broglow ble brightness 60                     # все лампы; удалить индивидуальные значения
```

CLI только сохраняет, без включения/предпросмотра. Подключённые лампы получают
изменения без переподключения. Яркость масштабирует внешние и синий встроенный LED,
включая индикацию, но не интервалы и красный LED питания. **При 0% BLE/heartbeat
работают, управляемые LED темны**; предпросмотр временно покажет другой уровень.
ШИМ — не калиброванная воспринимаемая яркость; восьмибитное округление объединяет малые уровни.

Плата хранит яркость в RAM и возвращает **100%** при питании/сбросе. Mac отправляет
сохранённое до первого статуса/вспышки соединения; более ранняя индикация использует
100%. Яркость, предпросмотр и Identify не продлевают **десятисекундный** срок статуса.
Изменение яркости не запускает анимацию заново. Отключение, потеря аутентификации
или устаревание отменяют предпросмотр и Identify.

BroGlow требует соответствующую прошивку с характеристиками статуса и яркости,
поддерживающими запись; неполная прошивка отклоняется.
USB/serial не отправляет яркость; переход с BLE сохраняет уровень RAM. Перезапустите
питание для 100%; следующее BLE-соединение восстановит настройку Mac.

<a name="preserve-or-return-to-usb"></a>

### Сохранение настроек USB или возврат к нему

```bash
broglow ble usb
```

Выбирает USB с отключённой передачей, сохраняя настройки USB и BLE ID.
Подключите определённую плату, включите через `l` или Lamps. При нескольких
Espressif задайте `led_port` её текущим найденным путём, сохраняя остальные
ключи; VID недостаточно.

Для диагностики только через USB из терминала:

```bash
broglow ble off
broglow led --port "$BOARD_PORT"
```

Остановите Ctrl-C перед включением обработчика строки меню. При подключённом BLE
serial-команды LED игнорируются; диагностика чтения остаётся доступна.

<a name="ble-contract-and-firmware-behavior"></a>

## Протокол BLE и поведение прошивки

BLE использует подтверждённые зашифрованные записи, **heartbeat две секунды**
и **тайм-аут десять секунд**. Подключённый BLE управляет LED; serial-цвета работают
при отключённом. Обновляйте прошивку через USB. UUID, сообщения, безопасность
и диагностика — в [справочнике прошивки](firmware.md#ble-contract-and-firmware-behavior).

<a name="gatt-cache-after-a-firmware-upgrade"></a>

### Кеш GATT после обновления прошивки

Прошивка запрашивает Service Changed для новых характеристик без удаления
сопряжений. Если после обновления нет яркости, переподключитесь и проверьте
возможности до сброса сопряжений. При смене закреплённого инструментария проверяйте
[реализацию SDK](firmware.md#gatt-cache-after-a-firmware-upgrade).

<a name="troubleshooting-and-service-recovery"></a>

## Поиск неисправностей и восстановление службы

| Симптом | Проверка / восстановление |
| --- | --- |
| Нет порта / запись не удаётся | Кабель данных, физическая C3/4 МБ, закрытые клиенты serial, BOOT/RESET и свежий список портов; не угадывайте. |
| `Run broglow install` / другой CLI | Используйте `~/.cargo/bin/broglow install`; проверьте PATH и пакет приложения. |
| Bluetooth занят | `broglow ble off`, подождите три секунды, закройте другие поиски, повторите. |
| Ошибка доступа / Bluetooth выключен | Включите Bluetooth, разрешите BroGlow в Privacy & Security → Bluetooth, перезапустите службу. |
| Нет ламп / постоянный поиск | Проверьте питание, прошивку, дальность, ID. Отключите клиентов (подключённые лампы могут не объявляться); сравните поиск без/с питанием. |
| Сопряжение/запись отклонены | BOOT три секунды после запуска; сопрягите за 60 секунд, сразу одобрите macOS (~30 секунд по наблюдению). Затем [восстановление сопряжений](#replace-a-mac-or-recover-mismatched-bonds). |
| BOOT убирает объявление | Отпустите BOOT, нажмите RESET, дождитесь обычного запуска перед удержанием. |
| `1/2` подключено | Нормально, если другая лампа далеко. |
| Нет характеристики / нужно обновление яркости | Проверьте идентичность/возможности; обновите определённую C3 по инструкции прошивки. |
| Подключена, но темно/не тот цвет | Проверьте яркость 0%, сеансы, Yellow mode; отключите BLE и проверьте провода. Подтверждение не доказывает электрическую исправность. |
| Тускло / один цвет тёмный | Отключите питание; проверьте полярность, GPIO, резистор, контакты, GND. [Отдельный тест](#standalone-led-test-firmware) читает логику, не напряжение/ток и не доказывает поломку платы. |
| Нет локальных сеансов Codex | [Разрешите шесть хуков](#connect-local-codex-sessions), отправьте новый запрос в локальной задаче. |
| Serial-цвета игнорируются | Отключите BLE. |
| Свет гаснет во сне Mac | Тайм-аут десять секунд; пробуждение должно переподключить/отправить статус. |
| `unavailable` / нет значка | Проверьте launchd/журналы, восстановите существующую службу, не запускайте вторую. |

Перезапуск без переустановки хуков:

```bash
launchctl kickstart -k "gui/$(id -u)/io.github.broglow.menubar"
broglow ble status
```

Читайте `~/.claude/broglow/menubar.log` и `menubar.err`. Для отсутствующего/сломавшегося
пакета или изменённого файла повторите [готовую](#install-a-prebuilt-package) или
[исходную](#build-from-source) установку через постоянный CLI. Установщик готовит
и проверяет замену; храните `~/Applications/.BroGlow.previous.*.app` до восстановления.
Восстанавливайте только затронутые настройки BroGlow, сохраняя поздние посторонние правки.

`broglow uninstall` удаляет службу, хуки BroGlow, пакет и каталог состояния;
не используйте для обычного исправления. Для намеренного удаления сначала
`broglow ble off`, затем `broglow uninstall`.

<a name="repeatable-checks-and-results"></a>

## Повторяемые проверки и результаты

Выполните из исходников:

```bash
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
node --test tests/js/*.test.mjs
python3 firmware/esp32-c3-supermini/tests/check.py
~/.platformio/penv/bin/pio run -d firmware/esp32-c3-supermini -e esp32-c3-supermini -e led-test
```

CI также проверяет C++ через clang-format **23.1.1** и сборки/тесты приложения
на Linux/macOS/Windows. Тесты прошивки имитируют GPIO, часы, serial и Bluetooth;
они не проверяют провода/радио. Проверки сайта/релиза — в `site/README.ru.md`
и `site/guide/ru/releasing.md`.

Наблюдения за электроникой и службой Mac ниже получены до ребрендинга BroGlow,
при работе с ClawLight. Повторите физическую приёмку с соответствующей прошивкой BroGlow, прежде чем
распространять на него эти результаты.

| Область | Подтверждения и ограничения |
| --- | --- |
| Apple Silicon Mac | Проверены подписанная служба, USB-сопряжение, переподключение, статус и яркость. |
| LED | Проверяйте все цвета и яркость на каждой сборке. |
| Несколько ламп | Тесты независимости проходят; одновременная работа двух физических плат не проверена. |
| Intel Mac / Windows / Linux | Цели CI/сборки, без заявления о физическом BLE или чистой установке. BLE только на macOS. |
| Распространение | Локальные проверки пакета/сайта — до публикации; загрузки, Pages и Gatekeeper чистого Mac требуют приёмки релиза. |
| Корпус | Требования к посадке и проверки сборки — в руководстве по печати; отчёты CAD/слайсера включены в комплект. |

Не включайте в инструкцию датированные журналы, ID плат и приватные пути восстановления.

<a name="physical-and-background-service-acceptance"></a>

### Приёмка оборудования и фоновой службы

Для каждой поддерживаемой комбинации Mac/платы запишите версии приложения/прошивки и:

1. Одну подписанную службу, свежий ACK, USB-настройку, согласие Bluetooth,
   переподключение без USB-данных; перезапуск/вход и восстановление отказа доступа.
2. Реальные статусы работы/ожидания/помощи/выключения, оба Yellow mode, цвета и яркость LED.
3. При сохранённом питании платы прерывание обновлений убирает цвет за **десять секунд**;
   немедленно восстановите службу.
4. Сброс платы, сон/пробуждение Mac, Bluetooth выкл./вкл., потерю дальности,
   отключение/включение; отсутствующая лампа не блокирует другую.
5. На удаляемых тестовых сопряжениях: физическое окно, отказ после истечения, сброс ключей.
6. Посадку PCB/USB, крепление LED и зацепление винтов корпуса по инструкции печати.

Сборки не заменяют эти проверки. Не прошивайте рабочую плату ради обновления приложения/документации.
