#!/usr/bin/env python3
"""Compile the real firmware with host mocks; no board or external Python packages."""

import configparser
from pathlib import Path
import shlex
import subprocess
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
CONFIG = configparser.ConfigParser(interpolation=None)
CONFIG.read(PROJECT / "platformio.ini")
FLAGS = shlex.split(CONFIG["env:esp32-c3-supermini"]["build_flags"])
SCENARIOS = (
    "protocol",
    "pairing",
    "indications",
    "brightness",
    "startup",
    "startup-live",
    "preview",
    "identify",
    "races",
    "gatt-change",
    "gatt-mode-failed",
)

with tempfile.TemporaryDirectory(prefix="broglow-c3-check-") as folder:
    directory = Path(folder)
    # check.cpp defines only the hardware API mocks used by the included firmware.
    for name in (
        "Arduino.h",
        "BLESecurity.h",
        "BLEDevice.h",
        "BLEServer.h",
        "esp_gatts_api.h",
        "freertos/FreeRTOS.h",
        "freertos/queue.h",
    ):
        header = directory / name
        header.parent.mkdir(exist_ok=True)
        header.touch()
    executable = directory / "check"
    for override in (None, "BUILTIN_LED_PIN=-1", "BUILTIN_LED_ACTIVE_LOW=0"):
        flags = (
            [] if override is None else ["-U" + override.split("=")[0], "-D" + override]
        )
        subprocess.run(
            [
                "c++",
                "-std=c++17",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-I",
                str(directory),
                *FLAGS,
                *flags,
                str(PROJECT / "tests/check.cpp"),
                "-o",
                str(executable),
            ],
            check=True,
        )
        # Each scenario starts with clean globals, like a fresh power-on.
        for scenario in (
            SCENARIOS
            if override is None
            else (
                "indications",
                "brightness",
                "startup",
                "startup-live",
                "preview",
                "identify",
            )
        ):
            subprocess.run([str(executable), scenario], check=True, timeout=10)
    print(
        "PASS: protocol/security/ownership, pairing, PWM/builtin, brightness/startup/preview/identify, races, GATT cache change"
    )
