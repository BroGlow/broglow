"""Package contents and release guards, using existing Cargo/firmware sources."""

from dataclasses import dataclass
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

import pytest

spec = importlib.util.spec_from_file_location(
    "package_macos", Path(__file__).resolve().parents[1] / "scripts/package-macos.py"
)
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)
notices = package.notices
RUST_TARGETS = tuple(
    pytest.param(target, id=target) for target in notices.SHIPPED_TARGETS
)
PLACEHOLDERS = (
    "<year>",
    "<copyright holders>",
    "<name of author>",
    "[yyyy]",
    "[name of copyright owner]",
    "{yyyy}",
    "{name of copyright owner}",
)
# Only verbatim licenses' "how to apply" instructions may contain template fields.
APACHE_APPENDIX = re.compile(
    r"APPENDIX: How to apply the Apache License to your work\.[\s\S]*?"
    r"limitations under the License\."
)
GNU_HOW_TO = re.compile(r"How to Apply These Terms to Your New (?:Programs|Libraries)")
TEXT_BLOCK = re.compile(r"(?ms)^(`{3,})text\n(.*?)^\1$")


def template_fields(markdown: str) -> list[str]:
    found = []

    def check(match: re.Match[str]) -> str:
        text = APACHE_APPENDIX.sub("", match[2])
        text = GNU_HOW_TO.split(text)[0]
        found.extend(field for field in PLACEHOLDERS if field in text)
        return ""

    prose = TEXT_BLOCK.sub(check, markdown)
    return sorted(set(found + [field for field in PLACEHOLDERS if field in prose]))


def cargo(*args: str) -> str:
    return subprocess.check_output(
        ["cargo", *args, "--locked"], cwd=package.ROOT, text=True, encoding="utf-8"
    )


def normalized(path: Path) -> str:
    data = path.read_bytes()
    try:
        text = data.decode("utf-8-sig")
    except UnicodeDecodeError:
        text = data.decode("latin-1")
    return text.replace("\r\n", "\n").replace("\r", "\n").rstrip("\n") + "\n"


def notice_rows(document: str) -> dict[tuple[str, str], tuple[str, str]]:
    return {
        (name, version): (use, texts)
        for name, version, use, texts in re.findall(
            r"(?m)^\| (\S+) \| (\S+) \| .*? \| .*? \| (binary|build) \| (.*) \|$",
            document,
        )
    }


def cargo_tree(target: str, edges: str) -> set[tuple[str, str]]:
    output = cargo(
        "tree", "--target", target, "-e", edges, "--prefix", "none", "--format", "{p}"
    )
    return {
        (line.split()[0], line.split()[1].removeprefix("v"))
        for line in output.splitlines()
        if line.strip() and not line.startswith("broglow ")
    }


@pytest.fixture(scope="module")
def rust_notices(tmp_path_factory: pytest.TempPathFactory) -> dict[str, Path]:
    directories = {}
    for target in notices.SHIPPED_TARGETS:
        destination = tmp_path_factory.mktemp(f"notices-{target}")
        notices.write_rust_notices(destination, target)
        directories[target] = destination
    return directories


@pytest.fixture(scope="module")
def rust_host() -> str:
    return next(
        line.split(": ", 1)[1]
        for line in subprocess.check_output(["rustc", "-vV"], text=True).splitlines()
        if line.startswith("host: ")
    )


@dataclass(frozen=True)
class FirmwareNotices:
    framework: Path
    document: str
    archives: dict[str, int]


@pytest.fixture(scope="module")
def local_firmware_notices() -> FirmwareNotices | None:
    build = package.C3 / ".pio/build/esp32-c3-supermini"
    platformio = Path.home() / ".platformio/packages"
    toolchain = platformio / "toolchain-riscv32-esp"
    if not (build / "firmware.map").is_file() or not toolchain.is_dir():
        return None
    framework = platformio / "framework-arduinoespressif32"
    document, archives = notices.firmware_notices(
        framework, toolchain, build / "firmware.map"
    )
    return FirmwareNotices(framework, document, archives)


