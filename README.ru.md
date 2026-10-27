[English](README.md) | [Русский](README.ru.md)

<a name="broglow"></a>

# BroGlow

<p align="center">
  <a href="https://github.com/BroGlow/broglow/releases/latest"><img alt="Последний релиз" src="https://img.shields.io/github/v/release/BroGlow/broglow?style=for-the-badge&labelColor=181A19&color=C25B2A&label=%D1%80%D0%B5%D0%BB%D0%B8%D0%B7&logo=github&logoColor=white"></a>
  <a href="https://github.com/BroGlow/broglow/actions/workflows/ci.yaml"><img alt="Статус CI" src="https://img.shields.io/github/actions/workflow/status/BroGlow/broglow/ci.yaml?branch=master&style=for-the-badge&labelColor=181A19&label=CI&logo=githubactions&logoColor=white"></a>
  <a href="https://github.com/BroGlow/broglow/releases"><img alt="Всего загрузок" src="https://img.shields.io/github/downloads/BroGlow/broglow/total?style=for-the-badge&labelColor=181A19&color=C25B2A&label=%D0%B7%D0%B0%D0%B3%D1%80%D1%83%D0%B7%D0%BA%D0%B8"></a>
  <a href="site/guide/ru/installation.md"><img alt="Установка через Homebrew" src="https://img.shields.io/badge/%D1%83%D1%81%D1%82%D0%B0%D0%BD%D0%BE%D0%B2%D0%BA%D0%B0-Homebrew-FBB040?style=for-the-badge&labelColor=181A19&logo=homebrew&logoColor=white"></a>
  <a href="LICENSE"><img alt="Лицензия: MIT" src="https://img.shields.io/badge/%D0%BB%D0%B8%D1%86%D0%B5%D0%BD%D0%B7%D0%B8%D1%8F-MIT-3D444D?style=for-the-badge&labelColor=181A19"></a>
