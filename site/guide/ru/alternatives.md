<a href="../../../">О проекте</a>

<a name="alternatives"></a>

# Другие способы установки

Рекомендуемый способ — [Homebrew](installation.md#homebrew). Эта страница нужна,
если Homebrew недоступен, нужен офлайн-пакет для Mac либо вы работаете на Windows
или Linux. Каждый способ заканчивается командой `broglow install`: она регистрирует
хуки в ваших coding-агентах, а на macOS ещё устанавливает приложение в строке меню и
включает его запуск при входе.

<a name="mac-package"></a>

## Пакет для Mac

Полный пакет для Mac содержит приложение, прошивку ESP32-C3 и руководство.
Сборка для Apple Silicon проверена на реальном Mac; для Intel установка и Bluetooth
пока не проверялись.

<!-- mac-downloads -->

Проверьте, распакуйте и установите пакет по разделу
[Установка готового пакета](hardware.md#install-a-prebuilt-package); он заканчивается
командой `"$HOME/.cargo/bin/broglow" install`. Затем [проверьте PATH](installation.md#path)
и [подключите лампу](installation.md#pairing).

<a name="from-source"></a>

## Сборка из исходников (macOS и Linux)

Нужен [Rust stable](https://doc.rust-lang.org/book/ch01-01-installation.html); на
macOS — ещё инструменты командной строки Apple (`xcode-select --install`), на Linux —
[системные библиотеки разработки]({{ repo_url }}/blob/HEAD/CONTRIBUTING.ru.md#development-setup)
для трея на GTK 3. Из полной рабочей копии исходников:

```bash
cargo install --locked --path .
"$HOME/.cargo/bin/broglow" install
```

Подробности и проверки: [Сборка из исходников](hardware.md#build-from-source).
Готовые сборки для Linux не публикуются; на Linux лампа работает только по USB, а
привязки GTK 3 трея имеют известные предупреждения безопасности, перечисленные в
руководстве для участников.

<a name="windows"></a>

## Windows

Скачайте `broglow-vVERSION-x86_64-pc-windows-msvc.tar.gz` из
[Releases]({{ repo_url }}/releases): приложение в трее и CLI в `broglow.exe` с
поддержкой лампы по USB; без установщика, Bluetooth, прошивки и руководств.
Распакуйте архив в постоянную папку (например, `tar -xf <файл>` в PowerShell) и
выполните из этой папки:

```powershell
.\broglow.exe install
```

`.\broglow.exe menubar` запускает трей напрямую; `.\broglow.exe` открывает
терминальную панель. Чтобы собрать из исходников на Windows, используйте набор
инструментов Rust MSVC: `cargo install --locked --path .`, затем `broglow install`.
