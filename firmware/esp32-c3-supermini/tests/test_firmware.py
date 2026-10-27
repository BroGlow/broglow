"""Exercise real firmware with host hardware fakes, without accessing a board."""

import configparser
import importlib.util
from pathlib import Path
import shlex
import subprocess

import pytest

PROJECT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location(
    "firmware_version", PROJECT.parents[1] / "scripts/firmware-version.py"
)
firmware_version = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(firmware_version)
# version.py stamps real builds; the host build passes the same define itself.
FIRMWARE_VERSION = firmware_version.version()
LED_SCENARIOS = (
    "indications",
    "brightness",
    "fade",
    "disconnect-fade",
    "fade-brightness",
    "startup",
    "startup-live",
    "preview",
    "identify",
)
PROTOCOL_SCENARIOS = (
    "protocol",
    "pairing",
    "races",
    "name",
    "gatt-change",
    "gatt-mode-failed",
)


@pytest.fixture(scope="module")
def firmware_binary(
    request: pytest.FixtureRequest, tmp_path_factory: pytest.TempPathFactory
) -> Path:
    directory = tmp_path_factory.mktemp("firmware")
    # check.cpp defines only the hardware APIs used by the included firmware.
    for name in (
        "Arduino.h",
        "BLESecurity.h",
        "BLEDevice.h",
        "BLEServer.h",
        "esp_gatts_api.h",
        "nvs.h",
        "freertos/FreeRTOS.h",
        "freertos/queue.h",
    ):
        header = directory / name
        header.parent.mkdir(exist_ok=True)
        header.touch()
    config = configparser.ConfigParser(interpolation=None)
    config.read(PROJECT / "platformio.ini")
    flags = shlex.split(config["env:esp32-c3-supermini"]["build_flags"])
    flags.append(f'-DBROGLOW_FIRMWARE_VERSION="{FIRMWARE_VERSION}"')
    if request.param is not None:
        flags.extend(["-U" + request.param.split("=")[0], "-D" + request.param])
    executable = directory / "check"
    subprocess.run(
        [
            "c++",
            "-std=c++17",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-I",
            str(directory),
            *flags,
            str(PROJECT / "tests/check.cpp"),
            "-o",
            str(executable),
        ],
        check=True,
        timeout=60,
    )
    return executable


@pytest.mark.parametrize(
    ("firmware_binary", "scenario"),
    [
        pytest.param(override, scenario, id=f"{variant}-{scenario}")
        for variant, override, scenarios in (
            ("default", None, PROTOCOL_SCENARIOS + LED_SCENARIOS),
            ("builtin-disabled", "BUILTIN_LED_PIN=-1", LED_SCENARIOS),
            ("builtin-active-high", "BUILTIN_LED_ACTIVE_LOW=0", LED_SCENARIOS),
        )
        for scenario in scenarios
    ],
    indirect=["firmware_binary"],
    scope="module",
)
def test_firmware_behavior(firmware_binary: Path, scenario: str) -> None:
    # Each scenario starts in a new process, like a fresh power-on.
    subprocess.run([str(firmware_binary), scenario], check=True, timeout=10)


def test_firmware_version_is_locked_to_its_sources() -> None:
    # A changed sketch or build config needs a new VERSION, or lamps already on
    # this version would silently run different firmware than new ones.
    assert firmware_version.stale() is None, firmware_version.stale()


def test_version_lock_refuses_to_re_record_changed_sources(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    c3 = tmp_path / "esp32-c3-supermini"
    (c3 / "src").mkdir(parents=True)
    (c3 / "src/main.cpp").write_text("// v1\n")
    (c3 / "platformio.ini").write_text("[env]\n")
    (c3 / "VERSION").write_text("1.2.0\n")
    monkeypatch.setattr(firmware_version, "C3", c3)
    monkeypatch.setattr(firmware_version, "VERSION", c3 / "VERSION")
    monkeypatch.setattr(firmware_version, "LOCK", c3 / "version-lock.json")
    firmware_version.record()
    assert firmware_version.stale() is None

    (c3 / "src/main.cpp").write_text("// v2\n")
    assert "set a new version" in firmware_version.stale()
    with pytest.raises(SystemExit, match="bump VERSION first"):
        firmware_version.record()
    (c3 / "VERSION").write_text("1.3.0\n")
    assert "records 1.2.0" in firmware_version.stale()
    firmware_version.record()
    assert firmware_version.stale() is None
