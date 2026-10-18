from contextlib import redirect_stdout
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
    device="/dev/cu.board",
    *,
    vid=0x303A,
    pid=0x1001,
    serial_number="001122334455",
    description="USB board",
):
    return SimpleNamespace(
        device=device,
        vid=vid,
        pid=pid,
        serial_number=serial_number,
        description=description,
    )


class SelectionTests(unittest.TestCase):
    def test_selects_only_plausible_usb_device(self):
        bluetooth = port("/dev/cu.Bluetooth", vid=None, pid=None, serial_number=None)
        self.assertEqual(
            flash.select_port([bluetooth, port()], None).device, "/dev/cu.board"
        )

    def test_zero_or_multiple_candidates_print_every_device(self):
        devices = [
            port("/dev/cu.one", serial_number="one"),
            port("/dev/cu.two", serial_number="two"),
            port("/dev/cu.Bluetooth", vid=None, pid=None, serial_number=None),
        ]
        for selected in (devices[2:], devices):
            output = io.StringIO()
            with redirect_stdout(output), self.assertRaises(flash.FlashError):
                flash.select_port(selected, None)
            for device in selected:
                self.assertIn(device.device, output.getvalue())

    def test_explicit_port_must_be_enumerated(self):
        self.assertEqual(
            flash.select_port([port()], "/dev/cu.board").device, "/dev/cu.board"
        )
        with self.assertRaisesRegex(flash.FlashError, "does not exist"):
            flash.select_port([port()], "/dev/cu.missing")


class HardwareTests(unittest.TestCase):
    def test_identity_requires_exact_protocol_and_service(self):
        reply = f"broglow_pair identity=146393924524 service={flash.SERVICE}"
        self.assertEqual(flash.parse_identity(reply), "146393924524")
        self.assertIsNone(flash.parse_identity(reply + " extra"))
        self.assertIsNone(flash.parse_identity(reply.replace(flash.SERVICE, "wrong")))

    @mock.patch.object(flash, "esptool")
    def test_requires_c3_and_four_megabytes(self, tool):
        tool.side_effect = [
            subprocess.CompletedProcess(
                [], 0, stdout="Chip type: ESP32-C3 (revision v0.4)\n"
            ),
            subprocess.CompletedProcess([], 0, stdout="Detected flash size: 4 MB\n"),
        ]
        self.assertEqual(flash.verify_hardware("port"), ("ESP32-C3", "4 MB"))

        tool.side_effect = [
            subprocess.CompletedProcess([], 0, stdout="Chip is ESP32-S3\n")
        ]
        with self.assertRaisesRegex(flash.FlashError, "requires ESP32-C3"):
            flash.verify_hardware("port")

        tool.side_effect = [
            subprocess.CompletedProcess([], 0, stdout="Chip is ESP32-C3\n"),
            subprocess.CompletedProcess([], 0, stdout="Detected flash size: 2MB\n"),
        ]
        with self.assertRaisesRegex(flash.FlashError, "requires 4 MB"):
            flash.verify_hardware("port")

    @mock.patch.object(flash, "Serial")
    def test_identity_probe_never_drains_output(self, serial):
        connection = serial.return_value.__enter__.return_value
        connection.flush.side_effect = AssertionError("tcdrain blocks forever")
        connection.readline.return_value = (
            f"broglow_pair identity=146393924524 service={flash.SERVICE}\n".encode()
        )
        self.assertEqual(flash.read_identity("port"), "146393924524")

    @mock.patch.object(flash, "read_identity")
    def test_skip_info_only_skips_preflash_probe(self, read_identity):
        self.assertEqual(flash.inspect_current_firmware("port", True), "skipped")
        read_identity.assert_not_called()


class BackupTests(unittest.TestCase):
    def test_backup_reads_complete_flash_and_is_private(self):
        with (
            tempfile.TemporaryDirectory() as folder,
            mock.patch.object(flash, "ROOT", Path(folder)),
            mock.patch.object(flash, "esptool") as tool,
        ):

            def write_backup(_port, *arguments, **_kwargs):
                Path(arguments[-1]).write_bytes(b"x" * flash.FLASH_BYTES)

            tool.side_effect = write_backup
            result = flash.backup_flash("port")
            self.assertEqual(result.stat().st_size, flash.FLASH_BYTES)
            self.assertEqual(result.stat().st_mode & 0o777, 0o600)
            self.assertEqual(
                tool.call_args.args[1:6],
                ("--chip", "esp32c3", "read-flash", "0", hex(flash.FLASH_BYTES)),
            )

    def test_requested_backup_failure_aborts_and_removes_partial(self):
        with (
            tempfile.TemporaryDirectory() as folder,
            mock.patch.object(flash, "ROOT", Path(folder)),
            mock.patch.object(flash, "esptool", side_effect=flash.FlashError("failed")),
        ):
            with self.assertRaises(flash.FlashError):
                flash.backup_flash("port")
            self.assertEqual(list((Path(folder) / ".recovery").glob("*")), [])