</p>
<p align="center">
  <a href="https://docs.anthropic.com/en/docs/claude-code"><img alt="Работает с Claude Code" src="https://img.shields.io/badge/%D1%80%D0%B0%D0%B1%D0%BE%D1%82%D0%B0%D0%B5%D1%82%20%D1%81-Claude%20Code-D97757?style=for-the-badge&labelColor=181A19&logo=claude&logoColor=white"></a>
  <a href="https://github.com/openai/codex"><img alt="Работает с Codex" src="https://img.shields.io/badge/%D1%80%D0%B0%D0%B1%D0%BE%D1%82%D0%B0%D0%B5%D1%82%20%D1%81-Codex-10A37F?style=for-the-badge&labelColor=181A19&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyBmaWxsPSIjZmZmIiByb2xlPSJpbWciIHZpZXdCb3g9IjAgMCAyNCAyNCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48dGl0bGU%2BT3BlbkFJPC90aXRsZT48cGF0aCBkPSJNMjIuMjgxOSA5LjgyMTFhNS45ODQ3IDUuOTg0NyAwIDAgMC0uNTE1Ny00LjkxMDggNi4wNDYyIDYuMDQ2MiAwIDAgMC02LjUwOTgtMi45QTYuMDY1MSA2LjA2NTEgMCAwIDAgNC45ODA3IDQuMTgxOGE1Ljk4NDcgNS45ODQ3IDAgMCAwLTMuOTk3NyAyLjkgNi4wNDYyIDYuMDQ2MiAwIDAgMCAuNzQyNyA3LjA5NjYgNS45OCA1Ljk4IDAgMCAwIC41MTEgNC45MTA3IDYuMDUxIDYuMDUxIDAgMCAwIDYuNTE0NiAyLjkwMDFBNS45ODQ3IDUuOTg0NyAwIDAgMCAxMy4yNTk5IDI0YTYuMDU1NyA2LjA1NTcgMCAwIDAgNS43NzE4LTQuMjA1OCA1Ljk4OTQgNS45ODk0IDAgMCAwIDMuOTk3Ny0yLjkwMDEgNi4wNTU3IDYuMDU1NyAwIDAgMC0uNzQ3NS03LjA3Mjl6bS05LjAyMiAxMi42MDgxYTQuNDc1NSA0LjQ3NTUgMCAwIDEtMi44NzY0LTEuMDQwOGwuMTQxOS0uMDgwNCA0Ljc3ODMtMi43NTgyYS43OTQ4Ljc5NDggMCAwIDAgLjM5MjctLjY4MTN2LTYuNzM2OWwyLjAyIDEuMTY4NmEuMDcxLjA3MSAwIDAgMSAuMDM4LjA1MnY1LjU4MjZhNC41MDQgNC41MDQgMCAwIDEtNC40OTQ1IDQuNDk0NHptLTkuNjYwNy00LjEyNTRhNC40NzA4IDQuNDcwOCAwIDAgMS0uNTM0Ni0zLjAxMzdsLjE0Mi4wODUyIDQuNzgzIDIuNzU4MmEuNzcxMi43NzEyIDAgMCAwIC43ODA2IDBsNS44NDI4LTMuMzY4NXYyLjMzMjRhLjA4MDQuMDgwNCAwIDAgMS0uMDMzMi4wNjE1TDkuNzQgMTkuOTUwMmE0LjQ5OTIgNC40OTkyIDAgMCAxLTYuMTQwOC0xLjY0NjR6TTIuMzQwOCA3Ljg5NTZhNC40ODUgNC40ODUgMCAwIDEgMi4zNjU1LTEuOTcyOFYxMS42YS43NjY0Ljc2NjQgMCAwIDAgLjM4NzkuNjc2NWw1LjgxNDQgMy4zNTQzLTIuMDIwMSAxLjE2ODVhLjA3NTcuMDc1NyAwIDAgMS0uMDcxIDBsLTQuODMwMy0yLjc4NjVBNC41MDQgNC41MDQgMCAwIDEgMi4zNDA4IDcuODcyem0xNi41OTYzIDMuODU1OEwxMy4xMDM4IDguMzY0IDE1LjExOTIgNy4yYS4wNzU3LjA3NTcgMCAwIDEgLjA3MSAwbDQuODMwMyAyLjc5MTNhNC40OTQ0IDQuNDk0NCAwIDAgMS0uNjc2NSA4LjEwNDJ2LTUuNjc3MmEuNzkuNzkgMCAwIDAtLjQwNy0uNjY3em0yLjAxMDctMy4wMjMxbC0uMTQyLS4wODUyLTQuNzczNS0yLjc4MThhLjc3NTkuNzc1OSAwIDAgMC0uNzg1NCAwTDkuNDA5IDkuMjI5N1Y2Ljg5NzRhLjA2NjIuMDY2MiAwIDAgMSAuMDI4NC0uMDYxNWw0LjgzMDMtMi43ODY2YTQuNDk5MiA0LjQ5OTIgMCAwIDEgNi42ODAyIDQuNjZ6TTguMzA2NSAxMi44NjNsLTIuMDItMS4xNjM4YS4wODA0LjA4MDQgMCAwIDEtLjAzOC0uMDU2N1Y2LjA3NDJhNC40OTkyIDQuNDk5MiAwIDAgMSA3LjM3NTctMy40NTM3bC0uMTQyLjA4MDVMOC43MDQgNS40NTlhLjc5NDguNzk0OCAwIDAgMC0uMzkyNy42ODEzem0xLjA5NzYtMi4zNjU0bDIuNjAyLTEuNDk5OCAyLjYwNjkgMS40OTk4djIuOTk5NGwtMi41OTc0IDEuNDk5Ny0yLjYwNjctMS40OTk3WiIvPjwvc3ZnPg%3D%3D&logoColor=white"></a>
  <a href="https://opencode.ai"><img alt="Работает с opencode" src="https://img.shields.io/badge/%D1%80%D0%B0%D0%B1%D0%BE%D1%82%D0%B0%D0%B5%D1%82%20%D1%81-opencode-3D444D?style=for-the-badge&labelColor=181A19&logo=opencode&logoColor=white"></a>
  <a href="https://github.com/github/copilot-cli"><img alt="Работает с GitHub Copilot CLI" src="https://img.shields.io/badge/%D1%80%D0%B0%D0%B1%D0%BE%D1%82%D0%B0%D0%B5%D1%82%20%D1%81-Copilot%20CLI-6E40C9?style=for-the-badge&labelColor=181A19&logo=githubcopilot&logoColor=white"></a>
