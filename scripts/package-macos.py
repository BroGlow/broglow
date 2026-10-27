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
_firmware_spec = importlib.util.spec_from_file_location(
    "package_firmware", ROOT / "scripts/package-firmware.py"
)
firmware = importlib.util.module_from_spec(_firmware_spec)
_firmware_spec.loader.exec_module(firmware)


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


def repository_url():
    """The planned public repository, set once in Cargo.toml."""
    return tomllib.loads((ROOT / "Cargo.toml").read_text())["package"][
        "repository"
    ].rstrip("/")


def resolve_repo_url(path):
    """Replace the docs' {{ repo_url }} token, as the website build does."""
    path.write_text(path.read_text().replace("{{ repo_url }}", repository_url()))


def copy_manual(package):
    repository = repository_url()
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
        if destination.suffix == ".md":
            resolve_repo_url(destination)
    # The packaged manual embeds the tray icons by relative path.
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

    source_files = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    source_files += [
        p
        for folder in ("src", "assets")
        for p in (ROOT / folder).rglob("*")
        if p.is_file()
    ]
    require(
        all(p.stat().st_mtime_ns <= binary.stat().st_mtime_ns for p in source_files),
        f"{binary.name} predates an input; rebuild before packaging.",
    )

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
        framework_version, linked = firmware.package_firmware(
            package / "firmware/esp32-c3-supermini",
            args.platformio_core,
            firmware.firmware_version(),
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
            "firmware_version": firmware.firmware_version(),
            "firmware_version": firmware.firmware_version(),
            "third_party_notices": {
                "rust_crates": len(crates),
                "rust_crates_without_license_files": [
                    f"{crate['name']} {crate['version']}" for crate, _ in unlicensed
                ],
                "firmware_linked_archives": len(linked),
            },
            "source_inputs_sha256": {
                str(p.relative_to(ROOT)): sha256(p)
                for p in sorted(source_files + firmware.firmware_sources())
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