@pytest.fixture(scope="module")
def packaged_manual(
    tmp_path_factory: pytest.TempPathFactory,
    rust_notices: dict[str, Path],
    local_firmware_notices: FirmwareNotices | None,
) -> Path:
    destination = tmp_path_factory.mktemp("packaged-manual")
    package.copy_manual(destination)
    generated = rust_notices[notices.SHIPPED_TARGETS[0]]
    for name in (notices.NOTICES, notices.RUST_STD):
        shutil.copy2(generated / name, destination / name)
    firmware = destination / "firmware/esp32-c3-supermini"
    firmware.mkdir(parents=True)
    package.write_firmware_dependencies(firmware)
    if local_firmware_notices is None:
        for name in ("ARDUINO-LICENSE.md", "BLE-LICENSE.txt", notices.NOTICES):
            (firmware / name).write_text("Written by the firmware packaging step.\n")
    else:
        package.write_firmware_notices(
            firmware, local_firmware_notices.framework, local_firmware_notices.document
        )
    shutil.copy2(package.C3 / "platformio.ini", firmware / "platformio.ini")
    return destination


@pytest.mark.parametrize(
    ("load_commands", "expected"),
    (
        pytest.param(
            "Load command 1\n cmd LC_BUILD_VERSION\n minos 11.0\n sdk 15.0\n",
            {"minos": "11.0", "sdk": "15.0"},
            id="arm64-build-version",
        ),
        pytest.param(
            "Load command 1\n cmd LC_VERSION_MIN_MACOSX\n version 11.0\n sdk 15.0\n",
            {"minos": "11.0", "sdk": "15.0"},
            id="intel-minimum-version",
        ),
        pytest.param(
            "Load command 1\n cmd LC_BUILD_VERSION\n minos 11.0\n sdk 26.2\n tool 3\n version 1230.1\n",
            {"minos": "11.0", "sdk": "26.2"},
            id="ignore-linker-tool-version",
        ),
        pytest.param("Load command 0\n cmd LC_MAIN\n", {}, id="no-version-command"),
    ),
)
def test_macho_build_info(load_commands: str, expected: dict[str, str]) -> None:
    assert package.macho_build_info(load_commands) == expected


def test_packaged_flash_entrypoint_runs_without_the_checkout(tmp_path: Path) -> None:
    package.copy_flash_tool(tmp_path)

    subprocess.run(
        [sys.executable, str(tmp_path / "tools/broglow_tools/flash.py"), "--help"],
        cwd=tmp_path,
        check=True,
        capture_output=True,
        text=True,
    )

    for name in ("pyproject.toml", "uv.lock"):
        assert (tmp_path / name).read_bytes() == (package.ROOT / name).read_bytes()
    assert not list(tmp_path.rglob("__pycache__"))
    assert not list(tmp_path.rglob("*.pyc"))


@pytest.mark.parametrize("target", RUST_TARGETS)
def test_rust_notices_have_license_texts_and_resolvable_anchors(
    rust_notices: dict[str, Path], target: str
) -> None:
    destination = rust_notices[target]
    document = (destination / notices.NOTICES).read_text(encoding="utf-8")

    assert (destination / notices.RUST_STD).stat().st_size > 0
    assert template_fields(document) == []
    for _, texts in notice_rows(document).values():
        assert re.search(r"\[T\d+\]\(#t\d+\)", texts), texts
    for anchor in set(re.findall(r"\(#(t\d+)\)", document)):
        assert f"\n### {anchor.upper()}\n" in document, anchor


@pytest.mark.parametrize("target", RUST_TARGETS)
def test_rust_notices_cover_the_independent_cargo_dependency_graph(
    rust_notices: dict[str, Path], rust_host: str, target: str
) -> None:
    document = (rust_notices[target] / notices.NOTICES).read_text(encoding="utf-8")
    rows = notice_rows(document)
    linked = {crate for crate, (use, _) in rows.items() if use == "binary"}

    assert linked == cargo_tree(target, "normal,no-proc-macro")
    if target == rust_host:
        assert set(rows) == cargo_tree(target, "normal,build")


