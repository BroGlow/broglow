#!/usr/bin/env python3
"""Check package contents and release-source guards; no builds or hardware.

Third-party notices are generated for every shipped Rust target from the locked
dependency graph (cargo metadata, cargo tree and crate sources; network only if
crates are not yet downloaded).
"""

import importlib.util
import json
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


notices = package.notices
PLACEHOLDERS = (
    "<year>",
    "<copyright holders>",
    "<name of author>",
    "[yyyy]",
    "[name of copyright owner]",
    "{yyyy}",
    "{name of copyright owner}",
)
# Verbatim license documents end with "how to apply" instructions that contain
# template fields; nothing else may.
APACHE_APPENDIX = re.compile(
    r"APPENDIX: How to apply the Apache License to your work\.[\s\S]*?"
    r"limitations under the License\."
)
GNU_HOW_TO = re.compile(r"How to Apply These Terms to Your New (?:Programs|Libraries)")
TEXT_BLOCK = re.compile(r"(?ms)^(`{3,})text\n(.*?)^\1$")


def template_fields(markdown):
    found = []

    def check(match):
        text = APACHE_APPENDIX.sub("", match[2])
        text = GNU_HOW_TO.split(text)[0]
        found.extend(field for field in PLACEHOLDERS if field in text)
        return ""

    prose = TEXT_BLOCK.sub(check, markdown)
    return sorted(set(found + [field for field in PLACEHOLDERS if field in prose]))


def cargo(*args):
    return subprocess.check_output(
        ["cargo", *args, "--locked"], cwd=package.ROOT, text=True, encoding="utf-8"
    )


def normalized(path):
    data = path.read_bytes()
    try:
        text = data.decode("utf-8-sig")
    except UnicodeDecodeError:
        text = data.decode("latin-1")
    return text.replace("\r\n", "\n").replace("\r", "\n").rstrip("\n") + "\n"


host = next(
    line.split(": ", 1)[1]
    for line in subprocess.check_output(["rustc", "-vV"], text=True).splitlines()
    if line.startswith("host: ")
)
for target in notices.SHIPPED_TARGETS:
    with tempfile.TemporaryDirectory(prefix="broglow-notices-check-") as folder:
        destination = Path(folder)
        notices.write_rust_notices(destination, target)
        document = (destination / notices.NOTICES).read_text(encoding="utf-8")
        assert (destination / notices.RUST_STD).stat().st_size > 0, target
    assert template_fields(document) == [], (target, template_fields(document))
    rows = {
        (name, version): (use, texts)
        for name, version, use, texts in re.findall(
            r"(?m)^\| (\S+) \| (\S+) \| .*? \| .*? \| (binary|build) \| (.*) \|$",
            document,
        )
    }
    for use, texts in rows.values():
        assert re.search(r"\[T\d+\]\(#t\d+\)", texts), texts
    for anchor in set(re.findall(r"\(#(t\d+)\)", document)):
        assert f"\n### {anchor.upper()}\n" in document, (target, anchor)

    # Independent graph: everything linked into the binary must be listed as such,
    # and on a native host the complete build graph must match too.
    def tree(edges):
        output = cargo(
            "tree",
            "--target",
            target,
            "-e",
            edges,
            "--prefix",
            "none",
            "--format",
            "{p}",
        )
        return {
            (line.split()[0], line.split()[1].removeprefix("v"))
            for line in output.splitlines()
            if line.strip() and not line.startswith("broglow ")
        }

    linked = {crate for crate, (use, _) in rows.items() if use == "binary"}
    expected = tree("normal,no-proc-macro")
    assert linked == expected, (target, sorted(linked ^ expected))
    if target == host:
        expected = tree("normal,build")
        assert set(rows) == expected, (target, sorted(set(rows) ^ expected))

    # Every license, notice and copyright file in each crate's top level is included.
    metadata = json.loads(
        cargo("metadata", "--format-version", "1", "--filter-platform", target)
    )
    sources = {
        (crate["name"], crate["version"]): Path(crate["manifest_path"]).parent
        for crate in metadata["packages"]
    }
    for crate in rows:
        shipped = [
            path
            for path in sorted(sources[crate].iterdir())
            if path.is_file()
            and re.match(
                r"(?i)(licen[cs]e|copying|notice|copyright|unlicense)", path.name
            )
            and path.suffix not in (".rs", ".toml")
        ]
        for path in shipped:
            assert normalized(path) in document, (target, crate, path.name)
        if not shipped:
            assert f"- **{crate[0]} {crate[1]}** (" in document, (target, crate)
    print(f"{target}: notices cover {len(rows)} crates ({len(linked)} linked)")


with tempfile.TemporaryDirectory(prefix="broglow-manual-check-") as folder:
    destination = Path(folder)
    package.copy_manual(destination)
    notices.write_rust_notices(destination, notices.SHIPPED_TARGETS[0])
    firmware = destination / "firmware/esp32-c3-supermini"
    firmware.mkdir(parents=True)
    package.write_firmware_dependencies(firmware)
    build = package.C3 / ".pio/build/esp32-c3-supermini"
    platformio = Path.home() / ".platformio/packages"
    if (build / "firmware.map").is_file() and (
        platformio / "toolchain-riscv32-esp"
    ).is_dir():
        # Real firmware notices when this machine has built the pinned image.
        framework = platformio / "framework-arduinoespressif32"
        document, archives = notices.firmware_notices(
            framework, platformio / "toolchain-riscv32-esp", build / "firmware.map"
        )
        assert template_fields(document) == [], template_fields(document)
        assert set(archives) >= {"libFrameworkArduino.a", "libBLE.a", "libc.a"}
        package.write_firmware_notices(firmware, framework, document)
        print(f"firmware: notices cover {len(archives)} linked archives")
    else:
        for name in ("ARDUINO-LICENSE.md", "BLE-LICENSE.txt", notices.NOTICES):
            (firmware / name).write_text("Written by the firmware packaging step.\n")
    # A newly linked, unreviewed firmware library must stop packaging.
    new_map = Path(folder) / "new.map"
    new_map.write_text(
        "Archive member included to satisfy reference by file (symbol)\n\n"
        "/p/libFrameworkArduino.a(main.cpp.o)\n                              (x)\n"
        "/p/libnew.a(new.c.o)\n                              main.o (x)\n"
    )
    try:
        notices.firmware_notices(Path(folder), Path(folder), new_map)
    except notices.NoticeError as error:
        assert "libnew.a" in str(error) and "libFrameworkArduino" not in str(error)
    else:
        raise AssertionError("Unreviewed firmware archive accepted")
    shutil.copy2(package.C3 / "platformio.ini", firmware / "platformio.ini")
    assert (destination / notices.NOTICES).is_file()
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
print("PASS: third-party notices, release source guards and arm64/Intel macOS metadata")
