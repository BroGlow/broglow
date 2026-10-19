"""Homebrew release URLs and architecture-specific archive checksums."""

import importlib.util
from pathlib import Path
import re

import pytest

spec = importlib.util.spec_from_file_location(
    "homebrew_formula",
    Path(__file__).resolve().parents[1] / "scripts/homebrew-formula.py",
)
formula = importlib.util.module_from_spec(spec)
spec.loader.exec_module(formula)

VERSION = "1.2.3"
REPOSITORY = "https://github.com/example/lamp"
URL_BASE = f"{REPOSITORY}/releases/download/v{VERSION}/"
ARCHIVES = {
    "arm": ("aarch64-apple-darwin", "a" * 64),
    "intel": ("x86_64-apple-darwin", "b" * 64),
}


@pytest.fixture
def checksums(tmp_path: Path) -> Path:
    for target, digest in ARCHIVES.values():
        archive = f"broglow-v{VERSION}-{target}.tar.gz"
        (tmp_path / f"{archive}.sha256").write_text(f"{digest}  {archive}\n")
    return tmp_path


def test_formula_selects_architecture_archives_and_explains_setup(
    checksums: Path,
) -> None:
    text = formula.render(VERSION, checksums, REPOSITORY, URL_BASE)

    for architecture, (target, digest) in ARCHIVES.items():
        url = f"{URL_BASE}broglow-v{VERSION}-{target}.tar.gz"
        assert re.search(
            rf'on_{architecture} do\s+url "{re.escape(url)}"\s+sha256 "{digest}"', text
        ), architecture
    assert 'bin.install "bin/broglow"' in text
    assert "broglow install" in text


def test_formula_rejects_a_checksum_for_another_archive(checksums: Path) -> None:
    target, digest = ARCHIVES["intel"]
    (checksums / f"broglow-v{VERSION}-{target}.tar.gz.sha256").write_text(
        f"{digest}  other.tar.gz\n"
    )

    with pytest.raises(SystemExit, match="Not a sha256sum line for broglow"):
        formula.render(VERSION, checksums, REPOSITORY, URL_BASE)


def test_versioned_formula_is_a_keg_only_alternate(checksums: Path) -> None:
    latest = formula.render(VERSION, checksums, REPOSITORY, URL_BASE)
    pinned = formula.render(VERSION, checksums, REPOSITORY, URL_BASE, versioned=True)

    assert latest.startswith("class Broglow < Formula")
    assert "keg_only" not in latest
    assert pinned.startswith("class BroglowAT123 < Formula")
    assert "keg_only :versioned_formula" in pinned
    assert (
        pinned.split("keg_only :versioned_formula\n\n")[1]
        == latest.split('license "MIT"\n\n')[1]
    )
