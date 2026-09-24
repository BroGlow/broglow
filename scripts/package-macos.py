#!/usr/bin/env python3
"""Package existing native macOS/C3 builds; never build, install, or touch hardware."""

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
C3 = ROOT / "firmware/esp32-c3-supermini"
_spec = importlib.util.spec_from_file_location(
    "third_party_notices", ROOT / "scripts/third-party-notices.py"
)
notices = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(notices)


def run(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def flash_tool_sources():
    return [ROOT / "pyproject.toml", ROOT / "uv.lock"] + [
        path
        for path in (ROOT / "tools/broglow_tools").rglob("*")
        if path.is_file() and "__pycache__" not in path.parts
    ]


def copy_flash_tool(package):
    for source in (ROOT / "pyproject.toml", ROOT / "uv.lock"):
        shutil.copy2(source, package / source.name)
    shutil.copytree(
        ROOT / "tools",
        package / "tools",
        ignore=shutil.ignore_patterns("__pycache__", "*.pyc"),
    )


def copy_manual(package):
    repository = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"][
        "repository"
    ].rstrip("/")
    for filename in (
        "LICENSE",
        "site/guide/en/hardware.md",
        "site/guide/ru/hardware.md",
        "site/guide/en/firmware.md",
        "site/guide/ru/firmware.md",
    ):
        destination = package / filename
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / filename, destination)
    shutil.copytree(ROOT / "assets/icons", package / "assets/icons")
    for suffix, locale, introduction, install, source, licenses, agent in (
        (
            "",
            "en",
            "Native macOS app, ESP32-C3 SuperMini firmware and offline manuals. "
            "The enclosure is a separate download.",
            "Install and connect",
            "Source and contributor guide",
            ("Project license", "Third-party notices", "Firmware dependencies"),
            "For installation, updates, lamp setup or service checks, follow the "
            "included [agent runbook](site/guide/en/hardware.md#agent-runbook). "
            "Use its prebuilt-package procedure and completion checks. Preserve "
            "unrelated settings and hooks; flash only when needed and authorized.",
        ),
        (
            ".ru",
            "ru",
            "Нативное приложение macOS, прошивка ESP32-C3 SuperMini и офлайн-инструкции. "
            "Корпус скачивается отдельно.",
            "Установка и подключение",
            "Исходники и руководство для участников",
            (
                "Лицензия проекта",
                "Лицензии сторонних компонентов",
                "Зависимости прошивки",
            ),
            "Для установки, обновления, подключения лампы или проверки службы следуйте "
            "включённой [инструкции для агента](site/guide/ru/hardware.md#agent-runbook). "
            "Используйте процедуру готового пакета и её критерии проверки. Сохраняйте "
            "посторонние настройки и хуки; прошивайте только при необходимости и разрешении.",
        ),
    ):
        languages = "[English](README.md) | [Русский](README.ru.md)\n\n"
        (package / f"README{suffix}.md").write_text(
            languages + "# BroGlow — macOS\n\n" + introduction + "\n\n"
            f"1. [{install}](site/guide/{locale}/hardware.md#install-a-prebuilt-package).\n"
            "2. `BUILD.json` · `SHA256SUMS` · `bin/broglow`.\n"
            f"3. [{source}]({repository}/blob/HEAD/CONTRIBUTING{suffix}.md) · "
            f"[GitHub Releases]({repository}/releases).\n\n"
            f"[{licenses[0]}](LICENSE) · [{licenses[1]}]({notices.NOTICES}) · "
            f"[{licenses[2]}](firmware/esp32-c3-supermini/DEPENDENCIES{suffix}.md).\n"
        )
        (package / f"AGENTS{suffix}.md").write_text(
            "[English](AGENTS.md) | [Русский](AGENTS.ru.md)\n\n"
            "# BroGlow\n\n" + agent + "\n"
        )


