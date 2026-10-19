[English](CONTRIBUTING.md) | [Русский](CONTRIBUTING.ru.md)

<a name="contributing-to-broglow"></a>

# Участие в разработке BroGlow

Спасибо за помощь в развитии BroGlow. В сообщениях об ошибках указывайте ОС,
версию BroGlow, шаги воспроизведения, ожидаемый и фактический результат, а также
относящийся к делу вывод (например, `broglow ble status` или логи). В описании
изменений расскажите, что изменилось и как вы это проверили.

Для установки и использования приложения см. [README](README.ru.md) и
[руководство по настройке](site/guide/ru/hardware.md).

<a name="development-setup"></a>

## Подготовка окружения

Выполняйте команды из корня репозитория. Используйте стабильный Rust с rustfmt
и Clippy; на Windows нужен набор инструментов MSVC. На Debian/Ubuntu установите системные библиотеки
для зависимостей трея, webview и последовательного порта:

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  libgtk-3-dev libxdo-dev libayatana-appindicator3-dev libudev-dev
```

Rust-привязки GTK 3 для трея на Linux (0.18, не сопровождаются) подтягивают
`glib` 0.18 ([RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html):
BroGlow не вызывает некорректный `VariantStrIter`, а исправление из `glib` 0.20
для GTK 3 недоступно) и используемый при сборке `proc-macro-error`
([RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html)).
Сборки для Mac и Windows их не содержат.

Установите [uv](https://docs.astral.sh/uv/getting-started/installation/) 0.12.17
или новее. Проект использует Python 3.14, закреплённый в
`.python-version`; если его нет, uv скачает его сам. Синхронизируйте закреплённые инструменты разработки
и включите хук коммита:

```bash
uv sync --frozen --group dev
uv run --frozen --group dev prek install
```

Группа зависимостей `dev` в `pyproject.toml` включает группу `test` и фиксирует версии Black, clang-format и prek,
группы `docs` и `cad` — инструменты сайта и CAD, а `uv.lock` — их транзитивные
зависимости. Хуки используют нативную конфигурацию
[`prek.toml`](prek.toml).

<a name="formatting-and-hooks"></a>

## Форматирование и хуки

```bash
cargo fmt
uv run --frozen --group dev black .
uv run --frozen --group dev clang-format -i firmware/esp32-c3-supermini/src/*.cpp firmware/esp32-c3-supermini/tests/*.cpp
uv run --frozen --group dev prek run --all-files
```

Black применяет стандартный стиль ко всем Python-файлам. Хук prek запускает Black
для Python-файлов в индексе. Если Black изменил файлы, коммит прерывается:
добавьте исправления в индекс (`git add`) и повторите коммит. CI падает на неотформатированном
Python-коде. Сохраняйте одинаковую версию Black в `pyproject.toml` и `prek.toml`.

Прошивка форматируется clang-format 23.1.1 по конфигурации на основе стиля Google:
[`firmware/esp32-c3-supermini/.clang-format`](firmware/esp32-c3-supermini/.clang-format).
Хук prek не запускает clang-format, поэтому форматируйте исходники прошивки сами;
CI падает на неотформатированной прошивке.

Соглашение (автоматически не проверяется): в YAML записывайте последовательности
в блочном стиле, по одному элементу на строку, в том числе `needs` и `matrix`
в workflow CI.

<a name="build-and-test"></a>

## Сборка и тестирование

```bash
cargo build --locked
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
uv run --frozen --group dev black --check --diff .
uv run --frozen --group dev clang-format --dry-run --Werror firmware/esp32-c3-supermini/src/*.cpp firmware/esp32-c3-supermini/tests/*.cpp
uv run --frozen --group test pytest
uv run --frozen flash --build-only
uv run --frozen python scripts/check-generated-artifacts.py
```

Python-тесты используют функции pytest, явные фикстуры и параметризованные случаи
в модулях `test_*.py`. Группа `test` фиксирует pytest и включает зависимости docs,
нужные тестам сайта.

Хостовый тест прошивки (`firmware/esp32-c3-supermini/tests/test_firmware.py`)
компилирует логику прошивки системным `c++`, поэтому нужен компилятор с поддержкой C++17.
Тесты Rust включают сквозные
проверки собранного исполняемого файла. При изменении интеграций агентов, popover
или статуса ламп также запускайте `node --test tests/js/*.test.mjs` после
`cargo build --locked` (нужен Node.js 24). Подробности — в описании
[покрытия тестами и ограничений платформ](CLAUDE.ru.md#ci).

[Workflow CI](.github/workflows/ci.yaml) содержит проверки прошивки на хосте,
упаковки и платформ. При изменениях иконок также запускайте
`assets/icons/render.py --check`. CI не собирает прошивку: при её изменениях
локально запустите `uv run --frozen flash --build-only` и
[сборку `led-test`](site/guide/ru/hardware.md#standalone-led-test-firmware).

Для локального обновления приложения и проверки работающей службы следуйте
[процедуре установки из исходников](site/guide/ru/hardware.md#install-or-update-the-mac-service).
Успешная сборка сама по себе не подтверждает разрешения Bluetooth или работу
светодиодов.

<a name="documentation-and-generated-files"></a>

## Документация и сгенерированные файлы

Держите README сосредоточенным на продукте, установке и повседневном использовании. Общие инструкции
для разработки размещайте здесь; [CLAUDE.ru.md](CLAUDE.ru.md) содержит карту кода
и заметки о реализации.

Обновляйте английские и русские документы вместе, сохраняя ID разделов, команды
и технические значения. Проверки переводов, локальная сборка, сборка Pages
и проверка ссылок описаны в [руководстве по документации сайта](site/README.ru.md#keep-the-instructions-in-sync).

Сгенерированные STL, STEP/STP, 3MF и ZIP должны игнорироваться и оставаться вне
Git независимо от регистра имени. Создавайте файлы корпуса в `dist/enclosure/`:
этот каталог игнорируется целиком. Если инструмент пишет в другое место, например
в `models/`, перед коммитом убедитесь, что путь игнорируется:
`git check-ignore -v <путь>`. Коммитьте редактируемые исходники и проверенные
результаты нарезки (см. [инструкцию для сопровождающих](RELEASING.ru.md)).
Используйте только собственные изображения проекта и соблюдайте
[правила работы с медиа](MEDIA.ru.md).

<a name="releases-and-website-publishing"></a>

## Релизы и публикация сайта

Подготовка релиза, генерация корпуса, упаковка и публикация описаны в
[инструкции для сопровождающих](RELEASING.ru.md), а развёртывание Pages —
в [руководстве по сайту](site/README.ru.md#publish-to-github-pages).