</p>
<p align="center">
  <img alt="Лампа: ESP32-C3" src="https://img.shields.io/badge/%D0%BB%D0%B0%D0%BC%D0%BF%D0%B0-ESP32--C3-3D444D?style=for-the-badge&labelColor=181A19&logo=espressif&logoColor=white">
  <img alt="Лампа подключается по USB или Bluetooth LE" src="https://img.shields.io/badge/%D0%BF%D0%BE%D0%B4%D0%BA%D0%BB%D1%8E%D1%87%D0%B5%D0%BD%D0%B8%D0%B5-USB%20%C2%B7%20Bluetooth%20LE-3D444D?style=for-the-badge&labelColor=181A19&logo=bluetooth&logoColor=white">
  <img alt="Работает на macOS, Windows и Linux" src="https://img.shields.io/badge/%D0%BF%D0%BB%D0%B0%D1%82%D1%84%D0%BE%D1%80%D0%BC%D1%8B-macOS%20%C2%B7%20Windows%20%C2%B7%20Linux-3D444D?style=for-the-badge&labelColor=181A19">
</p>

Монитор локальных сессий Claude Code, opencode, Codex и GitHub Copilot CLI
с приложением в строке меню, терминальной панелью и необязательной лампой ESP32-C3.
Лампа, подключённая по USB или Bluetooth LE, показывает работу, ожидание и запрос ввода.

Вы можете [установить и настроить BroGlow](site/guide/ru/installation.md) ещё до
сборки или получения лампы. Приложение и терминальная панель уже показывают ваши
локальные сессии агентов и их статусы без устройства; лампу можно добавить позже.

<p align="center">
  <img src="site/assets/photos/broglow-all-lights.jpg" width="420" alt="Лампа BroGlow с тремя горящими огнями — красным, жёлтым и зелёным — рядом с оранжевым маскотом">
</p>

