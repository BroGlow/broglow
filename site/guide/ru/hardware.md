---
title: Техническое руководство
---

<a name="broglow-setup-and-operation"></a>

# BroGlow: настройка и работа

BroGlow передаёт статусы сессий локальных агентов лампам на ESP32-C3 SuperMini по USB,
а на macOS — по Bluetooth LE. Один Mac может управлять несколькими лампами. Лампе
нужно только питание по USB, Wi-Fi не используется. Команды для исходников выполняйте
из корня репозитория, команды для пакета — из распакованного каталога пакета. Корпус
скачивается отдельным комплектом для печати. Прежде чем заявлять
о поддержке оборудования, посмотрите [ограничения проверок](#repeatable-checks-and-results).

<a name="choose-a-setup-path"></a>

## Выберите способ настройки

| Задача | С чего начать |
| --- | --- |
| Установить через Homebrew | [Установка через Homebrew]({{ repo_url }}/blob/HEAD/site/guide/ru/installation.md#homebrew), затем обязательный `broglow install` (регистрирует хуки BroGlow в ваших coding-агентах) |
| Поручить настройку Codex или другому агенту | [Инструкция для агента](#agent-runbook) |
| Установить без компиляции Rust | [Установка готового пакета](#install-a-prebuilt-package) |
| Собрать на своём Mac | [Сборка из исходников](#build-from-source) |
| Подключить уже прошитую лампу | [Настройка с помощью USB](#usb-assisted-setup) |
| Найти значок или проверить службу | [Значок в строке меню и проверка службы](#menu-bar-icon-and-service-validation) |
| Показывать статусы локальных задач настольного приложения Codex | [Подключение локальных сессий Codex](#connect-local-codex-sessions) |
| Записать готовую прошивку на новую C3 | [Запись готовой прошивки C3](#flash-prebuilt-c3-firmware) |
| Проверить подключение светодиодов без Bluetooth и управления сессиями | [Отдельная прошивка для проверки светодиодов](#standalone-led-test-firmware) |

<a name="agent-runbook"></a>

### Инструкция для агента

Вставьте этот запрос в агента (Codex, Claude Code или похожего). Открывайте его в
распакованном пакете или каталоге исходников, только если хотите этот способ вместо
Homebrew:

> Настрой BroGlow на этом Mac по «Инструкции для агента»:
> {{ repo_url }}/blob/HEAD/site/guide/ru/hardware.md#agent-runbook
> Установи через Homebrew: brew install broglow/tap/broglow, затем broglow install. Если
> в этой папке есть пакет или исходники BroGlow, используй их вместо Homebrew.
> Перед изменениями сохрани копию моих настроек. Сохрани посторонние хуки и их порядок,
> имена ламп, яркость и Bluetooth-сопряжения; служба в строке меню должна быть одна.
> Если лампа подключена, сначала убедись, что плата — ESP32-C3 SuperMini. Прошивай её,
> только если на ней нет рабочей прошивки BroGlow, и только с моего согласия.
> Запросы macOS на Bluetooth и сопряжение, а также доверие хукам в Codex (/hooks)
> оставь мне.
> В отчёте перечисли: что установлено или изменено, результат каждой проверки и что
> осталось подтвердить мне.

Агент выполняет такие шаги:

1. **Выберите способ установки.** Каталог с `BUILD.json` и `bin/broglow` — готовый
   пакет: следуйте разделу [Установка готового пакета](#install-a-prebuilt-package).
   Каталог с `Cargo.toml` и `src/` — исходники: следуйте разделу
   [Сборка из исходников](#build-from-source). Любой другой каталог означает Homebrew:
   `brew install broglow/tap/broglow`, затем `broglow install` (после снимка ниже);
   команда тогда лежит в `$(brew --prefix)/bin/broglow`. Архитектуру Mac покажет
   `uname -m` (для `arm64` нужен `aarch64-apple-darwin`, для `x86_64` —
   `x86_64-apple-darwin`).
2. **Проверьте установку и сделайте снимок.** Выполните команды проверки из раздела
   [Установка или обновление службы на Mac](#install-or-update-the-mac-service)
   и сделайте один снимок до любых изменений. Если BroGlow уже установлен и работает,
   а нужно только подключить лампу, не переустанавливайте его.
3. **Подключите лампу.** Сначала [определите плату](#build-and-identify-the-board).
   Прошивайте её, только если на ней нет рабочей прошивки BroGlow и пользователь
   согласился ([запись прошивки](#flash-and-recover-usb-upload)). Плате с рабочей
   прошивкой не нужны ни повторная прошивка, ни сброс сопряжений. Подключите одну
   лампу по USB: приложение на Mac начнёт настройку автоматически. Для повтора
   используйте **Lamps → Add lamp** или `broglow ble pair`.
4. **Оставьте подтверждения пользователю.** Попросите пользователя одобрить запросы
   macOS о Bluetooth и сопряжении и разрешить хуки BroGlow в Codex `/hooks`. Никогда
   не подделывайте хеши доверия, не записывайте `config.toml` Codex и не меняйте место
   и порядок посторонних хуков ([правила для Codex](#connect-local-codex-sessions)).
5. **Проверьте и сообщите результат каждой проверки.**
    - `launchctl print "gui/$(id -u)/io.github.broglow.menubar"` показывает
      `state = running`, и из `~/Applications/BroGlow.app` запущен ровно один процесс
      `broglow menubar` ([проверка службы](#menu-bar-icon-and-service-validation)).
    - В новом терминале вне каталога пакета или исходников `command -v broglow`
      выводит ваш `~/.cargo/bin/broglow`, а для Homebrew — его `bin/broglow`
      ([PATH](#run-broglow-from-any-directory-path)).
    - `broglow ble status` начинается с `connected`, например
      `connected: 1/1 lamps connected`.
    - Реальная локальная задача агента показывает **Working**, а после ответа — **Paused**.

    Сообщите, что проверить не удалось: нет согласия, нет платы или нужна визуальная
    проверка светодиодов, которую может сделать только пользователь. Успешная сборка
    или сохранённый ID лампы сами по себе не доказывают, что лампа работает.

Для разработки также прочитайте [AGENTS.ru.md]({{ repo_url }}/blob/HEAD/AGENTS.ru.md)
и [CONTRIBUTING.ru.md]({{ repo_url }}/blob/HEAD/CONTRIBUTING.ru.md).

<a name="wiring-and-resistor-choice"></a>

## Подключение и выбор резисторов

Перед подключением проводов отключите питание USB. Каждому светодиоду нужен свой
резистор: **330 Ω для красного и жёлтого, 220 Ω для зелёного**, мощностью 1/8 Вт
или 1/4 Вт.

<!-- --8<-- [start:lamp-wiring] -->
```text
GPIO0 ── 330 Ω ── анод красного светодиода (+)  катод (−) ── GND
GPIO1 ── 330 Ω ── анод жёлтого светодиода (+)   катод (−) ── GND
GPIO3 ── 220 Ω ── анод зелёного светодиода (+)  катод (−) ── GND
```
<!-- --8<-- [end:lamp-wiring] -->

Катод обычно имеет более короткую ножку и находится со стороны плоского края корпуса.
Светодиоды с обрезанными ножками проверьте мультиметром в режиме проверки диодов.
Катоды соединяются с общей GND, **а не через общий резистор**. Плата питается от USB-C;
светодиоды питаются от **3,3 В** выводов GPIO, а не от контакта 5 В. Используйте
отдельные индикаторные светодиоды, а не RGB-светодиод или адресную ленту.

| Светодиод | Контакт платы | Команда | Значение |
| --- | --- | --- | --- |
| Красный / верхний | `0` / GPIO0 | `R` | Сессии нужна помощь |
| Жёлтый / средний | `1` / GPIO1 | `Y` | Сессия на паузе |
| Зелёный / нижний | `3` / GPIO3 | `G` | Сессия работает |
| Нет цвета сессии | — | `N` | Выключены при свежем статусе, кроме запуска, подтверждения соединения, предпросмотра яркости и Identify |

Контакты **0, 1, 3 и G (GND) находятся на одном краю** платы. Если USB-C сверху,
на этом краю сверху вниз идут 5V, G, 3V3, 4, 3, 2, 1, 0. Контакт 2 не используйте. GPIO0, GPIO1
и GPIO3 поддерживают ШИМ и не задевают выводы конфигурации загрузки 2, 8 и 9, выводы
USB 18 и 19, кнопку BOOT (9) и синий светодиод (8). Функция внешнего кварца
32,768 кГц на GPIO0 и GPIO1 в лампе не используется.
[Назначение выводов Espressif](https://documentation.espressif.com/esp32-c3_datasheet_en.html),
[справочник GPIO](https://docs.espressif.com/projects/esp-idf/en/v5.4/esp32c3/api-reference/peripherals/gpio.html).

`I = (3.3 V − Vf) / R`: при `Vf = 2.0 V` резистор 330 Ω даёт **3,9 мА / 5 мВт**,
470 Ω — 2,8 мА, 220 Ω — 5,9 мА. Яркость зависит от прямого напряжения светодиода.
Для других светодиодов начните с 330 Ω на каждом и подберите номиналы в пределах
их допустимого тока. Никогда не обходитесь без резистора.

<a name="build-and-identify-the-board"></a>

## Сборка и определение платы

Установите [uv](https://docs.astral.sh/uv/getting-started/installation/). Он берёт
версии PlatformIO Core, esptool и pyserial, закреплённые в `uv.lock`. Инструменты Rust
и команда `broglow` устанавливаются в `~/.cargo/bin/`.

```bash
# Начните в каталоге исходников BroGlow.
export PATH="$HOME/.cargo/bin:$PATH"

# Только сборка; ни одна из команд не обращается к плате.
uv run --frozen flash --build-only
cargo build --locked

# Список последовательных USB-устройств; точную модель платы он не определяет.
uv run --frozen platformio device list
```

Перед записью прошивки или командами в последовательный порт физически определите
плату **ESP32-C3 SuperMini**. Сравните список портов до и после подключения кабелем
передачи данных; при необходимости отключите похожие платы. USB VID Espressif `0x303A`
общий для многих чипов, поэтому никогда не выбирайте порт по VID, по имени из примера
или по порядку в списке. Если подходящая плата не появилась, остановитесь. Замените
заглушку определённым портом:

```bash
BOARD_PORT='/dev/cu.REPLACE_WITH_IDENTIFIED_BOARD'
```

Профиль сборки `esp32-c3-devkitm-1` задаёт чип C3, **4 МБ флеш-памяти**, DIO / 40 МГц
и встроенный USB; разводку платы SuperMini он не описывает. Определив плату, проверьте
чип и объём памяти:

```bash
uv run --frozen esptool --chip esp32c3 --port "$BOARD_PORT" chip-id
uv run --frozen esptool --chip esp32c3 --port "$BOARD_PORT" flash-id
```

Эти команды ненадолго переводят плату в загрузчик, а затем перезапускают её. Нужны
**ESP32-C3 / 4 МБ**; любые расхождения устраните до записи.
[Профиль PlatformIO](https://docs.platformio.org/en/latest/boards/espressif32/esp32-c3-devkitm-1.html).

<a name="flash-and-recover-usb-upload"></a>

## Запись прошивки и восстановление USB-загрузки

Прошивайте только плату без рабочей прошивки BroGlow. Штатная команда одинакова
в каталоге исходников и в распакованном пакете для Mac:

```bash
uv run --frozen flash
```

Команда работает только с одной однозначно определённой последовательной USB-платой,
проверяет ESP32-C3 ровно с 4 МБ памяти, спрашивает подтверждение перед записью и после
перезапуска требует точный ответ BroGlow на `INFO`. Параметры:

- `--port "$BOARD_PORT"` выбирает определённый порт, например если подключено
  несколько плат.
- `--backup` сначала читает все 4 МБ в приватный файл в `.recovery/`, выводит его
  SHA-256 и предупреждает, что в копии могут быть ключи Bluetooth. Без этого параметра
  копия не создаётся.
- `--skip-info` пропускает только проверку прошивки, которая сейчас на плате.
- `--yes` пропускает подтверждение; с ним обязателен `--port`.

Перед первым обращением к последовательному порту команда приостанавливает управление
лампой и ждёт три секунды, пока остановится обработчик лампы. После проверки, отмены
или ошибки она снова включает управление (`broglow ble on`), если оно было включено;
отключённая лампа остаётся отключённой. `~/.cargo/bin/broglow` команда находит, даже
если этот каталог не добавлен в PATH.

В исходниках команда собирает и записывает прошивку через PlatformIO. В пакете она
проверяет `flash-manifest.json` и четыре готовых образа, а затем записывает их через
esptool. Она никогда не стирает всю память и не записывает `firmware.bin` по нулевому
адресу.

Для ручной записи из исходников сначала определите плату. Если BroGlow установлен,
отключите управление лампой:

```bash
broglow ble off
```

Подождите три секунды, закройте `broglow led` и мониторы порта, затем запишите:

```bash
uv run --frozen platformio run -d firmware/esp32-c3-supermini \
  -t upload --upload-port "$BOARD_PORT"
```

Если соединиться не удаётся, удерживайте **BOOT**, нажмите и отпустите **RESET**, затем
отпустите **BOOT**. Снова выведите список портов, обновите `BOARD_PORT` и повторите.
Если после этого прошивка не запустилась, нажмите RESET. Проверьте, что кабель
передаёт данные.
[Инструкция Espressif](https://docs.espressif.com/projects/esp-idf/en/stable/esp32c3/get-started/establish-serial-connection.html).
После успешной записи снова включите управление: `broglow ble on`.

Результаты сборки находятся в `firmware/esp32-c3-supermini/.pio/build/esp32-c3-supermini/`.
PlatformIO записывает загрузчик, таблицу разделов и приложение по их штатным адресам.
**Никогда не записывайте образ приложения `firmware.bin` по нулевому адресу.** Прошивка
обновляется только по USB — этой командой или процедурой esptool ниже; обновления через
последовательный порт или Bluetooth в прошивке нет.

После изменения `LED_RED_PIN`, `LED_YELLOW_PIN`, `LED_GREEN_PIN` или
`STATUS_TIMEOUT_MS` в [platformio.ini](../../../firmware/esp32-c3-supermini/platformio.ini)
пересоберите прошивку. Параметры встроенного светодиода описаны в разделе
[об индикации](#pairing-feedback-and-diagnostics-without-external-leds).

<a name="flash-prebuilt-c3-firmware"></a>

### Запись готовой прошивки C3

Для лампы с рабочей прошивкой этот шаг не нужен. Пакет для Mac содержит четыре образа
и `firmware/esp32-c3-supermini/flash-manifest.json`, поэтому компилятор прошивки
не нужен. Проверив контрольные суммы пакета, выполните `uv run --frozen flash`
из каталога пакета.

Ручная процедура esptool ниже нужна для сложного восстановления. Сначала
[определите плату](#build-and-identify-the-board). Задайте `PACKAGE_DIR` — распакованный
каталог пакета из раздела [Установка готового пакета](#install-a-prebuilt-package);
там uv предоставляет закреплённые esptool и pyserial:

```bash
PACKAGE_DIR="$HOME/Downloads/broglow-vVERSION-aarch64-apple-darwin"  # ваш каталог пакета
cd "$PACKAGE_DIR"
uv run --frozen python -m serial.tools.list_ports -v
# Задайте BOARD_PORT по физически определённой плате, а не по примеру из инструкции.
uv run --frozen esptool --chip esp32c3 --port "$BOARD_PORT" chip-id
uv run --frozen esptool --chip esp32c3 --port "$BOARD_PORT" flash-id
```

Нужны **ESP32-C3 / 4 МБ**. Отключите управление лампой командой `broglow ble off`,
подождите три секунды и закройте программы, открывшие порт. Чтобы вручную сделать
необязательную полную копию:

```bash
RECOVERY_ROOT="$PACKAGE_DIR/.recovery"
mkdir -p -m 700 "$RECOVERY_ROOT"
chmod 700 "$RECOVERY_ROOT"
FLASH_BACKUP="$RECOVERY_ROOT/c3-before-flash-$(date +%Y%m%d-%H%M%S).bin"
(umask 077; uv run --frozen esptool --chip esp32c3 --port "$BOARD_PORT" \
  read-flash 0x0 0x400000 "$FLASH_BACKUP")
shasum -a 256 "$FLASH_BACKUP"
```

Копия может содержать ключи Bluetooth, поэтому храните её приватно. Сверьте адреса
в манифесте с адресами ниже и запишите образы закреплённым
[esptool 5.4.0](https://pypi.org/project/esptool/5.4.0/):

```bash
FIRMWARE_DIR="$PACKAGE_DIR/firmware/esp32-c3-supermini"
cat "$FIRMWARE_DIR/flash-manifest.json"
uv run --frozen esptool --chip esp32c3 --port "$BOARD_PORT" \
  --baud 460800 write-flash --flash-mode dio --flash-freq 40m --flash-size 4MB \
  0x0 "$FIRMWARE_DIR/bootloader.bin" \
  0x8000 "$FIRMWARE_DIR/partitions.bin" \
  0xe000 "$FIRMWARE_DIR/boot_app0.bin" \
  0x10000 "$FIRMWARE_DIR/firmware.bin"
```

Запись должна пройти успешно с проверкой хеша, а прошивка — запуститься. Затем снова
включите управление командой `broglow ble on` или, для новой платы,
[выполните сопряжение через USB](#usb-assisted-setup). Эта запись сохраняет область
NVS `0x9000..0xe000` (включая сопряжения) только при совпадающей разметке разделов;
перед заменой посторонней прошивки сделайте полную копию. Не используйте `erase_flash`
при обычном обновлении и никогда не записывайте образ приложения по нулевому адресу.
Если записать не удаётся, используйте восстановление через BOOT и RESET, описанное выше.

<a name="full-flash-recovery-image"></a>

### Образ полной флеш-памяти для восстановления

Полный дамп памяти содержит прошивку и ключи Bluetooth. Храните его приватно вместе
с хешем и сведениями о плате, никому не передавайте и не восстанавливайте на плату
дамп другой платы. Полный дамп **4 МБ** начинается с нулевого адреса, в отличие
от образа приложения `firmware.bin`. Восстановление заменяет прошивку и сопряжения.
Закройте программы, открывшие порт, и восстанавливайте только на ту плату, с которой
дамп снят:

```bash
uv run --frozen esptool --chip esp32c3 --port "$BOARD_PORT" \
  write-flash 0x0 "$FLASH_BACKUP"
```

<a name="standalone-led-test-firmware"></a>

## Отдельная прошивка для проверки светодиодов

Соберите `led-test`, чтобы проверить провода без Bluetooth, состояния сессий и ШИМ.
Прошивка по очереди включает **красный GPIO0, жёлтый GPIO1, зелёный GPIO3** на полном
цифровом уровне, по секунде каждый; встроенный синий светодиод вспыхивает на 100 мс
при каждой смене. Каждому светодиоду нужен свой резистор и катод на общей GND. Пакет
содержит исходники теста, но не готовый тестовый образ.

[Заново определите плату и порт](#build-and-identify-the-board), проверьте
**C3 / 4 МБ**, закройте мониторы порта и выполните из каталога исходников:

```bash
broglow ble off
# Подождите, пока работающий обработчик лампы освободит устройство.
sleep 3
uv run --frozen platformio run -d firmware/esp32-c3-supermini -e led-test
uv run --frozen platformio run -d firmware/esp32-c3-supermini -e led-test \
  -t upload --upload-port "$BOARD_PORT"
uv run --frozen platformio device monitor --port "$BOARD_PORT" --baud 115200
```

Убедитесь, что запись прошла с проверкой. Сравните строки
`led-test color=red gpio=0 duration_ms=1000` и `pin_levels red=1 yellow=0 green=0`
с горящим светодиодом; затем идут жёлтый (GPIO1) и зелёный (GPIO3). Высокий уровень
должен быть только на названном выводе. Это чтение логического уровня не измеряет
напряжение, ток и исправность светодиода. Если светодиод не горит, отключите питание
и проверьте полярность, резистор, контакты и GND; один этот тест не покажет, какая
деталь неисправна.

В тестовой прошивке **нет Bluetooth, `INFO`, сопряжения, команды `?`, команд цвета
через последовательный порт и плавной индикации при запуске**. Ctrl-C закрывает монитор, но не
останавливает цикл; отключение питания останавливает его, а сброс запускает заново.
Оставьте плату в тестовом режиме, пока пользователь не попросит вернуть обычную
работу. Тогда закройте монитор, снова определите порт и верните основную прошивку:

```bash
uv run --frozen platformio run -d firmware/esp32-c3-supermini \
  -e esp32-c3-supermini -t upload --upload-port "$BOARD_PORT"
```

Запись сохраняет сопряжения платы, поэтому для той же платы просто снова включите
управление: `broglow ble on`. Новой или сменной плате нужно
[сопряжение через USB](#usb-assisted-setup); прежние ID ламп сохраняйте, если
пользователь не попросил их забыть. Образы лежат в `.pio/build/led-test/`
и `.pio/build/esp32-c3-supermini/` каталога прошивки.

<a name="check-the-wiring-before-connecting-broglow"></a>

## Проверка подключения перед соединением с BroGlow

Для проверки нужны **основная прошивка**, определённый `BOARD_PORT`, отключённое
управление лампой и закрытые мониторы порта. Bluetooth должен быть отключён: подключённый
по Bluetooth клиент управляет светодиодами. Запускайте через uv из каталога исходников
или [каталога пакета](#flash-prebuilt-c3-firmware):

```bash
uv run --frozen python - "$BOARD_PORT" <<'PY'
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

Красный, жёлтый и зелёный должны загораться по одному на секунду, десять циклов,
после чего все гаснут (`N`). При прерывании скрипт тоже отправляет `N`. После
трёхсекундной плавной индикации при запуске свежие команды через последовательный порт заменяют
анимацию ожидания или сопряжения. Если светодиод не горит или горит тускло, отключите
питание и проверьте полярность, GPIO, резистор и общую GND. Если не горит ни один,
проверьте также прошивку, порт, яркость и то, что Bluetooth отключён. Перезапуск
питания при отключённом Bluetooth возвращает яркость 100%.

Через последовательный порт прошивка принимает полные строки `R\n`, `Y\n`, `G\n`
и `N\n` на **115200 бод**, в том числе с окончанием CRLF. Пустые, неизвестные,
многосимвольные и слишком длинные строки игнорируются. Чтобы проверить тайм-аут,
отправьте один `R\n`, затем **десять секунд** ничего не отправляйте и отправьте `?`:
ответ должен содержать `state=N`, а плата без соединения вернётся к индикации
сопряжения или ожидания. Поля `state`, `age_ms` и `mode` отличают эту индикацию
от устаревшего цвета сессии.

<a name="local-recovery-files"></a>

## Локальные файлы восстановления

Храните снимки и приватные результаты проверок оборудования в `.recovery/` в корне
исходников или пакета, с правами **0700**. Git и публичные пакеты этот каталог
не включают. Дампы прошивки могут содержать ключи Bluetooth. Сделайте один снимок
перед изменением установки, используйте его при повторных попытках и храните до
завершения проверки. Сборке и правке документации снимок не нужен. Не перезаписывайте
неизвестную прошивку, если владелец явно не разрешил потерять данные платы. Файлы
восстановления не нужны для сборки, установки и запуска релиза.

<a name="install-or-update-the-mac-service"></a>

## Установка или обновление службы на Mac

Выполняйте все команды от имени вошедшего пользователя Mac, **без sudo**. Сначала
проверьте текущую установку; на новом Mac команды и службы может не быть, это нормально:

```bash
command -v broglow
launchctl print "gui/$(id -u)/io.github.broglow.menubar"
pgrep -fl 'broglow.*menubar'
```

Перед изменениями выполните из каталога исходников или пакета этот скрипт: он сохранит
настройки, хуки, CLI и службу. Если потом удалите пакет, сохраните `.recovery/`, пока
может понадобиться откат:

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

Оба способа установки кладут CLI в `~/.cargo/bin/broglow`, поэтому хуки продолжают
работать после удаления каталога пакета; для готового пакета Cargo не нужен.
`broglow install` устанавливает или обновляет приложение, хуки и службу входа
и сохраняет посторонние записи хуков. `config.toml` Codex он никогда не записывает;
доверие хукам Codex подтверждает пользователь ([правила для Codex](#connect-local-codex-sessions)).
Перезапустите открытые сессии opencode, чтобы они загрузили новый плагин. Если PATH
находит другой `broglow`, явно выполните `~/.cargo/bin/broglow install`.

Служба `io.github.broglow.menubar`, зарегистрированная в
`~/Library/LaunchAgents/io.github.broglow.menubar.plist`, запускает:

```text
~/Applications/BroGlow.app/Contents/MacOS/broglow menubar
```

Обновляйте эту службу, а не запускайте рядом второй `cargo run -- menubar`. Блокировки
не дают запустить второй экземпляр в строке меню и не позволяют поиску ламп
и обработчику лампы одновременно использовать Bluetooth. При восстановлении из снимка
возвращайте только затронутые настройки BroGlow и сохраняйте посторонние изменения.

<a name="install-a-prebuilt-package"></a>

### Установка готового пакета

Скачайте полный пакет для Mac и его файл `.tar.gz.sha256`: `aarch64-apple-darwin`
для Apple Silicon или `x86_64-apple-darwin` для Intel. Архивы для Windows (приложение
в области уведомлений и CLI) не содержат приложения для Mac и прошивки C3. Для
установки приложения не нужны Rust, Cargo, Node.js и PlatformIO.

```bash
# Замените фактическим путём к архиву.
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

Контрольные суммы и архитектура должны совпасть, версия — быть ожидаемой,
а `ble pair --help` — работать. В `BUILD.json` записаны минимальная версия macOS
и ревизия исходников. Контрольные суммы подтверждают целостность, но не издателя,
поэтому скачивайте пакет только из доверенного источника. Пакеты BroGlow, в том числе
релизные, не заверены у Apple через Developer ID. Если macOS блокирует приложение,
проверьте источник и используйте [Open Anyway](https://support.apple.com/en-us/102445)
только для этого приложения; никогда не отключайте Gatekeeper целиком. После
[снимка](#install-or-update-the-mac-service) установите:

```bash
mkdir -p "$HOME/.cargo/bin"
install -m 755 "$PACKAGE_DIR/bin/broglow" "$HOME/.cargo/bin/broglow"
cmp "$PACKAGE_DIR/bin/broglow" "$HOME/.cargo/bin/broglow"
"$HOME/.cargo/bin/broglow" install
export PATH="$HOME/.cargo/bin:$PATH"
```

Установщик сам подписывает пакет приложения на этом Mac. Затем выполните
[настройку PATH](#run-broglow-from-any-directory-path),
[проверку службы](#menu-bar-icon-and-service-validation)
и [сопряжение через USB](#usb-assisted-setup). Включённые в пакет
[образы C3](#flash-prebuilt-c3-firmware) нужны только новым или непрошитым платам.

<a name="build-from-source"></a>

### Сборка из исходников

Нужна полная копия исходников с `Cargo.lock`, `src/ble.rs` и
`firmware/esp32-c3-supermini/platformio.ini`; пакет для Mac исходниками не является.
Сохраняйте локальные изменения. Определяйте сборку по ревизии исходников или
`BUILD.json`, а не только по номеру версии.

Понадобятся инструменты командной строки Apple (`xcode-select --install`),
[Rust stable](https://doc.rust-lang.org/book/ch01-01-installation.html)
и [uv](https://docs.astral.sh/uv/getting-started/installation/). Подготовка окружения
и полный список проверок описаны в
[CONTRIBUTING.ru.md]({{ repo_url }}/blob/HEAD/CONTRIBUTING.ru.md#build-and-test).
Первая сборка скачивает закреплённые зависимости. После
[снимка](#install-or-update-the-mac-service) соберите и установите:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo install --path . --locked
"$HOME/.cargo/bin/broglow" install
```

После каждого изменения приложения выполняйте обе команды: одна компиляция оставляет
запущенной старую службу. Изменения прошивки попадают на плату только после явной
записи. Затем выполните [проверку службы](#menu-bar-icon-and-service-validation),
[проверку PATH](#run-broglow-from-any-directory-path) и, если нужно,
[настройку лампы](#usb-assisted-setup).

<a name="run-broglow-from-any-directory-path"></a>

### Запуск broglow из любого каталога (PATH)

Готовый пакет и сборка из исходников кладут CLI в `~/.cargo/bin/broglow`; Homebrew —
в `$(brew --prefix)/bin/broglow`, который уже есть в PATH. Исходники нужны только
для пересборки. Проверьте PATH вне каталога исходников или пакета:

<!-- --8<-- [start:path-check] -->
```bash
cd /tmp
command -v broglow
type -a broglow
broglow --help
```
<!-- --8<-- [end:path-check] -->

Первым должен быть ваш `~/.cargo/bin/broglow` (или `bin/broglow` из Homebrew). Если
установленной из пакета или исходников копии нет или раньше идёт другая копия, добавьте эту строку **один раз** в `~/.zshrc`, откройте новый терминал
и проверьте снова:

<!-- --8<-- [start:path-setup] -->
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```
<!-- --8<-- [end:path-setup] -->

Если PATH уже настраивает `~/.cargo/env` из Rust, ничего менять не нужно. Остальные
настройки оболочки сохраните; другие оболочки используют свои файлы запуска. Команда
`export`, набранная в одном терминале, не действует в следующих. `broglow install`
регистрирует хуки и службу, но не PATH. Скрипты и задания по расписанию могут
не читать файлы запуска оболочки, поэтому вызывайте в них `"$HOME/.cargo/bin/broglow"`.
LaunchAgent уже использует абсолютный путь к приложению.

<a name="make-a-local-prebuilt-package"></a>

### Создание локального готового пакета

Этот раздел для сопровождающих. Как собрать пакет для Mac без публикации, описано
в [руководстве по выпуску]({{ repo_url }}/blob/HEAD/site/guide/ru/releasing.md#package-locally-without-publishing). Пакет
содержит CLI, четыре образа C3 с манифестом, это руководство и метаданные сборки.
Упаковка ничего не собирает, не устанавливает, не прошивает и не публикует.

<a name="menu-bar-icon-and-service-validation"></a>

## Значок в строке меню и проверка службы

После установки нажмите **⌘ Space**, найдите **BroGlow** и нажмите Return. Значок
светофора открывает управление сессиями, в том числе если BroGlow уже запущен. Также
можно открыть `~/Applications/BroGlow.app` в Finder или выполнить:

```bash
open "$HOME/Applications/BroGlow.app"
```

BroGlow запускается при входе и остаётся в строке меню. Команда `broglow` в Terminal
открывает необязательную терминальную панель (TUI).

Найдите **значок светофора в правой части строки меню macOS** (подсказка: **BroGlow**).
Значка в Dock и окна нет. Щелчок левой кнопкой открывает список сессий. Кнопка внизу
(**Lamp**, **Lamps 1/2** или **Lamp off**) открывает **Lamps** — там видны транспорт
и состояние каждой лампы. Разверните строку лампы, чтобы увидеть её элементы
управления; ID и диагностика находятся в **Connection details**.

Кнопки Dashboard по умолчанию скрыты. Включите **Settings → Appearance → Show
Dashboard button** или выполните:

```bash
broglow dashboard-button show
broglow dashboard-button hide
```

Это сохраняет `show_dashboard_button` и применяется сразу. Пока кнопки скрыты, BroGlow
также не открывает Terminal, если не удалось перейти к сессии. Команда `broglow`
по-прежнему открывает TUI, а **Quit** остаётся в меню правой кнопки.

Под названием каждой сессии перед проектом показаны значок и имя её агента (Claude,
Codex, opencode или Copilot). Второй значок показывает, где запущена сессия: окно —
настольное приложение агента, `>_` — терминал, `</>` — IDE. При наведении видно точное
место, например **Claude app**, **Ghostty** или **VS Code**. Отключите **Settings →
Appearance → Show agent names**, чтобы оставить только значок; имя появится при
наведении. Это сохраняет `show_agent_names`. TUI следует той же настройке в столбце
AGENT и показывает имя или короткую метку (`cc` Claude, `cx` Codex, `oc` opencode,
`co` Copilot), а место запуска называет в столбце HOST (**app** — настольное
приложение агента). Меню в трее Linux показывает метку вместо значка.

Проект — это рабочая директория, о которой сообщил агент; показывается имя папки, а
если две показанные папки называются одинаково, добавляются родительские
(`arcadia/recco` и `arcadia-wt/recco`); сессия без известной директории подписана
**No project**. Включите **Settings → Appearance → Group by project**, чтобы вместо
групп **Needs help**, **Working** и **Paused** сессии шли под заголовком своей
директории: полный путь виден при наведении, в каждой строке после агента стоит её
статус, проекты с сессией, которой нужна помощь, идут первыми, а **Clear paused**
переезжает в заголовок **By project**. Это сохраняет `group_by_project`, по умолчанию
выключено. TUI тогда сортирует строки так же и подсвечивает имя проекта в первой
строке каждого проекта, а меню в трее Linux добавляет заголовок 📁 на каждый проект.
На Linux, где нет окна Settings, задайте `"group_by_project": true` в
`~/.claude/broglow/config.json`.

| Значок | Состояние сессий |
| --- | --- |
| ![Серый](../../../assets/icons/broglow-none.png) Серый | Нет текущих сессий |
| ![Зелёный](../../../assets/icons/broglow-green.png) Зелёный | Сессии в состоянии **Working**; см. **Settings → Mixed sessions** |
| ![Жёлтый](../../../assets/icons/broglow-yellow.png) Жёлтый | Есть сессия в состоянии **Paused**; см. **Settings → Mixed sessions** |
| ![Красный](../../../assets/icons/broglow-red.png) Красный | Есть сессия в состоянии **Needs help** |

Цвет значка не показывает исправность службы или Bluetooth. **Usage tracking**
(показ расхода лимитов) в Settings по умолчанию выключен. Если значок не виден,
покажите строку меню, выйдите из полноэкранного режима или раскройте менеджер строки
меню. [Справка Apple](https://support.apple.com/guide/mac-help/whats-in-the-menu-bar-mchlp1446/mac).

Проверьте службу:

```bash
launchctl print "gui/$(id -u)/io.github.broglow.menubar"
ps -axo pid=,command= | awk '$2 ~ /(^|\/)broglow$/ && $3 == "menubar" {print}'
/usr/bin/plutil -lint "$HOME/Applications/BroGlow.app/Contents/Info.plist"
/usr/bin/codesign --verify --strict "$HOME/Applications/BroGlow.app"
"$HOME/.cargo/bin/broglow" ble status
```

Нужны `state = running` в выводе launchctl и ровно один процесс `broglow menubar`,
запущенный из `~/Applications/BroGlow.app/Contents/MacOS/broglow`, с тем же PID,
что показывает launchctl. Проверки plist и подписи подтверждают регистрацию
и целостность, но не разрешение Bluetooth. Подпись меняет исполняемый файл приложения,
поэтому не сравнивайте его хеш с CLI. `ble status` сообщает о недавних записях
в лампы: `disabled` до настройки лампы — нормально, как и
`connected: 1/2 lamps connected`, если одна из двух ламп в другом месте. Горят ли
светодиоды на самом деле, нужно проверить глазами.

Если служба загружена, но не запущена:

```bash
launchctl kickstart -k "gui/$(id -u)/io.github.broglow.menubar"
```

Повторите проверки. Если пропало приложение или регистрация службы, повторите
установку выбранным способом. После неожиданного завершения прочитайте
`~/.claude/broglow/menubar.log` и `menubar.err`. Никогда не запускайте второй
экземпляр в строке меню для проверки. Первый запуск TUI на ненастроенном Mac может
установить хуки и автозапуск.

<a name="connect-local-codex-sessions"></a>

## Подключение локальных сессий Codex

Настройка Codex для пользователя описана в разделе [Подключение Codex]({{ repo_url }}/blob/HEAD/site/guide/ru/agents.md#codex-setup).
Агент, настраивающий BroGlow, соблюдает такие правила:

- Установка BroGlow добавляет шесть записей `codex-hook` в `$CODEX_HOME/hooks.json`
  (обычно `~/.codex/hooks.json`). Пользователь должен проверить их и разрешить
  в Codex `/hooks`; никогда не подделывайте хеши доверия и не обходите проверку.
- Никогда не записывайте `config.toml` Codex. Посторонние хуки оставляйте на месте
  и в прежнем порядке.
- Если Codex установлен после BroGlow, снова выполните `broglow install`
  ([нет записей BroGlow]({{ repo_url }}/blob/HEAD/site/guide/ru/agents.md#codex-missing-hooks)).
- `broglow ble status` и ручные тестовые события не доказывают, что хуки Codex
  работают. Это доказывает только реальная локальная задача Codex, которая в списке
  сессий BroGlow показывает **Working**, а затем **Paused**.

<a name="bluetooth-identity-and-permission"></a>

## Идентификатор Bluetooth и разрешения

Включите Bluetooth и разрешите **BroGlow** в **System Settings → Privacy & Security →
Bluetooth**. Установленный пакет приложения содержит `NSBluetoothAlwaysUsageDescription`
и постоянный ID `io.github.broglow.menubar`; одного plist LaunchAgent для этого
недостаточно. `broglow ble scan` и `broglow ble pair` перезапускают себя из этого
пакета приложения, чтобы использовать его разрешение Bluetooth.

Локальная установка подписывается ad-hoc, а designated requirement проверяет только
идентификатор. Разрешение сохраняется между сборками, но подпись **не удостоверяет
издателя**. Распространители могут задать в `BROGLOW_SIGNING_IDENTITY` настоящий
сертификат подписи и не менять его; после смены подписи macOS может снова запросить
разрешение Bluetooth.

Ни проверка подписи, ни поиск, разрешённый в Terminal, не доказывают, что разрешение
есть у фоновой службы. Проверьте запись в лампу из установленной службы, когда лампа
питается от USB-адаптера, затем повторите проверку после перезапуска службы или входа
в систему и после восстановления при отказе в доступе.

[Разрешения btleplug](https://github.com/deviceplug/btleplug#macos),
[описание использования Apple](https://developer.apple.com/documentation/bundleresources/information-property-list/nsbluetoothalwaysusagedescription),
[подпись и TCC](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements),
[designated requirements](https://developer.apple.com/library/archive/technotes/tn2206/).

<a name="pair-and-remember-the-home-and-office-lamps"></a>

## Сопряжение и сохранение домашней и офисной ламп

Лампы видны по Bluetooth как `BroGlow C3` с четырёхзначным шестнадцатеричным
суффиксом платы. Mac сохраняет UUID CoreBluetooth каждой лампы. Лампа одновременно
работает только с одним Mac, а один Mac может обновлять несколько ламп сразу.

<a name="usb-assisted-setup"></a>

### Настройка с помощью USB

Подключите одну прошитую плату кабелем передачи данных USB и закройте мониторы порта.
Приложение на macOS начинает настройку автоматически в течение **пяти секунд**, в том
числе если плата была подключена ещё до запуска приложения. Чтобы повторить вручную,
выберите **Lamps → Add lamp** или выполните:

```bash
broglow ble pair
broglow ble status
```

Не отключайте плату, пока показано **Adding…**, и сразу одобрите запрос macOS
о сопряжении. Настройка определяет прошивку, открывает сопряжение и сохраняет
соответствующий ID Bluetooth. **Connected** появляется только после первой успешной
зашифрованной записи обработчика лампы. После этого USB нужен только для питания —
от Mac или от USB-адаптера; управление идёт по Bluetooth. Если подключено несколько
плат, укажите определённый порт:

```bash
broglow ble pair --port "$BOARD_PORT"
```

Для настройки нужны точный ответ `INFO`, UUID службы BroGlow и полный шестибайтовый
базовый MAC платы в данных производителя (`0xffff`). Имени или USB VID для выбора
лампы недостаточно. Автоматический поиск по USB принимает только устройства Espressif
`303A:1001` со встроенным USB, объединяет порты `tty` и `cu` одной платы и требует
ровно одного кандидата. Если выбор неоднозначен, настройки и Bluetooth не меняются,
а вторая одновременная настройка отклоняется. `PAIR` отправляется только после
проверенного ответа BroGlow на `INFO`; приложение никогда не прошивает плату само.

Автоматическая настройка делает одну попытку на каждое подключение USB. При ошибке
прежние настройки ламп восстанавливаются; повторите через **Add lamp** или отключите
USB минимум на **пять секунд** и подключите снова. Проверенные USB-идентификаторы
сохраняются в `usb_lamps`. Ранее добавленные лампы сохраняют выбранный транспорт
и состояние включения; забытую лампу можно добавить снова только через **Add lamp**.
Ход настройки и ошибки видны в **Lamps**. На Windows и Linux работа по USB не меняется;
автоматическая настройка Bluetooth есть только на macOS.

Сначала настройка читает `INFO` на **115200 бод**; если плата уже есть в
`usb_lamps`, автоматическая настройка на этом заканчивается, и повторное подключение
сохранённой лампы не отключает остальные. Пока включены лампы Bluetooth, новая лампа
находится без паузы: BroGlow отправляет `PAIR`, и **восемь секунд** работающий
обработчик ламп сообщает об объявлениях BroGlow (`ble-sightings.json`, файл пишется
только по запросу настройки). Сохраняется ровно одно объявление, полученное после
`PAIR`, с тем же шестибайтовым идентификатором, и обработчик подключает эту лампу,
не отключая остальные. Если обработчик не ответил за **три секунды** или не увидел
подходящую лампу, а также всегда, когда лампы отключены, работают по USB или лампа
подключена к этому Mac (подключённая лампа перестаёт передавать объявления), BroGlow
приостанавливает управление лампой, ждёт до **восьми секунд**, пока остановится
обработчик, отправляет `PAIR`, освобождает последовательный порт и **восемь секунд**
ищет лампу. Пока лампы работают по USB, порт занят обработчиком, поэтому пауза идёт
до `INFO`.
Если порт занят, настройка завершится ошибкой: BroGlow не перехватывает чужое
подключение. При неудаче прежние настройки ламп восстанавливаются, а посторонние
настройки, изменённые за это время, сохраняются. Закрытие панели не останавливает
настройку или восстановление. Настройка через USB тоже использует сопряжение
Just Works и требует согласия macOS.

Повторите для каждой лампы: новые ID добавляются, существующие сопряжения сохраняются.
Если плата не отвечает на `INFO`, запишите [готовую](#flash-prebuilt-c3-firmware)
или [собранную](#flash-and-recover-usb-upload) прошивку — после определения платы
и только с согласия пользователя.

<a name="pair-without-a-usb-data-cable"></a>

### Сопряжение без USB-кабеля передачи данных

BOOT — это **GPIO9, активный низкий уровень**. Нажимайте его после обычного запуска,
причём перед удержанием один раз отпустите. Если удерживать BOOT при включении
питания или RESET, плата войдёт в загрузчик ROM.

| Действие после обычного запуска | Результат |
| --- | --- |
| Нет сохранённых сопряжений | Окно сопряжения открывается автоматически на 60 секунд |
| Удерживать BOOT три секунды и отпустить | Окно на 60 секунд; сопряжения сохраняются |
| Продолжать удерживать до десяти секунд и отпустить | Запрос на удаление всех сопряжений платы, отключение, окно на 60 секунд |
| `broglow ble pair` по USB | То же окно после идентификации; сопряжения сохраняются |
| Окно истекло | Новые сопряжения отклоняются; уже сопряжённые Mac могут переподключаться |

**LE Secure Connections Just Works** шифрует записи GATT и хранит постоянные
сопряжения, но **не защищает от MITM**: нет ни PIN-кода, ни сравнения чисел.
Сопрягайте только нужную лампу во время её физического окна. Сохранённый UUID выбирает
лампу, а ключи на плате и в macOS образуют сопряжение.
[API безопасности Espressif](https://docs.espressif.com/projects/esp-idf/en/v4.4.7/esp32c3/api-reference/bluetooth/esp_gap_ble.html).

Подключите лампу к USB-адаптеру питания. Перед поиском освободите обработчик лампы
и радиомодуль:

```bash
broglow ble off
# Подождите не менее трёх секунд, затем откройте сопряжение кнопкой BOOT, как описано выше.
broglow ble scan
```

Поиск длится **восемь секунд**. Включите только нужную лампу или сравните результаты
поиска с выключенной и включённой лампой. Точно скопируйте её UUID CoreBluetooth
(он свой для каждого Mac) и подставьте вместо заглушки:

```bash
broglow ble add <HOME_ID_FROM_SCAN>
broglow ble status
broglow ble list
```

`add` включает все сохранённые лампы. Сразу одобрите доступ к Bluetooth и запрос
macOS о сопряжении: по наблюдениям, запрос закрывается примерно через **30 секунд**,
хотя окно платы и ожидание записи на Mac длятся **60 секунд**. Если время вышло,
снова откройте окно и повторите. Первая зашифрованная запись передаёт сохранённую
яркость лампы — именно она может вызвать запрос о сопряжении. Затем обработчик
отправляет текущий статус. Первая запись статуса в каждом соединении вызывает
подтверждающую вспышку **250 мс**. Объявление по Bluetooth или низкоуровневое
соединение — ещё не подтверждённая запись.

Для офисной лампы повторите: `broglow ble off`, три секунды ожидания, BOOT,
`broglow ble scan`, `broglow ble add`. Оба ID сохраняются. **Lamps 1/2** — нормально,
пока одна лампа в другом месте; когда обе рядом, ожидайте **Lamps 2/2**. У каждой
лампы свои записи и свой срок свежести, поэтому одна лампа не блокирует и не
маскирует другую.

`~/.claude/broglow/config.json` хранит `ble_peripherals` и `led_transport: "ble"`.
Чтобы забыть лампу, используйте **Lamps → строка лампы → Forget lamp** или:

```bash
broglow ble remove <ID_TO_FORGET>
```

Forget работает, даже если лампа отсутствует или отключена. Сначала сохраняются
настройки, затем удаляется строка; если сохранить не удалось, список не меняется.
Отключается только эта лампа, остальные остаются подключены. Переподключение
прекращается, но сопряжения ни на одной стороне не стираются. Удаление последнего ID
отключает Bluetooth. `broglow ble off` сохраняет ID, а `broglow ble usb` сохраняет их и
переключает на USB. Чтобы снова добавить забытую лампу, используйте настройку через USB
или ID из поиска.

<a name="replace-a-mac-or-recover-mismatched-bonds"></a>

### Замена Mac или восстановление несовпадающих сопряжений

Отключите обработчик лампы на старом Mac. Установите BroGlow на новый Mac, откройте
окно сопряжения на плате, удерживая BOOT три секунды, затем найдите и добавьте её ID.
UUID CoreBluetooth на разных Mac разные, поэтому не копируйте ID со старого Mac.

Если сопряжение снова и снова не удаётся, удерживайте BOOT **десять секунд после
запуска**, чтобы стереть сопряжения платы. Если в настройках Bluetooth macOS есть
устаревшая запись этой лампы, удалите и её, затем найдите и добавьте лампу в новом
окне. `broglow ble remove` меняет только настройки BroGlow, а не ключи macOS.
Восстановление подтверждают успешная запись и поля диагностики `authenticated`
и `bonds`. Стирание идёт асинхронно: `bond_reset=requested` ещё не означает, что оно
завершено.

<a name="pairing-feedback-and-diagnostics-without-external-leds"></a>

### Индикация сопряжения и диагностика без внешних светодиодов

При запуске все управляемые светодиоды дважды плавно загораются и гаснут. Во время
сопряжения три цвета загораются по очереди; несопряжённая лампа мигает; сопряжённая
лампа без соединения раз в 30 секунд даёт тусклый импульс. Первая запись статуса
в каждом соединении вызывает одну короткую подтверждающую вспышку, после чего
показывается свежий статус сессии. Встроенный светодиод обозначает зелёный, жёлтый
и красный одним, двумя и тремя импульсами.

Команда `?` через определённый последовательный USB-порт сообщает состояние,
коэффициенты заполнения ШИМ, режим сопряжения и сохранённые сопряжения, а также время с последнего
принятого статуса (`age_ms`). Это заданные выходные значения, а не подтверждение
видимого свечения. Интервалы индикации, `INFO`, `PAIR` и диагностические команды
описаны в [справочнике прошивки](firmware.md#pairing-feedback-and-diagnostics-without-external-leds).

<a name="everyday-operation-and-status"></a>

## Повседневная работа и статусы

Включайте и отключайте выбранный транспорт клавишей `l` в TUI или через
**Lamps → Enable/Disable lamp**. Для включения USB нужна подключённая плата;
Bluetooth включает все сохранённые лампы без USB. В командной строке
`broglow ble off` отключает управление лампами и сохраняет ID, а `broglow ble on`
снова включает выбранный транспорт.

В **Lamps** разверните строку (щелчком, Enter или Space), чтобы увидеть элементы
управления, **Name** и **Forget lamp**; **Device ID** и **Diagnostic** находятся
в **Connection details**. Имена хранятся только на Mac и могут содержать до **40
символов Unicode**; пустое имя возвращает `Lamp 1`, `Lamp 2` и так далее. Имя можно
сохранить, даже если лампа отсутствует или отключена; объявляемое имя и сопряжения
при этом не меняются. В командной строке: `broglow ble rename <SAVED_ID> "Office"`
(`""` очищает имя). В **Troubleshooting** собраны сведения о службе и настройке;
Back или Escape возвращает к сессиям. В TUI `d` открывает и закрывает подробности,
стрелки или `j`/`k` прокручивают, Escape возвращает назад.

**Identify** работает только на выбранной Bluetooth-лампе, пока она подключена.
Светодиоды загораются по одному в порядке **красный → жёлтый → зелёный** на **100%**,
по **100 мс** на светодиод, **три прохода** (всего **0,9 секунды**). Затем
возвращаются последний статус и сохранённая яркость, даже если она 0%. Identify
не включает лампу и не сохраняет настройки. Лампу, которая отсутствует, приостановлена
или получила устаревший статус, определить нельзя. Identify требует соответствующей
прошивки: старая прошивка подтверждает запись, но молча её игнорирует. Обновление
приложения не прошивает плату; чтобы обновить её, следуйте процедуре резервной копии
и записи для определённой платы.

| Статус | Значение и действие |
| --- | --- |
| `disabled` | Включите в TUI или в **Lamps** |
| `searching` | Проверьте сохранённые ID и питание лампы |
| `connecting` | Идёт соединение или поиск служб |
| `connected` | Хотя бы одна лампа недавно приняла запись; проверьте счётчик и статусы ламп |
| `error` | Прочитайте подробности: разрешение, Bluetooth, тайм-аут или GATT |
| `unavailable` | Статуса обработчика нет или он устарел; проверьте установленную службу |

Зелёная точка соединения появляется только после принятой записи; объявления
по Bluetooth или подключения по USB недостаточно. `1/2` — нормально, если одна лампа
в другом месте. Подтверждённая запись не доказывает, что светодиоды горят.

Красный цвет побеждает, если хотя бы одной сессии нужна помощь. Смешанные состояния
задаёт **Settings → Mixed sessions**: **Yellow when any session is idle**
(`any_inactive`, по умолчанию) или **Green while any session is working**
(`active_wins`). Если текущих сессий нет, лампа получает свежий `N` и после вспышки
соединения остаётся тёмной. После смены этой настройки переподключаться не нужно.

Каждая лампа получает текущий статус при соединении, при каждом изменении и каждые
**две секунды**. При переподключении старый цвет из очереди не повторяется. При
отключении BroGlow пытается отправить `N`, затем разрывает соединение и прекращает
поиск и повторы; **десятисекундный** тайм-аут стирает статус, который не удалось
доставить. По `N` лампа не может отличить отключённый Mac от отсутствующего, поэтому
сопряжённая лампа без соединения со временем вернётся к импульсу ожидания. Отключение
не гарантирует, что лампа останется тёмной.

<a name="brightness-control"></a>

### Управление яркостью

В **Lamps** разверните лампу и задайте **Light brightness: 0–100%**. **All lamps**
задаёт уровень всем сохранённым лампам, сбрасывает индивидуальные значения и становится
значением по умолчанию для новых ламп. По умолчанию яркость 100%; уже заданный общий
уровень сохраняется. Mac хранит `led_brightness` и необязательные значения
`lamp_brightness` для отдельных ID в `~/.claude/broglow/config.json`. Отсутствующая
лампа получит сохранённое значение при переподключении.

При движении ползунка выбранные подключённые лампы показывают предпросмотр всех трёх
цветов; через **две секунды** после последнего предпросмотра возвращается статус.
Пока указатель удерживается, предпросмотр продолжается; изменения с клавиатуры тоже
показываются. Отпускание или отмена, выход из **Lamps** или закрытие панели
останавливают предпросмотр. Сохраняется только подтверждённое изменение; закрытие или
отмена сами по себе ничего не сохраняют. Предпросмотр никогда не включает лампу и не
записывает настройки. Сохранять изменения можно, даже если лампа отсутствует или
отключена. Forget удаляет имя лампы и её индивидуальную яркость.

```bash
broglow ble brightness 40 --id YOUR-SAVED-UUID  # одна сохранённая лампа
broglow ble brightness 60                     # все лампы; сбросить индивидуальные значения
```

Эти команды только сохраняют настройку: лампу они не включают и предпросмотр
не показывают. Подключённые лампы получают изменения без переподключения. Яркость
масштабирует внешние светодиоды и встроенный синий светодиод, включая индикацию,
но не интервалы мигания и не красный светодиод питания. **При 0% соединение Bluetooth
и обновления статуса продолжают работать, а управляемые светодиоды не горят**;
предпросмотр может временно показать другой уровень. ШИМ не откалиброван под
воспринимаемую яркость, а восьмибитное округление сливает очень малые уровни.

Плата хранит яркость в RAM и после отключения питания или сброса возвращается к **100%**.
Mac отправляет сохранённое значение до первого статуса и вспышки соединения, поэтому
более ранняя индикация при запуске идёт на 100%. Яркость, предпросмотр и Identify
не продлевают **десятисекундный** срок статуса, а смена яркости не перезапускает
анимацию. Разрыв соединения, потеря аутентификации или устаревший статус отменяют
предпросмотр и Identify.

BroGlow требует соответствующую прошивку с характеристиками статуса и яркости,
доступными для записи; прошивку без них он отклоняет. Управление через
последовательный порт яркость не передаёт, а при переходе с Bluetooth на USB уровень
в RAM сохраняется. Чтобы вернуть 100%, перезапустите питание платы; следующее
соединение по Bluetooth восстановит сохранённую на Mac настройку.

<a name="preserve-or-return-to-usb"></a>

### Сохранение настроек USB или возврат к нему

```bash
broglow ble usb
```

Команда выбирает USB с отключённым управлением лампой и сохраняет прежние настройки
USB и сохранённые ID Bluetooth. Подключите определённую плату и включите её клавишей `l`
или в **Lamps**. Если подключено несколько устройств Espressif, задайте `led_port`
в `~/.claude/broglow/config.json` равным текущему пути определённой платы и не меняйте
остальные ключи; одного VID недостаточно.

Для диагностики только по USB в терминале:

```bash
broglow ble off
broglow led --port "$BOARD_PORT"
```

Остановите её через Ctrl-C, прежде чем снова включать управление лампой из строки
меню. Пока Bluetooth подключён, плата игнорирует команды цвета через последовательный
порт; диагностические команды для чтения продолжают работать.

<a name="ble-contract-and-firmware-behavior"></a>

## Протокол BLE и поведение прошивки

Bluetooth использует подтверждённые зашифрованные записи, повторяет статус каждые
**две секунды** и стирает его после **десятисекундного** тайм-аута. Подключённый
по Bluetooth клиент управляет светодиодами; команды цвета через последовательный порт
работают, только пока Bluetooth не подключён. Прошивка обновляется по USB. UUID,
формат сообщений, безопасность и диагностика описаны в
[справочнике прошивки](firmware.md#ble-contract-and-firmware-behavior).

<a name="gatt-cache-after-a-firmware-upgrade"></a>

### Кеш GATT после обновления прошивки

Прошивка запрашивает Service Changed, чтобы macOS увидела новые характеристики без
удаления сопряжений. Если после обновления нет яркости, переподключитесь и проверьте
возможности прошивки, прежде чем сбрасывать сопряжения. При смене закреплённого
инструментария сверьтесь с [реализацией SDK](firmware.md#gatt-cache-after-a-firmware-upgrade).

<a name="troubleshooting-and-service-recovery"></a>

## Поиск неисправностей и восстановление службы

| Симптом | Проверка и восстановление |
| --- | --- |
| Нет порта или запись не удаётся | Используйте кабель передачи данных, убедитесь, что это C3 с 4 МБ памяти, закройте программы, открывшие порт, попробуйте BOOT и RESET и снова выведите список портов. Никогда не угадывайте порт. |
| `Run broglow install` или запускается не тот CLI | Выполните `~/.cargo/bin/broglow install`; проверьте PATH и пакет приложения. |
| Bluetooth занят | Выполните `broglow ble off`, подождите три секунды, закройте другие поиски и повторите. |
| Ошибка доступа или Bluetooth выключен | Включите Bluetooth, разрешите BroGlow в **Privacy & Security → Bluetooth** и перезапустите службу. |
| Ламп нет или `searching` не заканчивается | Проверьте питание, прошивку, расстояние и сохранённый ID. Подключённая лампа может перестать объявлять себя, поэтому отключите другие клиенты; сравните поиск с выключенной и включённой лампой. |
| Сопряжение или запись отклонены | Удерживайте BOOT три секунды после обычного запуска, выполните сопряжение за 60 секунд и сразу одобрите запрос macOS (по наблюдениям, около 30 секунд). Затем попробуйте [восстановление сопряжений](#replace-a-mac-or-recover-mismatched-bonds). |
| После BOOT лампа не видна | Отпустите BOOT, нажмите RESET и дождитесь обычного запуска, прежде чем удерживать BOOT для сопряжения. |
| Подключено `1/2` | Нормально, пока одна лампа в другом месте. |
| Нет характеристики или нужно обновление для яркости | Проверьте идентификатор и возможности платы; обновите определённую C3 по описанной процедуре записи прошивки. |
| Подключена, но не горит или не тот цвет | Проверьте яркость 0%, текущие сессии и **Settings → Mixed sessions**; отключите Bluetooth и выполните [проверку подключения](#check-the-wiring-before-connecting-broglow). Подтверждённая запись не доказывает исправность цепи. |
| Светодиод тусклый или один цвет не горит | Отключите питание; проверьте полярность, GPIO, резистор, контакты и общую GND. Используйте [отдельный тест](#standalone-led-test-firmware); чтение логического уровня не доказывает напряжение, ток или повреждение платы. |
| Нет локальных сессий Codex | Разрешите шесть хуков BroGlow в Codex `/hooks` и отправьте новый запрос в локальной задаче. Если Codex установлен после BroGlow, снова выполните `broglow install` ([настройка Codex]({{ repo_url }}/blob/HEAD/site/guide/ru/agents.md#codex-missing-hooks)). |
| Команды цвета через последовательный порт игнорируются | Отключите Bluetooth. |
| Лампа гаснет, когда Mac спит | Сработал десятисекундный тайм-аут; после пробуждения Mac переподключится и снова отправит статус. |
| `unavailable` или нет значка | Проверьте launchd и журналы и восстановите существующую службу; никогда не запускайте вторую. |

Перезапуск без переустановки хуков:

```bash
launchctl kickstart -k "gui/$(id -u)/io.github.broglow.menubar"
broglow ble status
```

Прочитайте `~/.claude/broglow/menubar.log` и `menubar.err`. Если пакет приложения
пропал или повреждён либо изменился исполняемый файл, повторите
[установку готового пакета](#install-a-prebuilt-package) или
[из исходников](#build-from-source) с CLI в `~/.cargo/bin/broglow`. Установщик готовит
новое приложение рядом со старым и возвращает старое, если замена не удалась. Если
установка прервалась и осталась папка `~/Applications/.BroGlow.previous.*.app`,
в ней предыдущее приложение; не удаляйте её, пока BroGlow снова не заработает.
При восстановлении из снимка возвращайте только затронутые настройки BroGlow
и сохраняйте более поздние посторонние изменения.

`broglow uninstall` удаляет службу входа, приложение, хуки BroGlow и `~/.claude/broglow`
(состояние сессий, журналы и настройки ламп, включая сохранённые ID). `~/.cargo/bin/broglow`
остаётся. Не используйте эту команду для обычного исправления. Чтобы намеренно удалить
BroGlow, сначала выполните `broglow ble off`, затем `broglow uninstall`.

<a name="repeatable-checks-and-results"></a>

## Повторяемые проверки и результаты

Проверки для разработки (форматирование, линтеры, тесты Rust и JavaScript, тесты
прошивки на компьютере и сборки) перечислены в
[CONTRIBUTING.ru.md]({{ repo_url }}/blob/HEAD/CONTRIBUTING.ru.md#build-and-test).
CI запускает их на Linux, macOS и Windows. Тесты прошивки имитируют GPIO, часы,
последовательный порт и Bluetooth, поэтому не проверяют провода и радиосвязь.

Результаты для Apple Silicon повторены с релизным приложением и основной прошивкой.
Первое сопряжение последний раз проверялось на более ранней сборке. Обновление
определённых сопряжённых Mac и C3 до прошивки с яркостью прошло повторное обнаружение
GATT и передачу яркости без удаления лампы и повторного сопряжения. Коэффициенты
заполнения ШИМ, которые сообщает прошивка, — это диагностика, а не визуальная
проверка светодиодов.

| Область | Подтверждения и ограничения |
| --- | --- |
| Mac на Apple Silicon | Проверены установка пакета, подписанная служба, переподключение по Bluetooth с сопряжением, статус, яркость, гашение устаревшего статуса и восстановление. |
| Светодиоды | Проверяйте все цвета и яркость на каждой сборке. |
| Несколько ламп | Тесты независимости проходят; одновременная работа двух физических плат не проверена. |
| Mac на Intel, Windows, Linux | Только цели CI и сборки; физический Bluetooth и чистая установка не проверялись. Bluetooth-лампы работают только на macOS. |
| Распространение | Локальные проверки пакета и сайта выполняются перед каждой публикацией; публичные загрузки, сайт и Gatekeeper на чистом Mac проверяйте после каждого релиза. |
| Корпус | Требования к посадке и проверки сборки — в [руководстве по печати]({{ repo_url }}/blob/HEAD/models/magnetic/GUIDE_ROUND3X2.md); глубину карманов под магниты и силу удержания ещё нужно проверить на практике. Комплект для печати содержит STL, STEP, два вида сборки и `BUILD.json`. |

<a name="physical-and-background-service-acceptance"></a>

### Приёмка оборудования и фоновой службы

Этот список — для сопровождающих перед релизом. Для каждой поддерживаемой пары Mac
и платы запишите точные версии приложения и прошивки и проверьте:

1. Один подписанный процесс в строке меню, подтверждённую запись, настройку через USB,
   согласие на Bluetooth и переподключение без передачи данных по USB. Проверьте
   перезапуск службы, вход в систему и восстановление после отказа в доступе.
2. Реальные сессии в состояниях работы, паузы, запроса помощи и выключения; оба
   варианта **Mixed sessions**; все цвета светодиодов и яркость.
3. При сохранённом питании платы прерванные обновления убирают цвет сессии
   за **десять секунд**; сразу после этого восстановите службу.
4. Сброс платы, сон и пробуждение Mac, выключение и включение Bluetooth, выход
   из зоны связи, отключение и повторное включение; отсутствующая лампа не должна
   блокировать другую.
5. На одноразовых тестовых сопряжениях: физическое окно сопряжения, отказ после его
   истечения и сброс сопряжений.
6. Посадку платы и разъёма USB в корпусе, крепление светодиодов и то, как держат винты,
   по руководству по печати.

Сборки не заменяют эти проверки. Никогда не перепрошивайте рабочую плату ради
обновления приложения или документации.