FIRMWARE_DEPENDENCIES = {
    "": """# Firmware dependencies

[English](DEPENDENCIES.md) | [Русский](DEPENDENCIES.ru.md)

The prebuilt images in this directory were built from the included `src/main.cpp`
and `platformio.ini` with PlatformIO Core 6.2.0, platform espressif32 7.1.3,
Arduino-ESP32 2.0.17 and its precompiled ESP-IDF v4.4.7 libraries. From the
package root, `uv run flash` detects a connected board and flashes them with
esptool 5.4.0.

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

1. Install [uv](https://docs.astral.sh/uv/). From the package root, run
   `uv run --frozen flash --build-only`. PlatformIO downloads the pinned framework
   to `~/.platformio/packages/framework-arduinoespressif32`.
2. Copy that directory and edit the copy. In `platformio.ini`, add
   `platform_packages = framework-arduinoespressif32 @ symlink:///path/to/copy`
   to `[env:esp32-c3-supermini]`.
3. Run `uv run --frozen flash --build-only` again. The new images are written to
   `.pio/build/esp32-c3-supermini/` in this directory.
4. To flash them, move `flash-manifest.json` out of this directory and run
   `uv run flash`: without the manifest it uploads the source build through
   PlatformIO instead of the prebuilt images.
""",
    ".ru": """# Зависимости прошивки

[English](DEPENDENCIES.md) | [Русский](DEPENDENCIES.ru.md)

Готовые образы в этой папке собраны из включённых `src/main.cpp` и
`platformio.ini` с PlatformIO Core 6.2.0, платформой espressif32 7.1.3,
Arduino-ESP32 2.0.17 и её предкомпилированными библиотеками ESP-IDF v4.4.7.
Из корня пакета `uv run flash` находит подключённую плату и прошивает их
через esptool 5.4.0.

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

1. Установите [uv](https://docs.astral.sh/uv/). Из корня пакета выполните
   `uv run --frozen flash --build-only`. PlatformIO скачает закреплённый фреймворк
   в `~/.platformio/packages/framework-arduinoespressif32`.
2. Скопируйте эту папку и измените копию. В `platformio.ini` добавьте
   `platform_packages = framework-arduinoespressif32 @ symlink:///path/to/copy`
   в `[env:esp32-c3-supermini]`.
3. Снова выполните `uv run --frozen flash --build-only`. Новые образы появятся
   в `.pio/build/esp32-c3-supermini/` в этой папке.
4. Чтобы прошить их, уберите `flash-manifest.json` из этой папки и выполните
   `uv run flash`: без манифеста он загружает сборку из исходников через
   PlatformIO вместо готовых образов.
""",
}


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


def release_identity(version, tag):
    commit = run("git", "rev-parse", "HEAD")
    dirty = bool(run("git", "status", "--porcelain", "--untracked-files=normal"))
    if tag:
        require(
            tag == f"v{version}",
            f"Release tag must equal Cargo.toml version: v{version}",
        )
        require(not dirty, "Release packaging requires a clean source checkout.")
        require(
            run("git", "rev-parse", f"{tag}^{{commit}}") == commit,
            "Release tag must point to the checked-out commit.",
        )
    return commit, dirty