**[Руководство на сайте](https://broglow.github.io/broglow/docs/ru/)**: установка,
сборка лампы, подключение Codex и решение проблем.

Проект поддерживает **Suren Khorenyan**. Уведомления об авторских правах — в [LICENSE](LICENSE).

<p align="center">
  <img src="site/assets/photos/signal-yellow.jpg" width="420" alt="Лампа BroGlow с единственным горящим жёлтым огнём, соответствующим статусу приостановленных сессий в приложении">
  <img src="site/assets/photos/popover-dark.png" width="353" alt="Окно BroGlow в строке меню: три активные сессии — одна работает, две на паузе — и кнопки Dashboard, Settings и Lamps">
</p>

<a name="clawlight"></a>

## Чем BroGlow отличается от ClawLight

BroGlow — отдельное, независимо поддерживаемое приложение, вдохновлённое
[ClawLight](https://github.com/clawlight/clawlight-cli). Большая часть приложения
переписана, интерфейс существенно переработан, а возможности расширены: сессии
открывают свой чат и показывают настоящие названия, беспроводные лампы управляются
по отдельности, Homebrew устанавливает и обновляет приложение, а маскот на магнитах
можно заменить. Полное сравнение — на странице [Почему BroGlow](site/guide/ru/why.md).

<a name="start-here"></a>

## Начните здесь

- **Установка или обновление:** [руководство по установке](site/guide/ru/installation.md) (Homebrew), [другие способы установки](site/guide/ru/alternatives.md) (пакет для Mac, исходники, Windows, Linux) или поручите настройку coding-агенту по [инструкции для агента](site/guide/ru/hardware.md#agent-runbook).
- **Подключение прошитой лампы:** [сопряжение](site/guide/ru/installation.md#pairing).
- **Сборка и подключение лампы ESP32-C3:** [схема подключения](site/guide/ru/hardware.md#wiring-and-resistor-choice).
- **Печать корпуса:** скачайте модели на [Printables](https://www.printables.com/model/1884645) или [MakerWorld](https://makerworld.com/models/3468773), затем следуйте [инструкции по печати и сборке](models/magnetic/GUIDE_ROUND3X2.md). Маскот держится на четырёх магнитах Ø3×2 мм N52 (два в маскоте, два в светофоре), и его можно заменить на любого другого.
- **Что-то не работает:** [решение проблем](site/guide/ru/troubleshooting.md).

Для новой или непрошитой C3 следуйте [инструкции по прошивке и резервному копированию](site/guide/ru/hardware.md#flash-and-recover-usb-upload).

| Платформа | Пакет | Лампа |
| --- | --- | --- |
| macOS | Приложение, прошивка C3 и руководства | USB или Bluetooth LE |
| Windows | Приложение в трее, CLI и образы прошивки (`broglow.exe flash`); без установщика и руководств | USB |
| Linux | Сборка из исходников | USB |

Корпус также можно скачать отдельным архивом из GitHub Releases.

<a name="install"></a>

## Установка

На macOS быстрее всего через Homebrew, затем обязательный `broglow install`: он
регистрирует хуки в ваших coding-агентах, устанавливает приложение и включает его
запуск при входе:

```bash
brew install broglow/tap/broglow
broglow install
```

Чтобы установить конкретный релиз, используйте `broglow/tap/broglow@ВЕРСИЯ`, например
`brew install broglow/tap/broglow@1.0.0`; brew подскажет, как поставить его первым в `PATH`.

Пакет для Mac из Releases, Windows, Linux и сборка из исходников описаны в
[других способах установки](site/guide/ru/alternatives.md). Установка приложения
не прошивает плату.

На macOS нажмите **⌘ Space**, найдите **BroGlow** и нажмите Return, чтобы открыть
управление. Приложение находится в `~/Applications/BroGlow.app`.
Также можно выполнить `open "$HOME/Applications/BroGlow.app"`.
Команда `broglow` в Terminal открывает необязательную терминальную панель, если
`~/.cargo/bin` есть в [PATH](site/guide/ru/alternatives.md#path).

<a name="connect-your-coding-agents"></a>

## Подключение агентов

Установка всегда регистрирует хуки Claude Code и добавляет хуки для других
обнаруженных агентов.
Подробности по каждому агенту: [Подключите coding-агентов](site/guide/ru/agents.md).

| Агент | Особенности настройки |
| --- | --- |
| Claude Code | Хуки в `~/.claude/settings.json`. |
| opencode | Устанавливается плагин; перезапустите работающие сессии. |
| Codex | Codex >= 0.144; одобрите хуки через `/hooks`. [Подключение Codex](site/guide/ru/agents.md#codex-setup). |
| GitHub Copilot CLI | Устанавливаются хуки; перезапустите сессии, открытые во время установки. |

Облачные задачи и обычные разговоры в ChatGPT не вызывают локальные хуки на этом Mac.

<a name="use-it"></a>

## Использование

Значок в строке меню или в трее открывает список сессий. На macOS раздел **Lamps**
добавляет лампу (настройка Bluetooth через USB), задаёт ей имя и яркость; сохранённые
лампы работают независимо. На Windows и Linux лампа подключается по USB.
Кнопка **×** скрывает сессию до новой активности; разговор остаётся в приложении агента.

```text
broglow --version               Узнать установленный релиз
broglow                         Открыть терминальную панель
broglow ble pair                Настроить прошитую C3 через USB и выполнить сопряжение по Bluetooth (macOS)
broglow ble status              Показать, доходят ли статусы до лампы
broglow dashboard-button show   Показать в приложении кнопки терминальной панели
broglow uninstall               Удалить хуки, автозапуск, приложение (macOS) и все данные BroGlow
```

Запрос ввода всегда имеет приоритет (красный); в **Settings** (macOS и Windows) можно
выбрать, каким цветом — жёлтым или зелёным — показывать состояние, когда одни сессии
работают, а другие ждут.

<a name="contributing"></a>

## Участие в проекте

Будем рады сообщениям об ошибках, идеям, улучшениям документации и кода.
Начните с [руководства для участников](CONTRIBUTING.ru.md).

<a name="license"></a>

## Лицензия

[MIT](LICENSE). Пакеты релиза содержат уведомления о сторонних компонентах: лицензии Rust-крейтов,
из которых собрано приложение, и уведомления стандартной библиотеки Rust; пакеты для Mac также
перечисляют скомпонованные библиотеки прошивки и их лицензии. Для иллюстраций действуют
[правила работы с изображениями](MEDIA.ru.md).
