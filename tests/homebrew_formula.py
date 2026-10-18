#!/usr/bin/env python3
"""Check the rendered Homebrew formula against release archive checksums."""

import importlib.util
from pathlib import Path
import tempfile

spec = importlib.util.spec_from_file_location(
    "homebrew_formula",
    Path(__file__).resolve().parents[1] / "scripts/homebrew-formula.py",
)
formula = importlib.util.module_from_spec(spec)
spec.loader.exec_module(formula)

with tempfile.TemporaryDirectory(prefix="broglow-formula-") as folder:
    checksums = Path(folder)
    digests = {"aarch64-apple-darwin": "a" * 64, "x86_64-apple-darwin": "b" * 64}
    for target, digest in digests.items():
        archive = f"broglow-v1.2.3-{target}.tar.gz"
        (checksums / f"{archive}.sha256").write_text(f"{digest}  {archive}\n")
    text = formula.render(
        "1.2.3",
        checksums,
        "https://github.com/example/lamp",
        "https://github.com/example/lamp/releases/download/v1.2.3/",
    )
    assert 'version "' not in text, "Homebrew infers the version from the URL"
    assert (
        'url "https://github.com/example/lamp/releases/download/v1.2.3/broglow-v1.2.3-aarch64-apple-darwin.tar.gz"'
        in text
    )
    assert f'sha256 "{"a" * 64}"' in text and f'sha256 "{"b" * 64}"' in text
    assert text.index("on_arm") < text.index("a" * 64) < text.index("on_intel")
    assert "broglow install" in text and 'bin.install "bin/broglow"' in text
    # A checksum line for another file name is never accepted.
    (checksums / "broglow-v1.2.3-x86_64-apple-darwin.tar.gz.sha256").write_text(
        f"{'b' * 64}  other.tar.gz\n"
    )
    try:
        formula.render("1.2.3", checksums, "https://x", "https://x/")
    except SystemExit as error:
        assert "Not a sha256sum line" in str(error)
    else:
        raise AssertionError("Accepted a checksum for another archive")
print("PASS: Homebrew formula renders release URLs, checksums and setup caveat")