def macho_build_info(load_commands):
    for command in load_commands.split("Load command"):
        if "cmd LC_BUILD_VERSION" in command or "cmd LC_VERSION_MIN_MACOSX" in command:
            minimum = "minos" if "cmd LC_BUILD_VERSION" in command else "version"
            return {
                "minos" if key == minimum else key: value
                for key, value in (
                    line.split()
                    for line in command.splitlines()
                    if len(line.split()) == 2
                )
                if key in (minimum, "sdk")
            }
    return {}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "dist")
    parser.add_argument(
        "--platformio-core", type=Path, default=Path.home() / ".platformio"
    )
    parser.add_argument(
        "--release-tag",
        help="Require a clean matching tag and use a versioned filename.",
    )
    parser.add_argument(
        "--check-only",
        action="store_true",
        help="Only validate --release-tag; no build outputs needed.",
    )
    args = parser.parse_args()
    version = re.search(
        r'^version = "([^"]+)"$', (ROOT / "Cargo.toml").read_text(), re.M
    ).group(1)
    commit, dirty = release_identity(version, args.release_tag)
    if args.check_only:
        require(args.release_tag, "--check-only requires --release-tag.")
        print(f"Release source verified: {args.release_tag} ({commit})")
        return
    require(platform.system() == "Darwin", "Run on the native macOS build machine.")
    arch = platform.machine()
    require(arch in ("arm64", "x86_64"), f"Unsupported Mac architecture: {arch}")
    target = {"arm64": "aarch64", "x86_64": "x86_64"}[arch] + "-apple-darwin"
    binary = ROOT / "target/release/broglow"
    require(binary.is_file(), "First run: cargo build --release --locked")
    require(
        run("/usr/bin/lipo", "-archs", str(binary)) == arch,
        "target/release/broglow must contain only this Mac's native architecture.",
    )
    reported_version = run(str(binary), "--version")
    require(
        reported_version == f"broglow {version}",
        "CLI version differs from Cargo.toml.",
    )
    run(str(binary), "--help")
    require(
        "--port" in run(str(binary), "ble", "pair", "--help"),
        "CLI lacks USB-assisted BLE pairing; rebuild this project first.",
    )
    run(str(binary), "install", "--help")  # Parser smoke test; never installs.
    mac_build = macho_build_info(run("/usr/bin/otool", "-l", str(binary)))
    require(
        "minos" in mac_build, "Could not determine the CLI's minimum macOS version."
    )

    build = C3 / ".pio/build/esp32-c3-supermini"
    framework = args.platformio_core / "packages/framework-arduinoespressif32"
    framework_version = json.loads((framework / "package.json").read_text())["version"]
    require(
        framework_version.startswith("4.20017."),
        "Expected pinned Arduino-ESP32 2.0.17.",
    )
    toolchain = args.platformio_core / "packages/toolchain-riscv32-esp"
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
    for index, (offset, path) in enumerate(segments):
        end = (offset + path.stat().st_size + 4095) // 4096 * 4096
        require(
            end <= 0x9000 or offset >= 0xE000, f"{path.name} would erase NVS sectors."
        )
        require(
            index == len(segments) - 1 or end <= segments[index + 1][0],
            f"{path.name} overlaps the next flash segment.",
        )

    source_files = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    source_files += [
        p
        for folder in ("src", "assets")
        for p in (ROOT / folder).rglob("*")
        if p.is_file()
    ]
    # Match the production build_src_filter; led-test.cpp is a separate image.
    firmware_sources = [C3 / "platformio.ini", C3 / "src/main.cpp"]
    for artifact, sources in (
        (binary, source_files),
        (build / "firmware.bin", firmware_sources),
        (linker_map, firmware_sources),
    ):
        require(
            all(p.stat().st_mtime_ns <= artifact.stat().st_mtime_ns for p in sources),
            f"{artifact.name} predates an input; rebuild before packaging.",
        )

    try:
        firmware_notices, linked = notices.firmware_notices(
            framework, toolchain, linker_map
        )
    except notices.NoticeError as error:
        raise SystemExit(f"Firmware notices: {error}")

    now = datetime.now(timezone.utc)
    name = f"broglow-v{version}-{target}"
    if not args.release_tag:
        name += f"-{now.strftime('%Y%m%dT%H%M%S%fZ')}"
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"{name}.tar.gz"
    require(not archive.exists(), f"Refusing to overwrite existing package: {archive}")
    with tempfile.TemporaryDirectory(prefix=".broglow-package-", dir=output) as temp:
        package = Path(temp) / name
        (package / "bin").mkdir(parents=True)
        shutil.copy2(binary, package / "bin/broglow")
        (package / "bin/broglow").chmod(0o755)
        copy_flash_tool(package)
        destination = package / "firmware/esp32-c3-supermini"
        destination.mkdir(parents=True)
        for _, path in segments:
            shutil.copy2(path, destination / path.name)
        shutil.copy2(C3 / "platformio.ini", destination / "platformio.ini")
        shutil.copytree(C3 / "src", destination / "src")
        shutil.copytree(
            C3 / "tests",
            destination / "tests",
            ignore=shutil.ignore_patterns("__pycache__"),
        )
        write_firmware_notices(destination, framework, firmware_notices)
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
        copy_manual(package)
        try:
            crates, unlicensed = notices.write_rust_notices(package, target)
        except notices.NoticeError as error:
            raise SystemExit(f"Rust notices: {error}")
        provenance = {
            "packaged_at_utc": now.isoformat(),
            "package_version_from_source": version,
            "cli_reported_version": reported_version,
            "target": target,
            "macho_build": mac_build,
            "build_macos": platform.mac_ver()[0],
            "other_architectures_included": False,
            "source_commit_at_packaging": commit,
            "source_dirty_at_packaging": dirty,
            "release_tag": args.release_tag,
            "artifacts_rebuilt_by_packager": False,
            "provenance_note": "Existing builds; input age checks and hashes do not prove reproducibility or test results.",
            "arduino_framework_package": framework_version,
            "third_party_notices": {
                "rust_crates": len(crates),
                "rust_crates_without_license_files": [
                    f"{crate['name']} {crate['version']}" for crate, _ in unlicensed
                ],
                "firmware_linked_archives": len(linked),
            },
            "source_inputs_sha256": {
                str(p.relative_to(ROOT)): sha256(p)
                for p in sorted(source_files + firmware_sources + flash_tool_sources())
            },
        }
        (package / "BUILD.json").write_text(json.dumps(provenance, indent=2) + "\n")
        files = sorted(p for p in package.rglob("*") if p.is_file())
        (package / "SHA256SUMS").write_text(
            "".join(f"{sha256(p)}  {p.relative_to(package)}\n" for p in files)
        )
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(package, arcname=name)
    archive.with_name(archive.name + ".sha256").write_text(
        f"{sha256(archive)}  {archive.name}\n"
    )
    print(archive)
    print(
        "Contains existing native builds only. Read site/guide/en/hardware.md before installation or flashing."
    )


if __name__ == "__main__":
    main()
