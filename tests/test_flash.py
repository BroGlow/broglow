from contextlib import redirect_stderr, redirect_stdout
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock
from types import SimpleNamespace

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


class SelectionTests(unittest.TestCase):
    def test_selects_only_plausible_usb_device(self) -> None:
        bluetooth = port("/dev/cu.Bluetooth", vid=None, pid=None, serial_number=None)
        self.assertEqual(
            flash.select_port([bluetooth, port()], None).device, "/dev/cu.board"
        )

    def test_zero_or_multiple_candidates_print_every_device(self) -> None:
        devices = [
            port("/dev/cu.one", serial_number="one"),
            port("/dev/cu.two", serial_number="two"),
            port("/dev/cu.Bluetooth", vid=None, pid=None, serial_number=None),
        ]
        for selected in (devices[2:], devices):
            output = io.StringIO()
            with (
                redirect_stdout(output),
                self.assertRaisesRegex(flash.FlashError, "exactly one plausible"),
            ):
                flash.select_port(selected, None)
            for device in selected:
                self.assertIn(device.device, output.getvalue())

    def test_explicit_port_must_be_enumerated(self) -> None:
        self.assertEqual(
            flash.select_port([port()], "/dev/cu.board").device, "/dev/cu.board"
        )
        with self.assertRaisesRegex(flash.FlashError, "does not exist"):
            flash.select_port([port()], "/dev/cu.missing")


class HardwareTests(unittest.TestCase):
    def test_identity_requires_exact_protocol_and_service(self) -> None:
        reply = f"broglow_pair identity=146393924524 service={flash.SERVICE}"
        self.assertEqual(flash.parse_identity(reply), "146393924524")
        self.assertIsNone(flash.parse_identity(reply + " extra"))
        self.assertIsNone(flash.parse_identity(reply.replace(flash.SERVICE, "wrong")))

    @mock.patch.object(flash, "Serial")
    def test_identity_probe_never_drains_output(self, serial: mock.Mock) -> None:
        connection = serial.return_value.__enter__.return_value
        connection.flush.side_effect = AssertionError("tcdrain blocks forever")
        connection.readline.return_value = (
            f"broglow_pair identity=146393924524 service={flash.SERVICE}\n".encode()
        )
        self.assertEqual(flash.read_identity("port"), "146393924524")


class BackupTests(unittest.TestCase):
    def test_backup_reads_complete_flash_and_is_private(self) -> None:
        with (
            tempfile.TemporaryDirectory() as folder,
            mock.patch.object(flash, "ROOT", Path(folder)),
            mock.patch.object(flash, "esptool") as tool,
        ):

            def write_backup(_port: str, *arguments: str, **_kwargs: bool) -> None:
                Path(arguments[-1]).write_bytes(b"x" * flash.FLASH_BYTES)

            tool.side_effect = write_backup
            result = flash.backup_flash("port")
            self.assertEqual(result.stat().st_size, flash.FLASH_BYTES)
            self.assertEqual(result.stat().st_mode & 0o777, 0o600)
            self.assertEqual(
                tool.call_args.args[1:6],
                ("--chip", "esp32c3", "read-flash", "0", hex(flash.FLASH_BYTES)),
            )


