#!/usr/bin/env python3
"""Assemble the release firmware directory from an existing PlatformIO build.

The four images at their flash offsets, the sources that built them (the core
is LGPL), licenses and notices, and flash-manifest.json. `broglow flash` and
the app's Update firmware button read this directory; the Mac package and the
Windows archive both ship it. Never builds or touches hardware.
"""

import argparse
import importlib.util
import json
from pathlib import Path
import re
import shutil
import struct

ROOT = Path(__file__).resolve().parents[1]
C3 = ROOT / "firmware/esp32-c3-supermini"
_spec = importlib.util.spec_from_file_location(
    "third_party_notices", ROOT / "scripts/third-party-notices.py"
)
notices = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(notices)

FIRMWARE_DEPENDENCIES = {
    "": """# Firmware dependencies

[English](DEPENDENCIES.md) | [Русский](DEPENDENCIES.ru.md)

The prebuilt images in this directory were built from the included `src/main.cpp`
and `platformio.ini` with PlatformIO Core 6.2.0, platform espressif32 7.1.3,
Arduino-ESP32 2.0.17 and its precompiled ESP-IDF v4.4.7 libraries. The app writes
them to a lamp: `broglow flash`, or **Update firmware** in Lamp details.

- [Third-party notices](THIRD-PARTY-NOTICES.md): every library linked into
  `firmware.bin`, its license and the notice texts.
- [ARDUINO-LICENSE.md](ARDUINO-LICENSE.md): Arduino-ESP32 core, LGPL-2.1-or-later.
- [BLE-LICENSE.txt](BLE-LICENSE.txt): ESP32 BLE Arduino library, Apache-2.0.
- Sources: https://github.com/espressif/arduino-esp32/tree/2.0.17 and
  https://github.com/espressif/esp-idf/tree/v4.4.7.

## Rebuild with a modified library

The build compiles the Arduino-ESP32 core and the BLE library from the framework
sources and links them with `src/main.cpp`; the ESP-IDF libraries are linked as
precompiled archives. To build with your own version of the core or a library:

1. Install [PlatformIO Core](https://docs.platformio.org/en/latest/core/) 6.2.0.
   From this directory, run `pio run`. PlatformIO downloads the pinned framework
   to `~/.platformio/packages/framework-arduinoespressif32`.
2. Copy that directory and edit the copy. In `platformio.ini`, add
   `platform_packages = framework-arduinoespressif32 @ symlink:///path/to/copy`
   to `[env:esp32-c3-supermini]`.
3. Run `pio run` again. The new images are written to
   `.pio/build/esp32-c3-supermini/` in this directory; outside a source checkout
   they report the firmware version `unknown`.
4. Copy its `bootloader.bin`, `partitions.bin` and `firmware.bin` over the images
   here (keep `boot_app0.bin`) and run `broglow flash --firmware` with this
   directory's path.
""",
    ".ru": """# Зависимости прошивки

[English](DEPENDENCIES.md) | [Русский](DEPENDENCIES.ru.md)

Готовые образы в этой папке собраны из включённых `src/main.cpp` и
`platformio.ini` с PlatformIO Core 6.2.0, платформой espressif32 7.1.3,
Arduino-ESP32 2.0.17 и её предкомпилированными библиотеками ESP-IDF v4.4.7.
Записывает их в лампу приложение: `broglow flash` или кнопка **Update firmware**
в деталях лампы.

- [Лицензии сторонних компонентов](THIRD-PARTY-NOTICES.md): все библиотеки,
  скомпонованные в `firmware.bin`, их лицензии и тексты уведомлений.
- [ARDUINO-LICENSE.md](ARDUINO-LICENSE.md): ядро Arduino-ESP32, LGPL-2.1-or-later.
- [BLE-LICENSE.txt](BLE-LICENSE.txt): библиотека ESP32 BLE Arduino, Apache-2.0.
- Исходники: https://github.com/espressif/arduino-esp32/tree/2.0.17 и
  https://github.com/espressif/esp-idf/tree/v4.4.7.

## Сборка с изменённой библиотекой

Сборка компилирует ядро Arduino-ESP32 и библиотеку BLE из исходников фреймворка
и компонует их с `src/main.cpp`; библиотеки ESP-IDF подключаются готовыми
предкомпилированными архивами. Чтобы собрать прошивку со своей версией ядра
или библиотеки:

1. Установите [PlatformIO Core](https://docs.platformio.org/en/latest/core/) 6.2.0.
   Из этой папки выполните `pio run`. PlatformIO скачает закреплённый фреймворк
   в `~/.platformio/packages/framework-arduinoespressif32`.
2. Скопируйте эту папку и измените копию. В `platformio.ini` добавьте
   `platform_packages = framework-arduinoespressif32 @ symlink:///path/to/copy`
   в `[env:esp32-c3-supermini]`.
3. Снова выполните `pio run`. Новые образы появятся в
   `.pio/build/esp32-c3-supermini/` в этой папке; вне чекаута исходников они
   сообщают версию прошивки `unknown`.
4. Скопируйте оттуда `bootloader.bin`, `partitions.bin` и `firmware.bin` поверх
   образов в этой папке (`boot_app0.bin` оставьте) и выполните
   `broglow flash --firmware` с путём к этой папке.
""",
}


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def write_firmware_dependencies(destination):
    for suffix, text in FIRMWARE_DEPENDENCIES.items():
        (destination / f"DEPENDENCIES{suffix}.md").write_text(text, encoding="utf-8")


