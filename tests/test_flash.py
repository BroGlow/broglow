from collections.abc import Callable
from dataclasses import dataclass
import json
from pathlib import Path
import subprocess
from types import SimpleNamespace
from unittest.mock import MagicMock, Mock, call

import pytest

from broglow_tools import flash


def port(
    device: str = "/dev/cu.board",
    *,
    vid: int | None = 0x303A,
    pid: int | None = 0x1001,
    serial_number: str | None = "001122334455",
    description: str = "USB board",
) -> SimpleNamespace:
    return SimpleNamespace(
        device=device,
        vid=vid,
        pid=pid,
        serial_number=serial_number,
        description=description,
    )


def test_selects_only_plausible_usb_device() -> None:
    bluetooth = port("/dev/cu.Bluetooth", vid=None, pid=None, serial_number=None)

    selected = flash.select_port([bluetooth, port()], None)

    assert selected.device == "/dev/cu.board"


@pytest.mark.parametrize(
    "devices",
    [
        pytest.param(
            [port("/dev/cu.Bluetooth", vid=None, pid=None, serial_number=None)],
            id="no-usb-board",
        ),
        pytest.param(
            [
                port("/dev/cu.one", serial_number="one"),
                port("/dev/cu.two", serial_number="two"),
                port("/dev/cu.Bluetooth", vid=None, pid=None, serial_number=None),
            ],
            id="multiple-usb-boards",
        ),
    ],
)
def test_ambiguous_selection_prints_every_device(
    devices: list[SimpleNamespace], capsys: pytest.CaptureFixture[str]
) -> None:
    with pytest.raises(flash.FlashError, match="exactly one plausible"):
        flash.select_port(devices, None)

    output = capsys.readouterr().out
    for device in devices:
        assert device.device in output


def test_explicit_port_must_be_enumerated() -> None:
    assert flash.select_port([port()], "/dev/cu.board").device == "/dev/cu.board"
    with pytest.raises(flash.FlashError, match="does not exist"):
        flash.select_port([port()], "/dev/cu.missing")


@pytest.mark.parametrize(
    ("reply", "expected"),
    [
        pytest.param(
            f"broglow_pair identity=146393924524 service={flash.SERVICE}",
            "146393924524",
            id="broglow-identity",
        ),
        pytest.param(
            f"broglow_pair identity=146393924524 service={flash.SERVICE} extra",
            None,
            id="extra-fields",
        ),
        pytest.param(
            "broglow_pair identity=146393924524 service=wrong",
            None,
            id="foreign-service",
        ),
    ],
)
def test_identity_requires_exact_protocol_and_service(
    reply: str, expected: str | None
) -> None:
    assert flash.parse_identity(reply) == expected


@pytest.fixture
def serial_port(monkeypatch: pytest.MonkeyPatch) -> MagicMock:
    serial = MagicMock()
    connection = serial.return_value.__enter__.return_value
    connection.readline.return_value = (
        f"broglow_pair identity=146393924524 service={flash.SERVICE}\n".encode()
    )
    monkeypatch.setattr(flash, "Serial", serial)
    return serial


def test_identity_probe_never_drains_output(serial_port: MagicMock) -> None:
    connection = serial_port.return_value.__enter__.return_value
    connection.flush.side_effect = AssertionError("tcdrain blocks forever")

    identity = flash.read_identity("port")

    assert identity == "146393924524"


