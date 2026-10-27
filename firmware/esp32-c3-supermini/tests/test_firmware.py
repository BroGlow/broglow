"""Exercise real firmware with host hardware fakes, without accessing a board."""

import configparser
from pathlib import Path
import re
import shlex
import subprocess

import pytest

PROJECT = Path(__file__).resolve().parents[1]
# version.py stamps real builds; the host build passes the same define itself.
APP_VERSION = re.search(
    r'^version = "([^"]+)"$', (PROJECT.parents[1] / "Cargo.toml").read_text(), re.M
).group(1)
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
    flags.append(f'-DBROGLOW_FIRMWARE_VERSION="{APP_VERSION}"')
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