def write_firmware_notices(destination, framework, document):
    """Copy the firmware license files and write its notices and guides."""
    shutil.copy2(
        ROOT / "licenses/arduino-esp32-2.0.17-LICENSE.md",
        destination / "ARDUINO-LICENSE.md",
    )
    shutil.copy2(framework / "libraries/BLE/LICENSE", destination / "BLE-LICENSE.txt")
    (destination / notices.NOTICES).write_text(document, encoding="utf-8")
    write_firmware_dependencies(destination)


def firmware_sources():
    """Inputs whose change needs a rebuild before packaging. Cargo.toml also
    feeds version.py, but the stamped version is checked directly instead."""
    return [C3 / "platformio.ini", C3 / "version.py", C3 / "src/main.cpp"]


def package_firmware(destination, platformio_core, version):
    """Verify the pinned build in firmware/esp32-c3-supermini and write the
    release firmware directory. Returns (framework_version, linked_archives)."""
    require(not destination.exists(), f"Refusing to overwrite {destination}")
    build = C3 / ".pio/build/esp32-c3-supermini"
    framework = platformio_core / "packages/framework-arduinoespressif32"
    framework_version = json.loads((framework / "package.json").read_text())["version"]
    require(
        framework_version.startswith("4.20017."),
        "Expected pinned Arduino-ESP32 2.0.17.",
    )
    toolchain = platformio_core / "packages/toolchain-riscv32-esp"
    linker_map = build / "firmware.map"
    require(
        linker_map.is_file(),
        f"Missing {linker_map}; build the pinned PlatformIO project first.",
    )
    segments = [
        (0x0000, build / "bootloader.bin"),
        (0x8000, build / "partitions.bin"),
        (0xE000, framework / "tools/partitions/boot_app0.bin"),
        (0x10000, build / "firmware.bin"),
    ]
    for _, path in segments:
        require(
            path.is_file(),
            f"Missing {path}; build the pinned PlatformIO project first.",
        )
    # Verify the actual partition table, not a guess from the board model.
    table = (build / "partitions.bin").read_bytes()
    partitions = {}
    for offset in range(0, len(table), 32):
        magic, kind, subtype, start, size, label, _ = struct.unpack_from(
            "<HBBII16sI", table, offset
        )
        if magic != 0x50AA:
            break
        partitions[label.rstrip(b"\0").decode()] = (kind, subtype, start, size)
    require(
        partitions.get("nvs") == (1, 2, 0x9000, 0x5000)
        and partitions.get("otadata") == (1, 0, 0xE000, 0x2000)
        and partitions.get("app0") == (0, 0x10, 0x10000, 0x140000),
        "Partition layout changed; review flash offsets and NVS preservation before packaging.",
    )
    require(
        (build / "firmware.bin").stat().st_size <= 0x140000, "Application exceeds app0."
    )
    require(
        f"version={version}\n".encode() in (build / "firmware.bin").read_bytes(),
        f"Firmware does not report version {version}; rebuild it from this checkout.",
    )
    for index, (offset, path) in enumerate(segments):
        end = (offset + path.stat().st_size + 4095) // 4096 * 4096
        require(
            end <= 0x9000 or offset >= 0xE000, f"{path.name} would erase NVS sectors."
        )
        require(
            index == len(segments) - 1 or end <= segments[index + 1][0],
            f"{path.name} overlaps the next flash segment.",
        )
    for artifact in (build / "firmware.bin", linker_map):
        require(
            all(
                p.stat().st_mtime_ns <= artifact.stat().st_mtime_ns
                for p in firmware_sources()
            ),
            f"{artifact.name} predates an input; rebuild before packaging.",
        )
    try:
        document, linked = notices.firmware_notices(framework, toolchain, linker_map)
    except notices.NoticeError as error:
        raise SystemExit(f"Firmware notices: {error}")

    destination.mkdir(parents=True)
    for _, path in segments:
        shutil.copy2(path, destination / path.name)
    shutil.copy2(C3 / "platformio.ini", destination / "platformio.ini")
    shutil.copy2(C3 / "version.py", destination / "version.py")
    shutil.copytree(C3 / "src", destination / "src")
    shutil.copytree(
        C3 / "tests",
        destination / "tests",
        ignore=shutil.ignore_patterns("__pycache__"),
    )
    write_firmware_notices(destination, framework, document)
    manifest = {
        "chip": "esp32c3",
        "flash_mode": "dio",
        "flash_freq": "40m",
        "flash_size": "4MB",
        "segments": [
            {"offset": hex(offset), "file": path.name} for offset, path in segments
        ],
        "preserved_nvs": {"offset": "0x9000", "size": "0x5000"},
        "requires_matching_existing_partition_layout": True,
    }
    (destination / "flash-manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n"
    )
    return framework_version, linked


def cargo_version():
    return re.search(
        r'^version = "([^"]+)"$', (ROOT / "Cargo.toml").read_text(), re.M
    ).group(1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--platformio-core",
        type=Path,
        default=Path.home() / ".platformio",
        help="PlatformIO core directory with the pinned framework and toolchain",
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="Directory to create as firmware/esp32-c3-supermini (must not exist)",
    )
    args = parser.parse_args()
    output = args.output.resolve()
    framework_version, linked = package_firmware(
        output, args.platformio_core, cargo_version()
    )
    print(f"{output}: Arduino-ESP32 {framework_version}, {len(linked)} linked archives")


if __name__ == "__main__":
    main()