class ManifestTests(unittest.TestCase):
    def test_validates_four_prebuilt_segments_and_offsets(self):
        with tempfile.TemporaryDirectory() as folder:
            firmware = Path(folder)
            for filename in flash.EXPECTED_SEGMENTS.values():
                (firmware / filename).write_bytes(b"image")
            manifest = {
                "chip": "esp32c3",
                "flash_mode": "dio",
                "flash_freq": "40m",
                "flash_size": "4MB",
                "segments": [
                    {"offset": hex(offset), "file": filename}
                    for offset, filename in flash.EXPECTED_SEGMENTS.items()
                ],
            }
            (firmware / "flash-manifest.json").write_text(json.dumps(manifest))
            with mock.patch.object(flash, "FIRMWARE", firmware):
                loaded, segments = flash.load_manifest()
            self.assertEqual(loaded, manifest)
            self.assertEqual(
                [offset for offset, _ in segments], sorted(flash.EXPECTED_SEGMENTS)
            )

    @mock.patch.object(flash, "esptool")
    def test_prebuilt_write_uses_manifest_offsets(self, tool):
        manifest = {
            "chip": "esp32c3",
            "flash_mode": "dio",
            "flash_freq": "40m",
            "flash_size": "4MB",
        }
        segments = [
            (offset, Path(filename))
            for offset, filename in flash.EXPECTED_SEGMENTS.items()
        ]
        flash.flash_prebuilt("port", manifest, segments)
        command = tool.call_args.args
        self.assertIn("write-flash", command)
        for offset, path in segments:
            index = command.index(hex(offset))
            self.assertEqual(command[index + 1], str(path))