def test_backup_reads_complete_flash_and_is_private(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    def write_backup(_port: str, *arguments: str, **_kwargs: bool) -> None:
        Path(arguments[-1]).write_bytes(b"x" * flash.FLASH_BYTES)

    tool = Mock(side_effect=write_backup)
    monkeypatch.setattr(flash, "ROOT", tmp_path)
    monkeypatch.setattr(flash, "esptool", tool)

    result = flash.backup_flash("port")

    assert result.stat().st_size == flash.FLASH_BYTES
    assert result.stat().st_mode & 0o777 == 0o600
    assert tool.call_args.args[1:6] == (
        "--chip",
        "esp32c3",
        "read-flash",
        "0",
        hex(flash.FLASH_BYTES),
    )


@dataclass
class Hardware:
    chip: str = "ESP32-C3"
    capacity: str = "4 MB"
    enabled: bool = True


@pytest.fixture
def hardware() -> Hardware:
    return Hardware()


@pytest.fixture
def firmware_workspace(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    firmware = tmp_path / "firmware"
    firmware.mkdir()
    (tmp_path / "pyproject.toml").write_text('version = "1.2.3"\n')
    monkeypatch.setattr(flash, "ROOT", tmp_path)
    monkeypatch.setattr(flash, "FIRMWARE", firmware)
    monkeypatch.setattr(flash.Path, "home", lambda: tmp_path)
    return firmware


@pytest.fixture
def external_runner(monkeypatch: pytest.MonkeyPatch, hardware: Hardware) -> Mock:
    def run_external(
        command: list[str], *, capture: bool = False
    ) -> subprocess.CompletedProcess[str]:
        output = ""
        if command[-2:] == ["ble", "status"]:
            output = (
                "connected: lamp connected"
                if hardware.enabled
                else "disabled: Lamps disabled"
            )
        elif "chip-id" in command or "flash-id" in command:
            assert call(["/bin/broglow", "ble", "off"]) in runner.call_args_list
            output = f"Chip type: {hardware.chip}\nDetected flash size: {hardware.capacity}\n"
        elif "read-flash" in command:
            Path(command[-1]).write_bytes(b"x" * flash.FLASH_BYTES)
        return subprocess.CompletedProcess(command, 0, stdout=output)

    runner = Mock(side_effect=run_external)
    monkeypatch.setattr(flash, "run", runner)
    monkeypatch.setattr(flash.shutil, "which", lambda _: "/bin/broglow")
    return runner


@pytest.fixture
def available_ports(monkeypatch: pytest.MonkeyPatch) -> Mock:
    enumerate_ports = Mock(return_value=[port()])
    monkeypatch.setattr(flash.list_ports, "comports", enumerate_ports)
    return enumerate_ports


@pytest.fixture
def confirmation(monkeypatch: pytest.MonkeyPatch) -> Mock:
    prompt = Mock(return_value="yes")
    monkeypatch.setattr("builtins.input", prompt)
    return prompt


@pytest.fixture
def delay(monkeypatch: pytest.MonkeyPatch) -> Mock:
    sleep = Mock()
    monkeypatch.setattr(flash.time, "sleep", sleep)
    return sleep


@pytest.fixture
def flash_cli(
    firmware_workspace: Path,
    external_runner: Mock,
    serial_port: MagicMock,
    available_ports: Mock,
    confirmation: Mock,
    delay: Mock,
) -> Callable[[list[str]], int]:
    """Use the real CLI, validation and recovery with only external I/O replaced."""

    def open_serial(*_args: object, **_kwargs: object) -> MagicMock:
        assert call(["/bin/broglow", "ble", "off"]) in external_runner.call_args_list
        return serial_port.return_value

    serial_port.side_effect = open_serial
    return flash.main


@pytest.fixture
def prebuilt_firmware(firmware_workspace: Path) -> Path:
    manifest = {
        "chip": "esp32c3",
        "flash_mode": "dio",
        "flash_freq": "40m",
        "flash_size": "4MB",
        "segments": [
            {"offset": "0x10000", "file": "firmware.bin"},
            {"offset": "0x0", "file": "bootloader.bin"},
            {"offset": "0xe000", "file": "boot_app0.bin"},
            {"offset": "0x8000", "file": "partitions.bin"},
        ],
    }
    for segment in manifest["segments"]:
        (firmware_workspace / segment["file"]).write_bytes(b"image")
    (firmware_workspace / "flash-manifest.json").write_text(json.dumps(manifest))
    return firmware_workspace


def upload_commands(runner: Mock) -> list[list[str]]:
    return [
        invocation.args[0]
        for invocation in runner.call_args_list
        if "upload" in invocation.args[0] or "write-flash" in invocation.args[0]
    ]


def test_source_upload_verifies_firmware_and_restores_control(
    flash_cli: Callable[[list[str]], int],
    external_runner: Mock,
    serial_port: MagicMock,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    result = flash_cli(["--port", "/dev/cu.board", "--yes", "--skip-info"])

    assert result == 0, capsys.readouterr().err
    (upload,) = upload_commands(external_runner)
    assert upload[-2:] == ["--upload-port", "/dev/cu.board"]
    assert "platformio" in upload
    serial_port.assert_called_once_with(
        "/dev/cu.board", 115200, timeout=0.1, write_timeout=1
    )
    serial_port.return_value.__enter__.return_value.write.assert_called_with(b"INFO\n")
    external_runner.assert_called_with(["/bin/broglow", "ble", "on"])
    assert not (tmp_path / ".recovery").exists()


def test_prebuilt_upload_uses_validated_offsets_and_keeps_disabled_control(
    flash_cli: Callable[[list[str]], int],
    prebuilt_firmware: Path,
    hardware: Hardware,
    external_runner: Mock,
    serial_port: MagicMock,
    capsys: pytest.CaptureFixture[str],
) -> None:
    hardware.enabled = False

    result = flash_cli(["--port", "/dev/cu.board", "--yes"])

    assert result == 0, capsys.readouterr().err
    (upload,) = upload_commands(external_runner)
    assert "write-flash" in upload
    for offset, filename in (
        ("0x0", "bootloader.bin"),
        ("0x8000", "partitions.bin"),
        ("0xe000", "boot_app0.bin"),
        ("0x10000", "firmware.bin"),
    ):
        assert upload[upload.index(offset) + 1] == str(prebuilt_firmware / filename)
    assert call(["/bin/broglow", "ble", "on"]) not in external_runner.call_args_list
    assert serial_port.call_count == 2  # Before and after the write.


def test_invalid_manifest_aborts_before_accessing_hardware(
    flash_cli: Callable[[list[str]], int],
    prebuilt_firmware: Path,
    external_runner: Mock,
    serial_port: MagicMock,
    capsys: pytest.CaptureFixture[str],
) -> None:
    path = prebuilt_firmware / "flash-manifest.json"
    manifest = json.loads(path.read_text())
    manifest["segments"][0]["offset"] = "0x20000"
    path.write_text(json.dumps(manifest))

    result = flash_cli(["--port", "/dev/cu.board", "--yes"])

    assert result == 1
    assert "four expected images and offsets" in capsys.readouterr().err
    external_runner.assert_not_called()
    serial_port.assert_not_called()


def test_yes_requires_explicit_port(
    flash_cli: Callable[[list[str]], int],
    available_ports: Mock,
    external_runner: Mock,
    capsys: pytest.CaptureFixture[str],
) -> None:
    result = flash_cli(["--yes"])

    assert result == 1
    assert "--yes requires" in capsys.readouterr().err
    available_ports.assert_not_called()
    external_runner.assert_not_called()


@pytest.mark.parametrize(
    "answer",
    [pytest.param("no", id="rejected"), pytest.param(EOFError(), id="closed-stdin")],
)
def test_rejection_or_eof_never_uploads_and_restores_control(
    flash_cli: Callable[[list[str]], int],
    confirmation: Mock,
    external_runner: Mock,
    tmp_path: Path,
    answer: str | EOFError,
) -> None:
    confirmation.side_effect = [answer]

    result = flash_cli(["--port", "/dev/cu.board"])

    assert result == 0
    assert upload_commands(external_runner) == []
    external_runner.assert_called_with(["/bin/broglow", "ble", "on"])
    assert not (tmp_path / ".recovery").exists()


@pytest.mark.parametrize(
    ("chip", "capacity", "message"),
    [
        pytest.param("ESP32-S3", "4 MB", "requires ESP32-C3", id="wrong-chip"),
        pytest.param("ESP32-C3", "2 MB", "requires 4 MB", id="wrong-capacity"),
    ],
)
def test_wrong_chip_or_capacity_never_uploads_and_restores_control(
    flash_cli: Callable[[list[str]], int],
    hardware: Hardware,
    external_runner: Mock,
    capsys: pytest.CaptureFixture[str],
    chip: str,
    capacity: str,
    message: str,
) -> None:
    hardware.chip, hardware.capacity = chip, capacity

    result = flash_cli(["--port", "/dev/cu.board", "--yes"])

    assert result == 1
    assert message in capsys.readouterr().err
    assert upload_commands(external_runner) == []
    external_runner.assert_called_with(["/bin/broglow", "ble", "on"])


def test_backup_failure_aborts_upload_and_restores_control(
    flash_cli: Callable[[list[str]], int],
    external_runner: Mock,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    run_external = external_runner.side_effect

    def fail_backup(
        command: list[str], *, capture: bool = False
    ) -> subprocess.CompletedProcess[str]:
        if "read-flash" in command:
            raise subprocess.CalledProcessError(1, command, stderr="backup failed")
        return run_external(command, capture=capture)

    external_runner.side_effect = fail_backup

    result = flash_cli(["--port", "/dev/cu.board", "--yes", "--backup"])

    assert result == 1
    assert "backup failed" in capsys.readouterr().err
    assert upload_commands(external_runner) == []
    assert list((tmp_path / ".recovery").iterdir()) == []
    external_runner.assert_called_with(["/bin/broglow", "ble", "on"])


def test_interrupt_during_port_release_restores_control(
    flash_cli: Callable[[list[str]], int],
    delay: Mock,
    external_runner: Mock,
    serial_port: MagicMock,
) -> None:
    delay.side_effect = KeyboardInterrupt

    result = flash_cli(["--port", "/dev/cu.board", "--yes"])

    assert result == 130
    assert upload_commands(external_runner) == []
    serial_port.assert_not_called()
    external_runner.assert_called_with(["/bin/broglow", "ble", "on"])


def test_postflash_identity_is_required_even_with_skip_info(
    flash_cli: Callable[[list[str]], int],
    serial_port: MagicMock,
    external_runner: Mock,
    capsys: pytest.CaptureFixture[str],
) -> None:
    serial_port.return_value.__enter__.return_value.readline.side_effect = OSError(
        "USB disconnected"
    )

    result = flash_cli(["--port", "/dev/cu.board", "--yes", "--skip-info"])

    assert result == 1
    assert "USB disconnected" in capsys.readouterr().err
    assert len(upload_commands(external_runner)) == 1
    external_runner.assert_called_with(["/bin/broglow", "ble", "on"])


def test_build_only_never_accesses_hardware(
    flash_cli: Callable[[list[str]], int],
    available_ports: Mock,
    serial_port: MagicMock,
    external_runner: Mock,
) -> None:
    result = flash_cli(["--build-only"])

    assert result == 0
    available_ports.assert_not_called()
    serial_port.assert_not_called()
    command = external_runner.call_args.args[0]
    assert "platformio" in command
    assert "upload" not in command
    assert external_runner.call_count == 1


@pytest.mark.usefixtures("firmware_workspace")
def test_pause_finds_the_documented_install_without_path_setup(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, external_runner: Mock
) -> None:
    executable = tmp_path / ".cargo/bin/broglow"
    executable.parent.mkdir(parents=True)
    executable.touch()
    monkeypatch.setattr(flash.shutil, "which", lambda _: None)

    restore = flash.release_serial_port()

    assert restore == str(executable)
    external_runner.assert_called_with([str(executable), "ble", "off"])