@pytest.mark.parametrize("target", RUST_TARGETS)
def test_rust_notices_include_original_crate_license_files(
    rust_notices: dict[str, Path], target: str
) -> None:
    document = (rust_notices[target] / notices.NOTICES).read_text(encoding="utf-8")
    metadata = json.loads(
        cargo("metadata", "--format-version", "1", "--filter-platform", target)
    )
    sources = {
        (crate["name"], crate["version"]): Path(crate["manifest_path"]).parent
        for crate in metadata["packages"]
    }

    for crate in notice_rows(document):
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
            assert normalized(path) in document, (crate, path.name)
        if not shipped:
            assert f"- **{crate[0]} {crate[1]}** (" in document, crate


def test_local_firmware_notices_cover_linked_archives(
    local_firmware_notices: FirmwareNotices | None,
) -> None:
    if local_firmware_notices is None:
        pytest.skip("Pinned firmware has not been built on this machine")

    assert template_fields(local_firmware_notices.document) == []
    assert set(local_firmware_notices.archives) >= {
        "libFrameworkArduino.a",
        "libBLE.a",
        "libc.a",
    }


def test_new_firmware_library_requires_notice_review(tmp_path: Path) -> None:
    linker_map = tmp_path / "new.map"
    linker_map.write_text(
        "Archive member included to satisfy reference by file (symbol)\n\n"
        "/p/libFrameworkArduino.a(main.cpp.o)\n                              (x)\n"
        "/p/libnew.a(new.c.o)\n                              main.o (x)\n"
    )

    with pytest.raises(notices.NoticeError, match=r"newly linked archives: libnew\.a$"):
        notices.firmware_notices(tmp_path, tmp_path, linker_map)


def test_packaged_manual_links_resolve_offline(packaged_manual: Path) -> None:
    assert (packaged_manual / notices.NOTICES).is_file()
    assert not (packaged_manual / "CLAUDE.md").exists()
    assert not (packaged_manual / "CONTRIBUTING.md").exists()
    for document in packaged_manual.rglob("*.md"):
        for target in re.findall(r"\[[^\]]*\]\(([^\s)]+)\)", document.read_text()):
            if ":" in target or target.startswith("#"):
                continue
            assert (document.parent / target.split("#")[0]).exists(), (
                document.relative_to(packaged_manual),
                target,
            )


def git(repository: Path, *args: str) -> str:
    return subprocess.check_output(
        ["git", *args], cwd=repository, text=True, stderr=subprocess.PIPE
    ).strip()


@pytest.fixture
def release_repo(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    monkeypatch.setattr(package, "ROOT", tmp_path)
    git(tmp_path, "init")
    git(tmp_path, "config", "user.name", "Package Check")
    git(tmp_path, "config", "user.email", "package-check@example.invalid")
    (tmp_path / "source").write_text("first\n")
    git(tmp_path, "add", "source")
    git(tmp_path, "commit", "-m", "first")
    git(tmp_path, "tag", "v1.2.3")
    return tmp_path


def test_release_tag_identifies_the_clean_head(release_repo: Path) -> None:
    assert package.release_identity("1.2.3", "v1.2.3") == (
        git(release_repo, "rev-parse", "HEAD"),
        False,
    )


@pytest.mark.usefixtures("release_repo")
def test_release_tag_must_match_package_version() -> None:
    with pytest.raises(SystemExit, match=r"Cargo.toml version: v1\.2\.3"):
        package.release_identity("1.2.3", "v9.9.9")


def test_dirty_checkout_requires_an_untagged_local_build(release_repo: Path) -> None:
    (release_repo / "source").write_text("second\n")

    with pytest.raises(SystemExit, match="clean source checkout"):
        package.release_identity("1.2.3", "v1.2.3")
    assert package.release_identity("1.2.3", None)[1]


def test_release_tag_must_point_to_the_checked_out_commit(release_repo: Path) -> None:
    (release_repo / "source").write_text("second\n")
    git(release_repo, "add", "source")
    git(release_repo, "commit", "-m", "second")

    with pytest.raises(SystemExit, match="checked-out commit"):
        package.release_identity("1.2.3", "v1.2.3")