class FlowTests(unittest.TestCase):
    def args(self, **changes):
        values = {
            "port": "/dev/cu.board",
            "backup": False,
            "skip_info": True,
            "yes": False,
            "build_only": False,
        }
        values.update(changes)
        return SimpleNamespace(**values)

    def common(self):
        return mock.patch.multiple(
            flash,
            inspect_current_firmware=mock.DEFAULT,
            verify_hardware=mock.DEFAULT,
            backup_flash=mock.DEFAULT,
            load_manifest=mock.DEFAULT,
            confirm=mock.DEFAULT,
            release_serial_port=mock.DEFAULT,
            flash_source=mock.DEFAULT,
            flash_prebuilt=mock.DEFAULT,
            wait_for_port=mock.DEFAULT,
            read_identity=mock.DEFAULT,
        )

    def test_yes_requires_explicit_port(self):
        with self.assertRaisesRegex(flash.FlashError, "--yes requires"):
            flash.execute(self.args(port=None, yes=True))

    @mock.patch("builtins.input", side_effect=EOFError)
    def test_missing_confirmation_defaults_to_no(self, _input):
        self.assertFalse(flash.confirm())

    @mock.patch.object(flash.time, "sleep")
    @mock.patch.object(flash.shutil, "which", return_value="/bin/broglow")
    @mock.patch.object(flash, "run")
    def test_release_restores_only_previously_enabled_control(
        self, run, _which, _sleep
    ):
        for status, expected in (
            ("connected: lamp connected\n", "/bin/broglow"),
            ("disabled: Lamps disabled\n", None),
        ):
            with self.subTest(status=status):
                run.reset_mock()
                run.side_effect = [
                    subprocess.CompletedProcess([], 0, stdout=status),
                    subprocess.CompletedProcess([], 0),
                ]
                self.assertEqual(flash.release_serial_port(), expected)
                self.assertEqual(
                    run.call_args_list,
                    [
                        mock.call(["/bin/broglow", "ble", "status"], capture=True),
                        mock.call(["/bin/broglow", "ble", "off"]),
                    ],
                )

    @mock.patch.object(flash.list_ports, "comports", return_value=[port()])
    def test_no_backup_and_rejection_write_nothing(self, _comports):
        output = io.StringIO()
        with self.common() as mocked, redirect_stdout(output):
            mocked["inspect_current_firmware"].return_value = "skipped"
            mocked["verify_hardware"].return_value = ("ESP32-C3", "4 MB")
            mocked["load_manifest"].return_value = None
            mocked["release_serial_port"].return_value = None
            mocked["confirm"].return_value = False
            self.assertEqual(flash.execute(self.args()), 0)
        mocked["backup_flash"].assert_not_called()
        mocked["flash_source"].assert_not_called()
        mocked["flash_prebuilt"].assert_not_called()
        self.assertIn("No backup requested", output.getvalue())

    @mock.patch.object(flash, "run")
    @mock.patch.object(flash.time, "sleep")
    @mock.patch.object(flash.list_ports, "comports", return_value=[port()])
    def test_pause_precedes_every_probe_and_restores_on_early_exit(
        self, _comports, sleep, run
    ):
        for failure in (None, "probe", "backup", "interrupt"):
            with self.subTest(failure=failure), self.common() as mocked:
                run.reset_mock()
                sleep.reset_mock()
                sleep.side_effect = (
                    KeyboardInterrupt if failure == "interrupt" else None
                )
                mocked["release_serial_port"].return_value = "/bin/broglow"
                mocked["load_manifest"].return_value = None
                mocked["confirm"].return_value = False

                def probe(*_args):
                    mocked["release_serial_port"].assert_called_once()
                    sleep.assert_called_once_with(3)
                    if failure == "probe":
                        raise flash.FlashError("probe failed")
                    return "recognized"

                mocked["inspect_current_firmware"].side_effect = probe
                mocked["verify_hardware"].return_value = ("ESP32-C3", "4 MB")
                if failure == "backup":
                    mocked["backup_flash"].side_effect = flash.FlashError(
                        "backup failed"
                    )
                args = self.args(backup=failure == "backup")
                if failure:
                    with self.assertRaises((flash.FlashError, KeyboardInterrupt)):
                        flash.execute(args)
                else:
                    self.assertEqual(flash.execute(args), 0)
                mocked["flash_source"].assert_not_called()
                mocked["flash_prebuilt"].assert_not_called()
                run.assert_called_once_with(["/bin/broglow", "ble", "on"])

    @mock.patch.object(flash, "run")
    @mock.patch.object(flash.shutil, "which", return_value=None)
    def test_pause_finds_the_documented_install_without_path_setup(self, _which, run):
        with (
            tempfile.TemporaryDirectory() as folder,
            mock.patch.object(flash.Path, "home", return_value=Path(folder)),
        ):
            executable = Path(folder) / ".cargo/bin/broglow"
            executable.parent.mkdir(parents=True)
            executable.touch()
            run.return_value = subprocess.CompletedProcess([], 0, stdout="connected:")
            self.assertEqual(flash.release_serial_port(), str(executable))
            self.assertEqual(run.call_args.args[0], [str(executable), "ble", "off"])

    @mock.patch.object(flash, "run")
    @mock.patch.object(flash.list_ports, "comports", return_value=[port()])
    def test_postflash_identity_is_mandatory_even_with_skip_info(self, _comports, run):
        executable = "/Applications/BroGlow.app/Contents/MacOS/broglow"
        with self.common() as mocked:
            mocked["inspect_current_firmware"].return_value = "skipped"
            mocked["verify_hardware"].return_value = ("ESP32-C3", "4 MB")
            mocked["load_manifest"].return_value = None
            mocked["release_serial_port"].return_value = executable
            mocked["wait_for_port"].return_value = "/dev/cu.board"
            mocked["read_identity"].side_effect = flash.FlashError("not BroGlow")
            with self.assertRaisesRegex(flash.FlashError, "not BroGlow"):
                flash.execute(self.args(yes=True))
        mocked["flash_source"].assert_called_once_with("/dev/cu.board")
        mocked["read_identity"].assert_called_once()
        run.assert_called_once_with([executable, "ble", "on"])

    @mock.patch.object(flash, "run")
    @mock.patch.object(flash.list_ports, "comports", return_value=[port()])
    def test_success_restores_enabled_lamp_control(self, _comports, run):
        executable = "/Applications/BroGlow.app/Contents/MacOS/broglow"
        with self.common() as mocked:
            mocked["inspect_current_firmware"].return_value = "skipped"
            mocked["verify_hardware"].return_value = ("ESP32-C3", "4 MB")
            mocked["load_manifest"].return_value = None
            mocked["release_serial_port"].return_value = executable
            mocked["wait_for_port"].return_value = "/dev/cu.board"
            mocked["read_identity"].return_value = "001122334455"
            self.assertEqual(flash.execute(self.args(yes=True)), 0)
        run.assert_called_once_with([executable, "ble", "on"])

    @mock.patch.object(flash, "run")
    def test_source_upload_uses_explicit_port(self, run):
        flash.flash_source("/dev/cu.explicit")
        command = run.call_args.args[0]
        self.assertEqual(command[-2:], ["--upload-port", "/dev/cu.explicit"])
        self.assertIn("platformio", command)


if __name__ == "__main__":
    unittest.main()
