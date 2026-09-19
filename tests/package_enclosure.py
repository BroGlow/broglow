#!/usr/bin/env python3
"""Check isolated CAD staging and release guards without CAD dependencies or hardware."""

import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile

spec = importlib.util.spec_from_file_location(
    "package_enclosure",
    Path(__file__).resolve().parents[1] / "scripts/package-enclosure.py",
)
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


def rejected(action, message):
    try:
        action()
    except SystemExit as error:
        assert message in str(error), error
    else:
        raise AssertionError("Unsafe input accepted")


with tempfile.TemporaryDirectory(prefix="broglow-enclosure-check-") as folder:
    package.ROOT = root = Path(folder).resolve() / "checkout"
    root.mkdir()
    for path in {
        Path("LICENSE"),
        *(Path(name) for name in package.prints.REFERENCE_FILES),
        *(package.MODEL / name for name in package.CAD_FILES),
    }:
        target = root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("source\n")
    model = root / package.MODEL
    (model / "PRINT_GUIDE.en.md").write_text("[Dimensions](preview/dimensions.png)\n")
    # A fresh checkout has no preview directory; staging creates it for dimensions.
    files = package.source_files()
    stage = package.prepare_source(root / "dist/enclosure", files)
    assert (stage / package.MODEL / "preview").is_dir()
    (model / "preview").mkdir()
    (model / "preview/review.png").write_bytes(b"review")
    (model / "preview/dimensions.png").write_bytes(b"stale dimensions")
    (model / "private.py").write_text("private analysis")
    (model / "old.STL").write_bytes(b"stale geometry")
    (model / "validation.json").write_text("stale report")
    (root / ".gitignore").write_bytes(
        (Path(__file__).resolve().parents[1] / ".gitignore").read_bytes()
    )
    assert package.source_files() == files
    assert not any("preview" in path.parts for path in files)
    assert all(
        path.name not in ("private.py", "old.STL", "validation.json", "dimensions.png")
        for path in files
    )
    stage = package.prepare_source(root / "dist/enclosure", files)
    assert not any((stage / package.MODEL / "preview").iterdir())
    assert not (stage / package.MODEL / "old.STL").exists()
    assert not (stage / package.MODEL / "private.py").exists()
    (stage / package.MODEL / "stale.step").write_bytes(b"old generated part")
    assert package.prepare_source(root / "dist/enclosure", files) == stage
    assert not (stage / package.MODEL / "stale.step").exists()
    unknown = root / "dist/unknown/source"
    unknown.mkdir(parents=True)
    rejected(lambda: package.prepare_source(unknown.parent, files), "unknown directory")
    blocked = root.parent / "source"
    original = package.ROOT
    package.ROOT = blocked
    rejected(
        lambda: package.prepare_source(root.parent, files),
        "replace the source checkout",
    )
    package.ROOT = original
    assert package.identity("1.2.3", None) == (None, None)
    rejected(lambda: package.identity("1.2.3", "v1.2.3"), "requires a Git checkout")

    def git(*args):
        return subprocess.check_output(
            ["git", *args], cwd=root, text=True, stderr=subprocess.PIPE
        ).strip()

    git("init")
    git("config", "user.name", "Enclosure Check")
    git("config", "user.email", "enclosure-check@example.invalid")
    git("add", ".")
    git("commit", "-m", "source")
    git("tag", "v1.2.3")
    guard = root / "scripts/check-generated-artifacts.py"
    guard.write_bytes(
        (
            Path(__file__).resolve().parents[1] / "scripts/check-generated-artifacts.py"
        ).read_bytes()
    )
    retired_media = [
        "site/assets/assembly.png",
        "site/vendor/model-viewer-4.3.1.min.js",
        "references/clawlight/claw-light-ar.usdz",
        "references/clawlight/claw-light-ar.glb",
        "references/clawlight/store-photo.jpg",
        "references/clawlight/product-photo.JPG",
        "references/clawlight/ar-reference-views.png",
        "docs/screenshots/opencode-tui.png",
        "docs/screenshots/opencode-popover.png",
    ]
    retired = [
        "retired/old.ZIP",
        "elsewhere/part.StP",
        "outside/mesh.sTl",
        "view.STEP",
        "models/broglow/cad-inspection.json",
        "models/broglow/coupon-inspection.json",
        "models/broglow/USB-COUPON-INSPECTION.JSON",
        "models/broglow/preview/review.png",
        "models/broglow/preview/arbitrary.PNG",
        "models/broglow/preview/nested/draft.SVG",
        "models/broglow/preview/nested/recipe.py",
        "models/broglow/preview/snapshot-job.json",
        "models/broglow/preview/base-fit-snapshot-job.json",
        "models/broglow/preview/cover-chamfers-snapshot-job.json",
        "models/broglow/preview/flat-floor-snapshot-job.json",
        "models/broglow/preview/TIGHT-USB-SNAPSHOT-JOB.JSON",
    ] + retired_media
    preserved = [
        "models/broglow/parts.json",
        "models/broglow/validation.json",
        "models/broglow/slicer-validation.json",
        "models/broglow/slicer-result.json",
        "site/assets/broglow-model.json",
        "site/vendor/bloom-provenance.json",
        "references/clawlight/ar-component-bounds.json",
        "references/clawlight/ar-model-metadata.json",
        "references/clawlight/sources.json",
        "assets/icons/broglow-green.png",
        "assets/icons/broglow-none.png",
        "assets/icons/broglow-red.png",
        "assets/icons/broglow-yellow.png",
        "site/assets/broglow.png",
        "site/assets/assembly.glb",
        "site/assets/assembly-model.json",
        "site/assets/broglow.glb",
        "site/vendor/model-viewer-with-effects.js",
        "site/vendor/model-viewer-module-4.3.1.min.js",
        "site/vendor/model-viewer-effects-1.5.0.min.js",
        "site/vendor/three-0.183.0/three.module.min.js",
        "site/vendor/three-0.183.0/three.core.min.js",
        "site/vendor/model-viewer-LICENSE",
        "site/vendor/qrcode-generator-2.0.4.js",
        "references/clawlight/new-photo.jpg",
        "docs/screenshots/new-screen.png",
        "site/guide/assets/favicon.svg",
        "config.json",
        "tests/fixtures/cad-inspection.json",
    ]
    for name in preserved:
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("{}\n")
    git("add", *preserved)
    for name in retired:
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"generated")
    assert set(git("check-ignore", *retired).splitlines()) == set(retired)
    assert not subprocess.run(
        ["git", "check-ignore", "--no-index", *preserved], cwd=root, capture_output=True
    ).stdout
    case_variant = "MODELS/BROGLOW/PREVIEW/nested/forced.PNG"
    path = root / case_variant
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"generated")
    git("add", "-f", *retired)
    case_variants = [case_variant, *(name.upper() for name in retired_media)]
    assert set(
        git("check-ignore", "--no-index", *case_variants[1:]).splitlines()
    ) == set(case_variants[1:])
    for name in case_variants:
        git(
            "update-index",
            "--add",
            "--cacheinfo",
            "100644",
            git("hash-object", "-w", str(path)),
            name,
        )
    retired.extend(case_variants)
    rejected_artifacts = subprocess.run(
        [sys.executable, str(guard)], text=True, capture_output=True
    )
    assert rejected_artifacts.returncode != 0
    assert all(
        name in rejected_artifacts.stderr for name in retired
    ), rejected_artifacts.stderr
    git("rm", "--cached", *retired)
    accepted = subprocess.run(
        [sys.executable, str(guard)], text=True, capture_output=True
    )
    assert accepted.returncode == 0, accepted.stderr
    assert "models/broglow/preview/review.png" not in git("ls-files")
    for name in retired + ["models/broglow/old.STL"]:
        (root / name).unlink(missing_ok=True)
    git("add", ".")
    git("commit", "-m", "check source tracking policy")
    git("tag", "-f", "v1.2.3")
    assert package.identity("1.2.3", "v1.2.3") == (git("rev-parse", "HEAD"), False)
    (model / "broglow.py").write_text("changed source\n")
    rejected(lambda: package.identity("1.2.3", "v1.2.3"), "clean source")
    (model / "broglow.py").unlink()
    (model / "broglow.py").symlink_to(model / "private.py")
    rejected(package.source_files, "unsafe source input")
print(
    "PASS: preview-free staging, release guards, folder ignores and forced-artifact rejection"
)
