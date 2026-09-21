#!/usr/bin/env python3
"""Check release-source guards using a disposable Git repository; no builds or hardware."""

import importlib.util
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

spec = importlib.util.spec_from_file_location(
    "package_macos", Path(__file__).resolve().parents[1] / "scripts/package-macos.py"
)
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)
for command, field in (
    ("LC_BUILD_VERSION", "minos"),
    ("LC_VERSION_MIN_MACOSX", "version"),
):
    assert package.macho_build_info(
        f"Load command 1\n cmd {command}\n {field} 11.0\n sdk 15.0\n"
    ) == {"minos": "11.0", "sdk": "15.0"}
assert package.macho_build_info(
    "Load command 1\n cmd LC_BUILD_VERSION\n minos 11.0\n sdk 26.2\n tool 3\n version 1230.1\n"
) == {"minos": "11.0", "sdk": "26.2"}
assert package.macho_build_info("Load command 0\n cmd LC_MAIN\n") == {}

with tempfile.TemporaryDirectory(prefix="broglow-flash-package-") as folder:
    destination = Path(folder)
    package.copy_flash_tool(destination)
    copied = sorted(
        str(path.relative_to(destination))
        for path in destination.rglob("*")
        if path.is_file()
    )
    expected = sorted(
        str(path.relative_to(package.ROOT)) for path in package.flash_tool_sources()
    )
    assert copied == expected


with tempfile.TemporaryDirectory(prefix="broglow-manual-check-") as folder:
    destination = Path(folder)
    package.copy_manual(destination)
    firmware = destination / "firmware/esp32-c3-supermini"
    firmware.mkdir(parents=True)
    for name in ("DEPENDENCIES.md", "DEPENDENCIES.ru.md"):
        (firmware / name).write_text("Included by the firmware packaging step.\n")
    shutil.copy2(package.C3 / "platformio.ini", firmware / "platformio.ini")
    assert not (destination / "CLAUDE.md").exists()
    assert not (destination / "CONTRIBUTING.md").exists()
    for document in destination.rglob("*.md"):
        for target in re.findall(r"\[[^\]]*\]\(([^\s)]+)\)", document.read_text()):
            if ":" in target or target.startswith("#"):
                continue
            assert (document.parent / target.split("#")[0]).exists(), (
                document.relative_to(destination),
                target,
            )


def rejected(tag, message):
    try:
        package.release_identity("1.2.3", tag)
    except SystemExit as error:
        assert message in str(error), error
    else:
        raise AssertionError("Invalid release source accepted")


with tempfile.TemporaryDirectory(prefix="broglow-release-check-") as folder:
    package.ROOT = Path(folder)

    def git(*args):
        return subprocess.check_output(
            ["git", *args], cwd=folder, text=True, stderr=subprocess.PIPE
        ).strip()

    git("init")
    git("config", "user.name", "Package Check")
    git("config", "user.email", "package-check@example.invalid")
    source = package.ROOT / "source"
    source.write_text("first\n")
    git("add", "source")
    git("commit", "-m", "first")
    git("tag", "v1.2.3")
    assert package.release_identity("1.2.3", "v1.2.3") == (
        git("rev-parse", "HEAD"),
        False,
    )
    rejected("v9.9.9", "Cargo.toml version")
    source.write_text("second\n")
    rejected("v1.2.3", "clean source")
    assert package.release_identity("1.2.3", None)[1]
    git("add", "source")
    git("commit", "-m", "second")
    rejected("v1.2.3", "checked-out commit")
print("PASS: release source guards and arm64/Intel macOS metadata")
