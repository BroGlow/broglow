#!/usr/bin/env python3
"""Export the current CAD appearance to an ordinary GLB without changing CAD outputs.

Run with the CAD Python environment (models/clawlight/requirements.txt) and the
installed CAD skill. Only gen_step() is invoked; generator main/export_all is not.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.metadata
import json
import math
from pathlib import Path
import shutil
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
MODEL = ROOT / "models/clawlight"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def glb_facts(path):
    data = path.read_bytes()
    assert struct.unpack_from("<4sII", data) == (b"glTF", 2, len(data))
    length, kind = struct.unpack_from("<II", data, 12)
    assert kind == 0x4E4F534A
    document = json.loads(data[20 : 20 + length])
    binary_length, binary_kind = struct.unpack_from("<II", data, 20 + length)
    assert binary_kind == 0x004E4942 and 28 + length + binary_length == len(data)
    binary = data[28 + length :]
    assert document["asset"]["version"] == "2.0"
    assert "STEP_topology" not in document.get("extensionsUsed", [])
    assert len(document["buffers"]) == 1 and "uri" not in document["buffers"][0]
    assert all(
        "uri" not in item for item in document.get("images", [])
    ), "External image URL"
    points, labels, triangles = [], [], 0

    def visit(index, offset=(0, 0, 0)):
        nonlocal triangles
        node = document["nodes"][index]
        assert not any(
            key in node for key in ("rotation", "scale", "translation")
        ), "Review new exporter transform"
        matrix = node.get("matrix", [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1])
        assert (
            matrix[:12] == [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0] and matrix[15] == 1
        ), "Review rotated exporter transform"
        offset = [offset[i] + matrix[12 + i] for i in range(3)]
        if "mesh" in node:
            labels.append(node["name"])
            for primitive in document["meshes"][node["mesh"]]["primitives"]:
                assert primitive.get("mode", 4) == 4
                accessor = document["accessors"][primitive["attributes"]["POSITION"]]
                assert (
                    accessor["componentType"] == 5126
                    and accessor["type"] == "VEC3"
                    and "sparse" not in accessor
                )
                view = document["bufferViews"][accessor["bufferView"]]
                start = view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
                for index in range(accessor["count"]):
                    point = struct.unpack_from(
                        "<3f", binary, start + index * view.get("byteStride", 12)
                    )
                    assert all(math.isfinite(value) for value in point)
                    points.append([point[i] + offset[i] for i in range(3)])
                count = (
                    document["accessors"][primitive["indices"]]["count"]
                    if "indices" in primitive
                    else accessor["count"]
                )
                assert count % 3 == 0
                triangles += count // 3
        for child in node.get("children", []):
            visit(child, offset)

    for node in document["scenes"][document.get("scene", 0)]["nodes"]:
        visit(node)
    return {
        "glb_bounds_m": [
            [fn(p[i] for p in points) for i in range(3)] for fn in (min, max)
        ],
        "labels": labels,
        "triangles": triangles,
        "materials": len(document.get("materials", [])),
        "bytes": len(data),
        "glb_sha256": digest(data),
    }


def check(metadata, facts, allow_missing_sources=False):
    sources_checked = not allow_missing_sources or MODEL.exists()
    if sources_checked:
        for name, sha in metadata["source_sha256"].items():
            source = ROOT / name
            assert source.is_file(), f"Incomplete CAD sources: missing {name}"
            assert (
                digest(source.read_bytes()) == sha
            ), f"Stale web model: {name} changed; explicitly re-export current CAD"
    assert all(
        metadata[key] == value for key, value in facts.items()
    ), "GLB and provenance differ"
    low, high = (metadata["cad_bounds_mm"][key] for key in ("min", "max"))
    expected = [
        [low[0] / 1000, low[2] / 1000, -high[1] / 1000],
        [high[0] / 1000, high[2] / 1000, -low[1] / 1000],
    ]
    tolerance = metadata["exporter"]["linear_deflection_mm"] / 1000 + 1e-7
    assert all(
        abs(actual - want) <= tolerance
        for row, expected_row in zip(facts["glb_bounds_m"], expected)
        for actual, want in zip(row, expected_row)
    ), "Unexpected GLB axes or scale"
    assert metadata["up_axis"] == "Y" and metadata["units"] == "metres"
    assert facts["bytes"] < 2_000_000, "Review web mesh size before publishing"
    return sources_checked


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--cad-skill", type=Path, default=Path.home() / ".agents/skills/cad"
    )
    parser.add_argument("--mesh-tolerance", type=float, default=0.08)
    parser.add_argument("--mesh-angular-tolerance", type=float, default=0.2)
    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify the committed GLB/provenance using only Python stdlib.",
    )
    args = parser.parse_args()
    destination = ROOT / "site/assets"
    if args.check:
        sources_checked = check(
            json.loads((destination / "clawlight-model.json").read_text()),
            glb_facts(destination / "clawlight.glb"),
            allow_missing_sources=True,
        )
        print("PASS: GLB structure, hash, meter scale/Y-up and self-contained assets")
        print(
            "PASS: CAD source hashes"
            if sources_checked
            else "CAD sources absent (app-only export): source hashes not checked"
        )
        return
    assert args.mesh_tolerance > 0 and args.mesh_angular_tolerance > 0
    snapshot = (
        ROOT
        / ".recovery/landing-model"
        / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
    )
    snapshot.mkdir(parents=True)
    names = ("appearance.py", "clawlight.py", "hand_joint.py", "requirements.txt")
    inputs = {name: (MODEL / name).read_bytes() for name in names}
    for name, data in inputs.items():
        (snapshot / name).write_bytes(data)
    assert all(
        (MODEL / name).read_bytes() == data for name, data in inputs.items()
    ), "CAD changed while copying; retry"
    step = snapshot / "appearance.step"
    subprocess.run(
        [
            sys.executable,
            str(args.cad_skill / "scripts/step"),
            str(snapshot / "appearance.py"),
            "--output",
            str(step),
            "--glb",
            "clawlight.glb",
            "--mesh-tolerance",
            str(args.mesh_tolerance),
            "--mesh-angular-tolerance",
            str(args.mesh_angular_tolerance),
        ],
        cwd=ROOT,
        check=True,
    )
    inspection = subprocess.check_output(
        [
            sys.executable,
            str(args.cad_skill / "scripts/inspect"),
            "refs",
            str(step.relative_to(ROOT)),
            "--facts",
            "--planes",
            "--positioning",
        ],
        cwd=ROOT,
        text=True,
    )
    (snapshot / "inspection.json").write_text(inspection)
    facts = json.loads(inspection)
    assert facts["ok"], "CAD inspection failed"
    summary = facts["tokens"][0]["summary"]
    glb = snapshot / "clawlight.glb"
    mesh = glb_facts(glb)
    assert (
        len(mesh["labels"]) == summary["leafOccurrenceCount"]
    ), "Assembly inventory changed; review labels"
    metadata = {
        "source": "models/clawlight/appearance.py",
        "status": "release_candidate",
        "purpose": "Web/AR visualization of the enclosure release candidate",
        "source_sha256": {
            f"models/clawlight/{name}": digest(content)
            for name, content in inputs.items()
        },
        "source_changed_during_export": any(
            (MODEL / name).read_bytes() != content for name, content in inputs.items()
        ),
        "exporter": {
            "tool": "cad/scripts/step --glb",
            "build123d": importlib.metadata.version("build123d"),
            "linear_deflection_mm": args.mesh_tolerance,
            "angular_deflection_radians": args.mesh_angular_tolerance,
        },
        "cad_bounds_mm": summary["bounds"],
        "up_axis": "Y",
        "units": "metres",
        **mesh,
    }
    check(metadata, mesh)
    shutil.copy2(glb, destination / "clawlight.glb")
    (destination / "clawlight-model.json").write_text(
        json.dumps(metadata, indent=2) + "\n"
    )
    print(json.dumps(metadata, indent=2))
    print(f"Snapshot STEP and inspection: {snapshot}")


if __name__ == "__main__":
    main()
