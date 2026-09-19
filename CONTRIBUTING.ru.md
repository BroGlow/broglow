[English](CONTRIBUTING.md) | [Русский](CONTRIBUTING.ru.md)

<a name="contributing-to-broglow"></a>

# Участие в разработке BroGlow

Спасибо за помощь в развитии BroGlow. В сообщениях об ошибках указывайте ОС,
версию BroGlow, шаги воспроизведения и ожидаемый результат. В описании изменений
расскажите, что изменилось и как вы это проверили.

Эти инструкции относятся к репозиторию исходников. Для установки и использования
приложения начните с [README](README.ru.md) и [руководства по настройке](docs/hardware-setup.ru.md).

<a name="development-setup"></a>

## Подготовка окружения

Выполняйте команды из корня репозитория. Используйте стабильный Rust с rustfmt
и Clippy; на Windows нужен MSVC. На Debian/Ubuntu установите системные библиотеки
для зависимостей трея, webview и последовательного порта:

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  libgtk-3-dev libxdo-dev libayatana-appindicator3-dev libudev-dev
```

Python 3.14 соответствует версии в CI. Установите закреплённые инструменты
разработки и включите хук коммита:

```bash
python3 -m venv .venv
. .venv/bin/activate
python -m pip install -r requirements-dev.txt
prek install
```

В Windows PowerShell активируйте окружение командой `.venv\Scripts\Activate.ps1`.
Зависимости разработки фиксируют версии Black и prek; хуки используют нативную
конфигурацию [`prek.toml`](prek.toml).

<a name="formatting-and-hooks"></a>

## Форматирование и хуки

```bash
cargo fmt
black .
prek run --all-files
```

Black применяет стандартный стиль ко всем Python-файлам. Хук prek форматирует
Python-файлы, добавленные в индекс; перед повторным коммитом добавьте исправления
форматирования в индекс. CI запускает `black --check --diff .` и завершается
с ошибкой, если требуется форматирование. Сохраняйте одинаковую версию Black
в `requirements-dev.txt` и `prek.toml`.

Прошивка использует clang-format 23.1.1 с конфигурацией стиля Google из репозитория.
Последовательности YAML записывайте в блочном стиле: один элемент на строку,
включая `needs` и матрицы процессов CI.

<a name="build-and-test"></a>

## Сборка и тестирование

```bash
cargo build --locked
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
black --check --diff .
python3 scripts/check-generated-artifacts.py
```

Тесты Rust включают сквозные проверки собранного исполняемого файла. При изменении
интеграций агентов после сборки также запускайте `node --test tests/js/*.test.mjs`;
CI выполняет этот набор на Linux. Подробности — в описании
[покрытия тестами и ограничений платформ](CLAUDE.ru.md#ci).
[Процесс CI](.github/workflows/ci.yaml) содержит полный набор проверок прошивки,
упаковки и платформ; запускайте проверки, относящиеся к вашим изменениям.

Для локального обновления приложения и проверки работающей службы следуйте
[процедуре установки из исходников](docs/hardware-setup.ru.md#install-or-update-the-mac-service).
Успешная сборка сама по себе не подтверждает разрешения Bluetooth или работу LED.

<a name="documentation-and-generated-files"></a>

## Документация и сгенерированные файлы

README описывает продукт, установку и повседневное использование. Общие инструкции
для разработки размещайте здесь; [CLAUDE.ru.md](CLAUDE.ru.md) содержит карту кода
и заметки о реализации.

Обновляйте английские и русские документы вместе, сохраняя ID разделов, команды
и технические значения. Проверки переводов, локальная сборка, сборка Pages
и проверка ссылок описаны в [руководстве по документации сайта](site/README.ru.md#keep-the-instructions-in-sync).

Сгенерированные STL, STEP/STP и ZIP должны игнорироваться и оставаться вне Git
независимо от регистра имени. Создавайте файлы корпуса в `dist/enclosure/`
и проверяйте пути через `git check-ignore`. Коммитьте редактируемые исходники
и проверенные свидетельства. Используйте наши изображения и соблюдайте
[правила работы с ними](docs/media.ru.md).

<a name="releases-and-website-publishing"></a>

## Релизы и публикация сайта

Подготовка релиза, генерация корпуса, упаковка и публикация описаны в
[инструкции для сопровождающих](docs/releasing.ru.md), а развёртывание Pages —
в [руководстве по сайту](site/README.ru.md#publish-to-github-pages).
