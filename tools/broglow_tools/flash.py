"""Build or safely flash the BroGlow ESP32-C3 firmware."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

from serial import Serial
from serial.tools import list_ports

ROOT = Path(__file__).resolve().parents[2]
FIRMWARE = ROOT / "firmware/esp32-c3-supermini"
ENVIRONMENT = "esp32-c3-supermini"
FLASH_BYTES = 0x400000
SERVICE = "7d2ea28a-f7bd-485a-bd9d-92ad6ecfe93e"
IDENTITY = re.compile(rf"^broglow_pair identity=([0-9a-f]{{12}}) service={SERVICE}$")
EXPECTED_SEGMENTS = {
    0x0000: "bootloader.bin",
    0x8000: "partitions.bin",
    0xE000: "boot_app0.bin",
    0x10000: "firmware.bin",
}


class FlashError(RuntimeError):
    pass


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--port", help="serial port to flash")
    result.add_argument(
        "--backup", action="store_true", help="save the complete 4 MB flash first"
    )
    result.add_argument(
        "--skip-info",
        action="store_true",
        help="skip identifying the currently installed firmware",
    )
    result.add_argument(
        "--yes", action="store_true", help="flash without an interactive prompt"
    )
    result.add_argument(
        "--build-only",
        action="store_true",
        help="build the firmware without accessing serial hardware",
    )
    return result


def run(command: list[str], *, capture: bool = False) -> subprocess.CompletedProcess:
    return subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        capture_output=capture,
        check=True,
    )


def build_firmware() -> None:
    run(
        [
            sys.executable,
            "-m",
            "platformio",
            "run",
            "-d",
            str(FIRMWARE),
            "-e",
            ENVIRONMENT,
        ]
    )


def plausible_ports(ports) -> list:
    usb = [port for port in ports if port.vid is not None and port.pid is not None]
    names = {port.device for port in usb}
    return [
        port
        for port in usb
        if not (
            port.device.startswith("/dev/tty.")
            and port.device.replace("/dev/tty.", "/dev/cu.", 1) in names
        )
    ]


def print_ports(ports) -> None:
    rows = [
        (
            port.device,
            port.description or "-",
            (
                f"{port.vid:04x}:{port.pid:04x}"
                if port.vid is not None and port.pid is not None
                else "-"
            ),
            port.serial_number or "-",
        )
        for port in ports
    ]
    if not rows:
        print("Detected serial devices: (none)")
        return
    headers = ("PORT", "DESCRIPTION", "VID:PID", "SERIAL")
    widths = [max(len(row[index]) for row in [headers, *rows]) for index in range(4)]
    print("Detected serial devices:")
    for row in (headers, *rows):
        print("  " + "  ".join(value.ljust(width) for value, width in zip(row, widths)))


def select_port(ports, explicit: str | None):
    if explicit:
        for port in ports:
            if port.device == explicit:
                return port
        print_ports(ports)
        raise FlashError(f"Serial port does not exist: {explicit}")
    candidates = plausible_ports(ports)
    if len(candidates) != 1:
        print_ports(ports)
        raise FlashError(
            f"Expected exactly one plausible USB serial board, found {len(candidates)}; use --port."
        )
    return candidates[0]


def parse_identity(line: str) -> str | None:
    match = IDENTITY.fullmatch(line.strip())
    return match.group(1) if match else None


def read_identity(port: str, timeout: float = 5) -> str:
    deadline = time.monotonic() + timeout
    with Serial(port, 115200, timeout=0.1, write_timeout=1) as connection:
        connection.dtr = True
        connection.reset_input_buffer()
        while time.monotonic() < deadline:
            connection.write(b"INFO\n")
            connection.flush()
            reply_deadline = min(deadline, time.monotonic() + 0.5)
            while time.monotonic() < reply_deadline:
                line = connection.readline(257)
                if len(line) > 256:
                    raise FlashError("Unexpected oversized USB reply.")
                identity = parse_identity(line.decode("utf-8", errors="replace"))
                if identity:
                    return identity
    raise FlashError("No exact BroGlow INFO response received.")


def inspect_current_firmware(port: str, skip: bool) -> str:
    if skip:
        print("Skipped current-firmware identification by request.")
        return "skipped"
    try:
        identity = read_identity(port)
    except Exception as error:
        print(f"Warning: current firmware not recognized ({error}).")
        return "not recognized"
    print(f"BroGlow firmware detected (identity {identity}).")
    return f"BroGlow {identity}"


def esptool(port: str, *arguments: str, capture: bool = False):
    command = [sys.executable, "-m", "esptool", "--port", port, *arguments]
    try:
        return run(command, capture=capture)
    except subprocess.CalledProcessError as error:
        output = "\n".join(
            part for part in (error.stdout, error.stderr) if part
        ).strip()
        raise FlashError(
            f"esptool failed{f': {output}' if output else '.'}\n"
            "Hold BOOT, tap RESET, release BOOT, and check that the USB cable carries data."
        ) from error


def verify_hardware(port: str) -> tuple[str, str]:
    chip_output = esptool(port, "chip-id", capture=True).stdout
    match = re.search(r"(?:Chip type:|Chip is)\s*(ESP32-[A-Z0-9-]+)", chip_output)
    if not match:
        raise FlashError("Could not determine the connected chip from esptool output.")
    chip = match.group(1)
    if chip != "ESP32-C3":
        raise FlashError(f"Unsupported chip {chip}; this firmware requires ESP32-C3.")

    flash_output = esptool(port, "--chip", "esp32c3", "flash-id", capture=True).stdout
    match = re.search(r"Detected flash size:\s*([0-9]+)\s*MB", flash_output)
    if not match:
        raise FlashError("Could not determine flash capacity from esptool output.")
    size = f"{match.group(1)} MB"
    if match.group(1) != "4":
        raise FlashError(f"Unsupported flash size {size}; this firmware requires 4 MB.")
    return chip, size


def backup_flash(port: str) -> Path:
    recovery = ROOT / ".recovery"
    recovery.mkdir(mode=0o700, exist_ok=True)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    destination = recovery / f"c3-before-flash-{stamp}.bin"
    partial = destination.with_suffix(".bin.partial")
    descriptor = os.open(partial, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    os.close(descriptor)
    print("Backup may contain Bluetooth keys; do not share or commit it.")
    try:
        esptool(
            port, "--chip", "esp32c3", "read-flash", "0", hex(FLASH_BYTES), str(partial)
        )
        if partial.stat().st_size != FLASH_BYTES:
            raise FlashError(
                f"Backup has {partial.stat().st_size} bytes; expected {FLASH_BYTES}."
            )
        os.chmod(partial, 0o600)
        partial.replace(destination)
    except Exception:
        partial.unlink(missing_ok=True)
        raise
    digest = hashlib.sha256(destination.read_bytes()).hexdigest()
    print(f"Backup: {destination} (SHA-256 {digest})")
    return destination


def load_manifest() -> tuple[dict, list[tuple[int, Path]]] | None:
    path = FIRMWARE / "flash-manifest.json"
    if not path.is_file():
        return None
    try:
        manifest = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise FlashError(f"Invalid flash manifest: {error}") from error
    expected = {
        "chip": "esp32c3",
        "flash_mode": "dio",
        "flash_freq": "40m",
        "flash_size": "4MB",
    }
    for field, value in expected.items():
        if manifest.get(field) != value:
            raise FlashError(f"Invalid flash manifest {field}: expected {value}.")
    try:
        segments = [
            (int(segment["offset"], 0), FIRMWARE / segment["file"])
            for segment in manifest["segments"]
        ]
    except (KeyError, TypeError, ValueError) as error:
        raise FlashError(f"Invalid flash manifest segments: {error}") from error
    if (
        len(segments) != len(EXPECTED_SEGMENTS)
        or {offset: path.name for offset, path in segments} != EXPECTED_SEGMENTS
    ):
        raise FlashError(
            "Flash manifest must contain the four expected images and offsets."
        )
    if any(path.parent != FIRMWARE or not path.is_file() for _, path in segments):
        raise FlashError(
            "A flash-manifest image is missing or outside the firmware directory."
        )
    return manifest, sorted(segments)


def flash_prebuilt(port: str, manifest: dict, segments: list[tuple[int, Path]]) -> None:
    arguments = [
        "--chip",
        manifest["chip"],
        "--baud",
        "460800",
        "--before",
        "default-reset",
        "--after",
        "hard-reset",
        "write-flash",
        "--flash-mode",
        manifest["flash_mode"],
        "--flash-freq",
        manifest["flash_freq"],
        "--flash-size",
        manifest["flash_size"],
    ]
    for offset, path in segments:
        arguments.extend((hex(offset), str(path)))
    esptool(port, *arguments)


def release_serial_port() -> str | None:
    executable = shutil.which("broglow")
    if not executable:
        installed = Path.home() / ".cargo/bin/broglow"
        if installed.is_file():
            executable = str(installed)
    if executable:
        enabled = not run(
            [executable, "ble", "status"], capture=True
        ).stdout.startswith("disabled:")
        run([executable, "ble", "off"])
        return executable if enabled else None
    return None


def flash_source(port: str) -> None:
    try:
        run(
            [
                sys.executable,
                "-m",
                "platformio",
                "run",
                "-d",
                str(FIRMWARE),
                "-e",
                ENVIRONMENT,
                "-t",
                "upload",
                "--upload-port",
                port,
            ]
        )
    except subprocess.CalledProcessError as error:
        raise FlashError(
            "PlatformIO upload failed. If it could not connect, hold BOOT, tap RESET, "
            "release BOOT, and check that the USB cable carries data."
        ) from error


def same_device(original, candidate) -> bool:
    if original.serial_number:
        return candidate.serial_number == original.serial_number
    if candidate.device == original.device:
        return True
    return (
        original.vid is not None
        and original.pid is not None
        and candidate.vid == original.vid
        and candidate.pid == original.pid
    )


def wait_for_port(original, timeout: float = 20) -> str:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        matches = [
            port for port in list_ports.comports() if same_device(original, port)
        ]
        matches = plausible_ports(matches) or matches
        if len(matches) == 1:
            return matches[0].device
        time.sleep(0.25)
    raise FlashError(
        "The flashed board did not return as one identifiable serial device."
    )


def confirm() -> bool:
    try:
        return input("Flash this board? [y/N] ").strip().lower() in {"y", "yes"}
    except EOFError:
        return False


def version() -> str:
    match = re.search(
        r'^version = "([^"]+)"$', (ROOT / "pyproject.toml").read_text(), re.M
    )
    return match.group(1) if match else "unknown"


def execute(args) -> int:
    if args.build_only:
        build_firmware()
        print(f"Built firmware environment {ENVIRONMENT}.")
        return 0
    if args.yes and not args.port:
        raise FlashError("--yes requires an explicit --port.")

    ports = list(list_ports.comports())
    selected = select_port(ports, args.port)
    prebuilt = load_manifest()
    restore = release_serial_port()
    failed = True
    try:
        # Probes and backups also need exclusive USB access. Keep the wait inside
        # the restoration guard so Ctrl-C cannot leave lamp control disabled.
        time.sleep(3)
        info = inspect_current_firmware(selected.device, args.skip_info)
        chip, flash_size = verify_hardware(selected.device)

        if args.backup:
            backup = str(backup_flash(selected.device))
        else:
            backup = "no backup"
            print(
                "No backup requested. Use --backup to save the complete existing flash."
            )

        mode = "prebuilt release images" if prebuilt else "source PlatformIO upload"
        print("\nReady to flash:")
        print(f"  port: {selected.device}")
        print(f"  chip: {chip}")
        print(f"  flash: {flash_size}")
        print(f"  path: {mode}")
        print(f"  current firmware: {info}")
        print(f"  backup: {backup}")
        print(f"  target: BroGlow {version()} / {ENVIRONMENT}")
        if not args.yes and not confirm():
            print("Flash cancelled; nothing was written.")
            failed = False
            return 0

        sys.stdout.flush()
        if prebuilt:
            flash_prebuilt(selected.device, *prebuilt)
        else:
            flash_source(selected.device)

        returned = wait_for_port(selected)
        identity = read_identity(returned, timeout=10)
        print(
            f"Flashed and verified BroGlow firmware (identity {identity}) on {returned}."
        )
        failed = False
    finally:
        if restore:
            try:
                run([restore, "ble", "on"])
            except Exception as error:
                if not failed:
                    raise
                print(
                    f"Warning: could not restore lamp control: {error}", file=sys.stderr
                )
    if restore:
        print("Restored lamp control.")
    else:
        print(f"Next: broglow ble pair --port {returned}")
    return 0


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        return execute(args)
    except KeyboardInterrupt:
        print("\nFlash interrupted.", file=sys.stderr)
        return 130
    except (FlashError, OSError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
