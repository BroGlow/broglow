<a href="../../../">О проекте</a>

<a name="alternatives"></a>

# Другие способы установки

Рекомендуемый способ — [Homebrew](installation.md#homebrew); каждый способ
заканчивается той же командой `broglow install`.

<a name="mac-package"></a>

## Пакет для Mac

Полный пакет для Mac содержит приложение, прошивку ESP32-C3 и руководство.
Сборка для Apple Silicon проверена на реальном Mac; для Intel установка и Bluetooth
не проверялись.

<!-- mac-downloads -->

Проверьте, распакуйте и установите пакет по разделу
[Установка готового пакета](hardware.md#install-a-prebuilt-package). Затем
[проверьте PATH](#path) и [подключите лампу](installation.md#pairing).

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

<a name="path"></a>

## Запуск broglow из любой папки (PATH)

Готовый пакет и сборка из исходников кладут команду в `~/.cargo/bin/broglow` (для
готового пакета Cargo не нужен); `$(brew --prefix)/bin` Homebrew уже есть в PATH.
В новом терминале вне каталога пакета или исходников проверьте:

--8<-- "site/guide/ru/hardware.md:path-check"

Если первая строка — не ваш `~/.cargo/bin/broglow` (он выводится полным путём, например
`/Users/имя/.cargo/bin/broglow`), добавьте эту строку **один раз** в `~/.zshrc`
(стандартная оболочка macOS), сохранив остальные настройки:

--8<-- "site/guide/ru/hardware.md:path-setup"

Откройте новый терминал и повторите проверку. Если `~/.zshrc` уже подключает
`"$HOME/.cargo/env"` (эту строку добавляет установщик Rust), ничего менять не нужно;
для другой оболочки используйте её файл запуска. `broglow install` не меняет `PATH`.

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