class FlowTests(unittest.TestCase):
    """Run the CLI with real validation/recovery, faking only external I/O."""

    def setUp(self) -> None:
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.firmware = self.root / "firmware"
        self.firmware.mkdir()
        (self.root / "pyproject.toml").write_text('version = "1.2.3"\n')
        self.enterContext(mock.patch.object(flash, "ROOT", self.root))
        self.enterContext(mock.patch.object(flash, "FIRMWARE", self.firmware))
        self.enterContext(mock.patch.object(flash.Path, "home", return_value=self.root))
        self.enterContext(
            mock.patch.object(flash.shutil, "which", return_value="/bin/broglow")
        )
        self.ports = self.enterContext(
            mock.patch.object(flash.list_ports, "comports", return_value=[port()])
        )
        self.sleep = self.enterContext(mock.patch.object(flash.time, "sleep"))
        self.input = self.enterContext(mock.patch("builtins.input", return_value="yes"))
        self.output = self.enterContext(redirect_stdout(io.StringIO()))
        self.errors = self.enterContext(redirect_stderr(io.StringIO()))
        self.serial = self.enterContext(mock.patch.object(flash, "Serial"))
        self.connection = self.serial.return_value.__enter__.return_value
        self.connection.readline.return_value = (
            f"broglow_pair identity=146393924524 service={flash.SERVICE}\n".encode()
        )
        self.chip = "ESP32-C3"
        self.capacity = "4 MB"
        self.enabled = True
        self.runner = self.enterContext(
            mock.patch.object(flash, "run", side_effect=self.run_external)
        )

        def open_serial(*_args: object, **_kwargs: object) -> mock.Mock:
            self.assertIn(
                mock.call(["/bin/broglow", "ble", "off"]), self.runner.call_args_list
            )
            return self.serial.return_value

        self.serial.side_effect = open_serial

    def run_external(
        self, command: list[str], *, capture: bool = False
    ) -> subprocess.CompletedProcess:
        output = ""
        if command[-2:] == ["ble", "status"]:
            output = (
                "connected: lamp connected"
                if self.enabled
                else "disabled: Lamps disabled"
            )
        elif "chip-id" in command or "flash-id" in command:
            self.assertIn(
                mock.call(["/bin/broglow", "ble", "off"]), self.runner.call_args_list
            )
            output = f"Chip type: {self.chip}\nDetected flash size: {self.capacity}\n"
        elif "read-flash" in command:
            Path(command[-1]).write_bytes(b"x" * flash.FLASH_BYTES)
        return subprocess.CompletedProcess(command, 0, stdout=output)

    def uploads(self) -> list[list[str]]:
        return [
            call.args[0]
            for call in self.runner.call_args_list
            if "upload" in call.args[0] or "write-flash" in call.args[0]
        ]

    def write_manifest(self) -> dict:
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
            (self.firmware / segment["file"]).write_bytes(b"image")
        (self.firmware / "flash-manifest.json").write_text(json.dumps(manifest))
        return manifest

    def test_source_upload_verifies_firmware_and_restores_control(self) -> None:
        result = flash.main(["--port", "/dev/cu.board", "--yes", "--skip-info"])

        self.assertEqual(result, 0, self.errors.getvalue())
        (upload,) = self.uploads()
        self.assertEqual(upload[-2:], ["--upload-port", "/dev/cu.board"])
        self.assertIn("platformio", upload)
        self.serial.assert_called_once_with(
            "/dev/cu.board", 115200, timeout=0.1, write_timeout=1
        )
        self.connection.write.assert_called_with(b"INFO\n")
        self.runner.assert_called_with(["/bin/broglow", "ble", "on"])
        self.assertFalse((self.root / ".recovery").exists())

    def test_prebuilt_upload_uses_validated_offsets_and_keeps_disabled_control(
        self,
    ) -> None:
        self.write_manifest()
        self.enabled = False

        result = flash.main(["--port", "/dev/cu.board", "--yes"])

        self.assertEqual(result, 0, self.errors.getvalue())
        (upload,) = self.uploads()
        self.assertIn("write-flash", upload)
        for offset, filename in (
            ("0x0", "bootloader.bin"),
            ("0x8000", "partitions.bin"),
            ("0xe000", "boot_app0.bin"),
            ("0x10000", "firmware.bin"),
        ):
            self.assertEqual(
                upload[upload.index(offset) + 1], str(self.firmware / filename)
            )
        self.assertNotIn(
            mock.call(["/bin/broglow", "ble", "on"]), self.runner.call_args_list
        )
        self.assertEqual(self.serial.call_count, 2)  # Before and after the write.

    def test_invalid_manifest_aborts_before_accessing_hardware(self) -> None:
        manifest = self.write_manifest()
        manifest["segments"][0]["offset"] = "0x20000"
        (self.firmware / "flash-manifest.json").write_text(json.dumps(manifest))

        result = flash.main(["--port", "/dev/cu.board", "--yes"])

        self.assertEqual(result, 1)
        self.assertIn("four expected images and offsets", self.errors.getvalue())
        self.runner.assert_not_called()
        self.serial.assert_not_called()

    def test_yes_requires_explicit_port(self) -> None:
        self.assertEqual(flash.main(["--yes"]), 1)
        self.assertIn("--yes requires", self.errors.getvalue())
        self.ports.assert_not_called()
        self.runner.assert_not_called()

    def test_rejection_or_eof_never_uploads_and_restores_control(self) -> None:
        for answer in ("no", EOFError()):
            with self.subTest(answer=type(answer).__name__):
                self.input.side_effect = [answer]
                self.runner.reset_mock()

                self.assertEqual(flash.main(["--port", "/dev/cu.board"]), 0)

                self.assertEqual(self.uploads(), [])
                self.runner.assert_called_with(["/bin/broglow", "ble", "on"])
                self.assertFalse((self.root / ".recovery").exists())

    def test_wrong_chip_or_capacity_never_uploads_and_restores_control(self) -> None:
        for chip, capacity, message in (
            ("ESP32-S3", "4 MB", "requires ESP32-C3"),
            ("ESP32-C3", "2 MB", "requires 4 MB"),
        ):
            with self.subTest(chip=chip, capacity=capacity):
                self.chip, self.capacity = chip, capacity
                self.runner.reset_mock()

                self.assertEqual(flash.main(["--port", "/dev/cu.board", "--yes"]), 1)

                self.assertIn(message, self.errors.getvalue())
                self.assertEqual(self.uploads(), [])
                self.runner.assert_called_with(["/bin/broglow", "ble", "on"])

    def test_backup_failure_aborts_upload_and_restores_control(self) -> None:
        def fail_backup(
            command: list[str], *, capture: bool = False
        ) -> subprocess.CompletedProcess:
            if "read-flash" in command:
                raise subprocess.CalledProcessError(1, command, stderr="backup failed")
            return self.run_external(command, capture=capture)

        self.runner.side_effect = fail_backup

        result = flash.main(["--port", "/dev/cu.board", "--yes", "--backup"])

        self.assertEqual(result, 1)
        self.assertIn("backup failed", self.errors.getvalue())
        self.assertEqual(self.uploads(), [])
        self.assertEqual(list((self.root / ".recovery").iterdir()), [])
        self.runner.assert_called_with(["/bin/broglow", "ble", "on"])

    def test_interrupt_during_port_release_restores_control(self) -> None:
        self.sleep.side_effect = KeyboardInterrupt

        self.assertEqual(flash.main(["--port", "/dev/cu.board", "--yes"]), 130)

        self.assertEqual(self.uploads(), [])
        self.serial.assert_not_called()
        self.runner.assert_called_with(["/bin/broglow", "ble", "on"])

    def test_postflash_identity_is_required_even_with_skip_info(self) -> None:
        self.connection.readline.side_effect = OSError("USB disconnected")

        result = flash.main(["--port", "/dev/cu.board", "--yes", "--skip-info"])

        self.assertEqual(result, 1)
        self.assertIn("USB disconnected", self.errors.getvalue())
        self.assertEqual(len(self.uploads()), 1)
        self.runner.assert_called_with(["/bin/broglow", "ble", "on"])

    def test_build_only_never_accesses_hardware(self) -> None:
        self.assertEqual(flash.main(["--build-only"]), 0)

        self.ports.assert_not_called()
        self.serial.assert_not_called()
        command = self.runner.call_args.args[0]
        self.assertIn("platformio", command)
        self.assertNotIn("upload", command)
        self.assertEqual(self.runner.call_count, 1)

    def test_pause_finds_the_documented_install_without_path_setup(self) -> None:
        executable = self.root / ".cargo/bin/broglow"
        executable.parent.mkdir(parents=True)
        executable.touch()
        with mock.patch.object(flash.shutil, "which", return_value=None):
            self.assertEqual(flash.release_serial_port(), str(executable))

        self.runner.assert_called_with([str(executable), "ble", "off"])


if __name__ == "__main__":
    unittest.main()
