#!/usr/bin/env python3
"""Package existing native macOS/C3 builds; never build, install, or touch hardware."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
C3 = ROOT / "firmware/esp32-c3-supermini"


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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "dist")
    parser.add_argument("--platformio-core", type=Path, default=Path.home() / ".platformio")
    args = parser.parse_args()
    require(platform.system() == "Darwin", "Run on the native macOS build machine.")
    arch = platform.machine()
    require(arch in ("arm64", "x86_64"), f"Unsupported Mac architecture: {arch}")
    target = {"arm64": "aarch64", "x86_64": "x86_64"}[arch] + "-apple-darwin"
    binary = ROOT / "target/release/clawlight"
    require(binary.is_file(), "First run: cargo build --release --locked")
    require(run("/usr/bin/lipo", "-archs", str(binary)) == arch,
            "target/release/clawlight must contain only this Mac's native architecture.")
    version = re.search(r'^version = "([^"]+)"$', (ROOT / "Cargo.toml").read_text(), re.M).group(1)
    reported = subprocess.run([str(binary), "--version"], capture_output=True, text=True)
    reported_version = reported.stdout.strip() if reported.returncode == 0 else None
    require(reported_version in (None, f"clawlight {version}"), "CLI version differs from Cargo.toml.")
    run(str(binary), "--help")
    require("--port" in run(str(binary), "ble", "pair", "--help"),
            "CLI lacks USB-assisted BLE pairing; rebuild this fork first.")
    run(str(binary), "install", "--help") # Parser smoke test; never installs.
    mac_build = {}
    for command in run("/usr/bin/otool", "-l", str(binary)).split("Load command"):
        if "cmd LC_BUILD_VERSION" in command:
            mac_build = {key: value for key, value in
                         (line.split() for line in command.splitlines() if len(line.split()) == 2)
                         if key in ("minos", "sdk")}

    build = C3 / ".pio/build/esp32-c3-supermini"
    framework = args.platformio_core / "packages/framework-arduinoespressif32"
    framework_version = json.loads((framework / "package.json").read_text())["version"]
    require(framework_version.startswith("3.20017."), "Expected pinned Arduino-ESP32 2.0.17.")
    segments = [
        (0x0000, build / "bootloader.bin"),
        (0x8000, build / "partitions.bin"),
        (0xE000, framework / "tools/partitions/boot_app0.bin"),
        (0x10000, build / "firmware.bin"),
    ]
    for _, path in segments:
        require(path.is_file(), f"Missing {path}; build the pinned PlatformIO project first.")
    # Verify the actual partition table, not a guess from the board model.
    table = (build / "partitions.bin").read_bytes()
    partitions = {}
    for offset in range(0, len(table), 32):
        magic, kind, subtype, start, size, label, _ = struct.unpack_from("<HBBII16sI", table, offset)
        if magic != 0x50AA:
            break
        partitions[label.rstrip(b"\0").decode()] = (kind, subtype, start, size)
    require(partitions.get("nvs") == (1, 2, 0x9000, 0x5000) and
            partitions.get("otadata") == (1, 0, 0xE000, 0x2000) and
            partitions.get("app0") == (0, 0x10, 0x10000, 0x140000),
            "Partition layout changed; review flash offsets and NVS preservation before packaging.")
    require((build / "firmware.bin").stat().st_size <= 0x140000, "Application exceeds app0.")
    for index, (offset, path) in enumerate(segments):
        end = (offset + path.stat().st_size + 4095) // 4096 * 4096
        require(end <= 0x9000 or offset >= 0xE000, f"{path.name} would erase NVS sectors.")
        require(index == len(segments) - 1 or end <= segments[index + 1][0],
                f"{path.name} overlaps the next flash segment.")

    source_files = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    source_files += [p for folder in ("src", "assets") for p in (ROOT / folder).rglob("*") if p.is_file()]
    # Match the production build_src_filter; led-test.cpp is a separate image.
    firmware_sources = [C3 / "platformio.ini", C3 / "src/main.cpp"]
    for artifact, sources in ((binary, source_files), (build / "firmware.bin", firmware_sources)):
        require(all(p.stat().st_mtime_ns <= artifact.stat().st_mtime_ns for p in sources),
                f"{artifact.name} predates an input; rebuild before packaging.")

    now = datetime.now(timezone.utc)
    name = f"clawlight-v{version}-{target}-{now.strftime('%Y%m%dT%H%M%S%fZ')}"
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"{name}.tar.gz"
    with tempfile.TemporaryDirectory(prefix=".clawlight-package-", dir=output) as temp:
        package = Path(temp) / name
        (package / "bin").mkdir(parents=True)
        shutil.copy2(binary, package / "bin/clawlight")
        (package / "bin/clawlight").chmod(0o755)
        destination = package / "firmware/esp32-c3-supermini"
        destination.mkdir(parents=True)
        for _, path in segments:
            shutil.copy2(path, destination / path.name)
        manifest = {
            "chip": "esp32c3", "flash_mode": "dio", "flash_freq": "40m", "flash_size": "4MB",
            "segments": [{"offset": hex(offset), "file": path.name} for offset, path in segments],
            "preserved_nvs": {"offset": "0x9000", "size": "0x5000"},
            "requires_matching_existing_partition_layout": True,
        }
        (destination / "flash-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        for filename in ("README.md", "LICENSE", "CLAUDE.md", "AGENTS.md"):
            if (ROOT / filename).is_file():
                shutil.copy2(ROOT / filename, package / filename)
        shutil.copytree(ROOT / "docs", package / "docs",
                        ignore=shutil.ignore_patterns("RELEASE.md", ".DS_Store"))
        shutil.copytree(ROOT / "assets/icons", package / "assets/icons")
        provenance = {
            "packaged_at_utc": now.isoformat(), "package_version_from_source": version,
            "cli_reported_version": reported_version, "target": target, "macho_build": mac_build,
            "build_macos": platform.mac_ver()[0],
            "other_architectures_included": False,
            "source_commit_at_packaging": run("git", "rev-parse", "HEAD"),
            "source_dirty_at_packaging": bool(run("git", "status", "--porcelain", "--untracked-files=normal")),
            "artifacts_rebuilt_by_packager": False,
            "provenance_note": "Existing builds; input age checks and hashes do not prove reproducibility or test results.",
            "arduino_framework_package": framework_version,
            "source_inputs_sha256": {str(p.relative_to(ROOT)): sha256(p) for p in sorted(source_files + firmware_sources)},
        }
        (package / "BUILD.json").write_text(json.dumps(provenance, indent=2) + "\n")
        files = sorted(p for p in package.rglob("*") if p.is_file())
        (package / "SHA256SUMS").write_text("".join(f"{sha256(p)}  {p.relative_to(package)}\n" for p in files))
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(package, arcname=name)
    archive.with_name(archive.name + ".sha256").write_text(f"{sha256(archive)}  {archive.name}\n")
    print(archive)
    print("Contains existing native builds only. Read docs/hardware-setup.md before installation or flashing.")


if __name__ == "__main__":
    main()
