"""Stamp the firmware with its own version from the VERSION file next to it.

PlatformIO pre-script: the firmware reports this version over USB and BLE so the
app can offer an update. The version belongs to the firmware, not the app:
scripts/firmware-version.py keeps it in step with the sources.
"""

from pathlib import Path

Import("env")  # noqa: F821 - provided by PlatformIO's SCons environment.

version = (Path(env["PROJECT_DIR"]) / "VERSION").read_text().strip()  # noqa: F821
if not version:
    raise SystemExit("firmware/esp32-c3-supermini/VERSION is empty")
env.Append(  # noqa: F821
    CPPDEFINES=[("BROGLOW_FIRMWARE_VERSION", env.StringifyMacro(version))]  # noqa: F821
)
