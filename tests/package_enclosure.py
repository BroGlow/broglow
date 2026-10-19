#!/usr/bin/env python3
"""Check isolated CAD staging and release guards without CAD dependencies or hardware."""

import importlib.util
import hashlib
import json
from zipfile import ZipFile
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
    for path in package.SOURCE_FILES:
        target = root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("source\n")
    model = root / package.MODEL
    files = package.source_files()
    # A clean release contains only the five production meshes, never coupons.
    stage = package.prepare_source(root / "dist/magnetic", files)
    magnetic_model = stage / package.MAGNETIC
    records = []
    for name in package.PARTS:
        path = magnetic_model / "stl" / f"{name}.stl"
        path.parent.mkdir(exist_ok=True)
        path.write_bytes(b"reviewed mesh")
        records.append(
            {
                "file": f"stl/{name}.stl",
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            }
        )
        step = magnetic_model / "step" / f"{name}.step"
        step.parent.mkdir(exist_ok=True)
        step.write_bytes(b"STEP")
    (magnetic_model / "stl/06_old_coupon.stl").write_bytes(b"stale")
    (magnetic_model / "slicer-validation.json").write_text(
        json.dumps({"parts": records})
    )
    (magnetic_model / "validation.json").write_text('{"status": "passed"}')
    (stage / "BUILD.json").write_text("{}")
    archive = package.package_magnetic(stage, root / "dist", "1.2.3")
    with ZipFile(archive) as zipped:
        # stl/ and step/ print files plus provenance only.
        assert archive.name == "broglow-models-v1.2.3.zip", archive
        assert sorted(zipped.namelist()) == sorted(
            [f"stl/{name}.stl" for name in package.PARTS]
            + [f"step/{name}.step" for name in package.PARTS]
            + ["BUILD.json"]
        ), zipped.namelist()
    path.write_bytes(b"changed mesh")
    rejected(
        lambda: package.package_magnetic(stage, root / "dist", "1.2.3"),
        "Stale magnetic slicer evidence",
    )
    (model / "private.py").write_text("private analysis")
    (model / "old.STL").write_bytes(b"stale geometry")
    (model / "validation.json").write_text("stale report")
    (root / ".gitignore").write_bytes(
        (Path(__file__).resolve().parents[1] / ".gitignore").read_bytes()
    )
    assert package.source_files() == files
    assert all(
        path.name not in ("private.py", "old.STL", "validation.json") for path in files
    )
    stage = package.prepare_source(root / "dist/enclosure", files)
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
    ]
    retired = [
        "retired/old.ZIP",
        "elsewhere/part.StP",
        "outside/mesh.sTl",
        "view.STEP",
        "outside/project.3MF",
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
        "docs/reintroduced.md",
    ] + retired_media
    preserved = [
        "models/magnetic/slicer-validation.json",
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
        "site/assets/screenshots/new-screen.png",
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
    "PASS: stl/step models package, release guards, folder ignores and forced-artifact rejection"
)
