"""Stamp the firmware with the app version from the checkout's Cargo.toml.

PlatformIO pre-script: the firmware reports this version over USB and BLE so the
app can offer an update. A package rebuild without Cargo.toml stamps "unknown".
"""

from pathlib import Path
import re

Import("env")  # noqa: F821 - provided by PlatformIO's SCons environment.

MANIFEST = Path(env["PROJECT_DIR"]).resolve().parents[1] / "Cargo.toml"  # noqa: F821
version = "unknown"
if MANIFEST.is_file():
    match = re.search(r'^version = "([^"]+)"$', MANIFEST.read_text(), re.M)
    if match:
        version = match.group(1)
env.Append(  # noqa: F821
    CPPDEFINES=[("BROGLOW_FIRMWARE_VERSION", env.StringifyMacro(version))]  # noqa: F821
)
