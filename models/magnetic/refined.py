"""Production magnetic enclosure, mm; X toward lamp, Y rearward, Z upward.

round3x2.py is the release entry point. Geometry preserves the approved fit5.
"""

from functools import lru_cache
from pathlib import Path
import sys
from build123d import Axis, Compound, Face, Wire, extrude, chamfer, fillet

sys.path.insert(0, str(Path(__file__).resolve().parent))
import magnetic as old

base = old.base

OUT = old.OUT / "refinement"
FACE_X = 26.0
RECESS = 0.10  # Thin magnets benefit from a small, controlled magnetic gap.
DEPTH = 1.30
BACK = 0.80
POCKET_ALLOWANCE = 0.30  # Total size allowance; tune with printed coupons.
VARIANTS = {
    "round2": {
        "size": (3.0, 3.0),
        "ys": (7.0, 16.5),
        "z": 24.3,
        "seat": 0.0,
        "depth": 2.3,
        "recess": 0.0,
        "retainer_edge_x": 29.5,
        "thickness": 2.0,
        "pocket_allowance": 0.05,
        "roof_bridge": 0.4,
    },
}


def envelope(x0, x1, y, z, width, height):
    return base.box(
        x0, x1, y - width / 2, y + width / 2, z - height / 2, z + height / 2
    )


def magnet_shape(variant, x0, x1, y, allowance=0):
    v = VARIANTS[variant]
    w, h = v["size"]
    if variant in ("tiny", "round", "round2"):
        return old.cx(x0, x1, y, v["z"], w + allowance)
    return envelope(x0, x1, y, v["z"], w + allowance, h + allowance)


def pocket(variant, x0, x1, y, mascot):
    v = VARIANTS[variant]
    if variant in ("tiny", "round", "round2"):
        return old.pocket(
            x0,
            x1,
            y,
            v["z"],
            -1 if mascot else 1,
            v["size"][0] + v.get("pocket_allowance", POCKET_ALLOWANCE),
            v.get("roof_bridge", old.ROOF_BRIDGE),
        )
    # Print direction is Y; the bridge spans only 2.3 mm along Z.
    return magnet_shape(variant, x0, x1, y, POCKET_ALLOWANCE)


def tip(variant):
    # Rising finger, not a rectangular mounting pad. Body and wrist stay intact.
    extra = 0.6 if variant in ("round", "round2") else 0.0
    points = [
        (22.1, 18.7),
        (22.75, 18.7),
        (26, 21.95 - extra / 2),
        (26, 26.65 + extra / 2),
        (25.0, 26.65 + extra / 2),
        (22.9, 26.65) if variant == "round2" else (23.8, 25.7 + extra / 2),
        (22.1, 22.7),
    ]
    s = base.profile(points, 3.25, 20.5)
    # Smaller edge rounding preserves the .8-mm rim at the round2 mating face.
    s = fillet(s.edges().filter_by(Axis.Y), radius=0.2 if variant == "round2" else 0.45)
    s = chamfer(
        base.y_face_edges(s, 3.25) + base.y_face_edges(s, 20.5),
        length=0.15 if variant == "round2" else 0.26,
    )
    return s


def seat_outline(x0, x1, allowance=0):
    # Very shallow recess with a lead-in: no snap, dovetail, or undercut.
    s = envelope(x0, x1, 11.875, 24.1, 15.7 + allowance, 3.4 + allowance)
    s = fillet(s.edges().filter_by(Axis.X), radius=0.65)
    return chamfer(s.faces().sort_by(Axis.X)[-1].edges(), length=0.15)


@lru_cache(maxsize=None)
def parts(variant="round2"):
    v = VARIANTS[variant]
    hand = base.mascot(hand_connection=False) + tip(variant)
    tower = base.tower(hand_connection=False)
    # Every added lamp feature is inward of the original X=26 outside wall.
    face = FACE_X + v["seat"]
    depth = v.get("depth", DEPTH)
    end = face + depth + BACK
    root = base.UPRIGHT_LEFT + base.UPRIGHT_WALL - 0.1
    for y in v["ys"]:
        w, h = v["size"]
        y0, y1 = y - w / 2 - 0.85, y + w / 2 + 0.85
        if variant == "round2":
            y1 = y + 3.4  # Preserve >=.8 mm around the pointed printable roof.
        z0, z1 = v["z"] - h / 2 - 1, v["z"] + h / 2 + 1
        xy = [(26, y0), (root, y0 - (end - root)), (end, y0), (end, y1), (26, y1)]
        tower += extrude(
            Face(Wire.make_polygon([(x, yy, z0) for x, yy in xy], close=True)),
            amount=z1 - z0,
            dir=(0, 0, 1),
        )
    if v["seat"]:
        hand += seat_outline(FACE_X - 0.1, face)
        # .2 mm per-side fit clearance plus a 0.1 mm axial relief.
        tower -= seat_outline(FACE_X - 0.1, face + 0.1, 0.4)
    for y in v["ys"]:
        hand -= pocket(variant, face - depth, face + 0.1, y, True)
        tower -= pocket(variant, face - 0.1, face + depth, y, False)
    retainer = base.led_retainer()
    if variant == "round2":
        # Keep the fit4 relief; shallower pockets now leave .4-mm clearance,
        # including the 1.3-mm downward release and rearward removal path.
        retainer -= base.profile(
            [
                (29.0, 21.1),
                (v["retainer_edge_x"], 21.4),
                (v["retainer_edge_x"], 28.5),
                (29.0, 28.8),
            ],
            base.LED_SUPPORT_Y - 0.1,
            base.RETAINER_BACK_Y + 0.1,
        )
        retainer = base.label(retainer, "black_LED_retainer_magnet_depth_2p4_clearance")
    return {
        "01_mascot_magnetic_orange": base.label(
            hand, "Mascot_slim_" + variant, base.ORANGE
        ),
        "02_eye_black_x2": base.eye(-12.5),
        "03_tower_magnetic_black": base.label(tower, "Traffic_Light_flush_" + variant),
        "04_rear_cover_black": base.rear_cover(),
        "05_LED_retainer_black": retainer,
    }


def magnets(variant="round2"):
    v = VARIANTS[variant]
    face = FACE_X + v["seat"]
    recess = v.get("recess", RECESS)
    thickness = v.get("thickness", 1.0)
    result = {}
    for i, y in enumerate(v["ys"], 1):
        for side, x0 in [
            ("mascot", face - recess - thickness),
            ("tower", face + recess),
        ]:
            name = f"REFERENCE_ONLY_{side}_magnet_{i}_{variant}"
            result[name] = base.label(
                magnet_shape(variant, x0, x0 + thickness, y), name, (0.76, 0.78, 0.81)
            )
    return result


def assembly(variant="round2", explode=0):
    return Compound(
        children=[
            s.translate((-explode if "mascot" in n or "eye" in n else explode, 0, 0))
            for n, s in (
                parts(variant)
                | {"02_eye_right_black": base.eye(12.5)}
                | base.electronics()
                | magnets(variant)
            ).items()
        ],
        label="BroGlow_slim_" + variant,
    )


def gen_step():
    return assembly()


def validate(output, selected="round2"):
    """One end-to-end check: BREP fit, real STEP roundtrip, then printable meshes."""
    import itertools
    import json
    import hashlib
    import trimesh
    from build123d import import_step

    records = {}
    for variant in (selected,):
        v = VARIANTS[variant]
        depth = v.get("depth", DEPTH)
        recess = v.get("recess", RECESS)
        p = parts(variant)
        physical = p | {"02_eye_right_black": base.eye(12.5)} | magnets(variant)
        for n, s in physical.items():
            assert s.is_valid and len(s.solids()) == 1, (variant, n)
        for (an, a), (bn, b) in itertools.combinations(physical.items(), 2):
            old.clear(a, b, variant + "/" + an + "/" + bn)
        hand, tower = p["01_mascot_magnetic_orange"], p["03_tower_magnetic_black"]
        for n, s in base.electronics().items():
            old.clear(s, tower, variant + "/electronics/" + n)
            old.clear(s, hand, variant + "/electronics/" + n)
        retainer = p["05_LED_retainer_black"]
        for drop in (0, 0.65, base.RETAINER_RELEASE_DROP):
            old.clear(
                tower, retainer.translate((0, 0, -drop)), variant + "/retainer/drop"
            )
        for rear in (1, 3, 6, 12, 24):
            old.clear(
                tower,
                retainer.translate((0, rear, -base.RETAINER_RELEASE_DROP)),
                variant + "/retainer/slide",
            )
        face = FACE_X + v["seat"]
        for y in v["ys"]:
            for mascot, s in ((True, hand), (False, tower)):
                x0, x1 = (
                    (face - depth - BACK, face - depth)
                    if mascot
                    else (face + depth, face + depth + BACK)
                )
                backing = (
                    pocket(variant, x0, x1, y, mascot)
                    if variant == "round2"
                    else magnet_shape(variant, x0, x1, y, POCKET_ALLOWANCE)
                )
                assert (backing - s).volume < 1e-5, (variant, "backing")
                for delta in (0, 0.5, 1.5, 3):
                    # Measured Ø3.04 maximum; reserve .10 mm behind the magnet for glue.
                    thickness = depth - recess - 0.1 if variant == "round2" else 1.1
                    a, b = (
                        (face - recess - thickness + delta, face - recess + delta)
                        if mascot
                        else (face + recess - delta, face + recess + thickness - delta)
                    )
                    old.clear(
                        s,
                        magnet_shape(
                            variant, a, b, y, 0.04 if variant == "round2" else 0.1
                        ),
                        variant + "/magnet insertion",
                    )
                if variant == "flat":
                    # At least .8 mm material surrounding the entire rectangular seat.
                    a, b = (
                        (face - DEPTH, face - 0.01)
                        if mascot
                        else (face + 0.01, face + DEPTH)
                    )
                    collar = envelope(
                        a,
                        b,
                        y,
                        v["z"],
                        5 + POCKET_ALLOWANCE + 1.6,
                        2 + POCKET_ALLOWANCE + 1.6,
                    ) - pocket(variant, a - 0.1, b + 0.1, y, mascot)
                    assert (collar - s).volume < 1e-5, "flat pocket wall"
                if variant == "round2":
                    # Both horizontal bores must close upward in the exported print pose.
                    print_up = -1 if mascot else 1
                    bore = pocket(variant, 0, depth, y, mascot).rotate(
                        Axis.X, print_up * 90
                    )
                    diameter = v["size"][0] + v["pocket_allowance"]
                    peak = diameter / 2 * 2**0.5 - v["roof_bridge"] / 2
                    assert abs(bore.bounding_box().max.Z - print_up * y - peak) < 1e-6
                    a, b = (
                        (face - depth, face - 0.01)
                        if mascot
                        else (face + 0.01, face + depth)
                    )
                    collar = old.pocket(
                        a,
                        b,
                        y,
                        v["z"],
                        -1 if mascot else 1,
                        diameter + 1.6,
                        v["roof_bridge"],
                    ) - pocket(variant, a - 0.1, b + 0.1, y, mascot)
                    assert (collar - s).volume < 1e-5, (
                        variant,
                        "minimum .8 mm collar",
                        mascot,
                        (collar - s).volume,
                    )
        if variant == "round2":
            for i in range(1, len(v["ys"]) + 1):
                pair = magnets(variant)
                a = pair[f"REFERENCE_ONLY_mascot_magnet_{i}_{variant}"]
                b = pair[f"REFERENCE_ONLY_tower_magnet_{i}_{variant}"]
                assert abs(a.bounding_box().max.X - face) < 1e-6
                assert abs(b.bounding_box().min.X - face) < 1e-6
                assert a.distance_to(b) < 1e-6, "magnet faces must touch"
            bosses = tower & base.box(
                28.6, face + depth + BACK + 0.01, 0, 21, 21.8, 26.8
            )
            for drop in (0, 0.65, base.RETAINER_RELEASE_DROP):
                assert (
                    bosses.distance_to(retainer.translate((0, 0, -drop))) >= 0.4 - 1e-6
                )
        for dx in (0, 0.1, 0.2, 0.4, 1, 3, 10):
            old.clear(hand.translate((-dx, 0, 0)), tower, variant + "/straight release")
        # Added lamp material cannot create any external podium above the base.
        outside = base.box(-40, FACE_X - 1e-5, -10, 40, base.BASE_HEIGHT + 1, 50)
        assert old.overlap(tower, outside) < 1e-5, variant + "/external projection"
        assert (
            abs(hand.bounding_box().min.Z) < 1e-6
            and abs(tower.bounding_box().min.Z) < 1e-6
        )
        records[variant] = {
            "valid_single_solids": True,
            "magnet_count": len(v["ys"]) * 2,
            "magnet_mm": [v["size"][0], v["size"][1], v.get("thickness", 1)],
            "pocket_mm": [
                v["size"][0] + v.get("pocket_allowance", POCKET_ALLOWANCE),
                v["size"][1] + v.get("pocket_allowance", POCKET_ALLOWANCE),
                depth,
            ],
            "centers_yz_mm": [(y, v["z"]) for y in v["ys"]],
            "face_recess_each_mm": recess,
            "magnet_gap_mm": 2 * recess,
            "backing_mm": BACK,
            "retainer_clearance_mm": (
                v["retainer_edge_x"] if variant == "round2" else base.RETAINER_LEFT
            )
            - face
            - depth
            - BACK,
            "external_projection_mm": 0,
            "contour_depth_mm": v["seat"],
            "hand_added_volume_mm3": (hand - base.mascot(hand_connection=False)).volume,
        }
        if variant == "round2":
            records[variant].update(
                nominal_purchase_mm=[3, 2],
                tested_magnet_diameter_mm=3.04,
                CAD_thickness_limit_mm=depth - recess - 0.1,
                glue_gap_at_limit_mm=0.1,
                glue_gap_at_nominal_thickness_mm=depth - recess - v["thickness"],
                paired_magnet_faces_touch=True,
                roof_angle_deg=45,
                roof_bridge_mm=v["roof_bridge"],
                roof_points_up_in_both_print_poses=True,
                minimum_teardrop_collar_mm=0.8,
                retainer_relief_mm=v["retainer_edge_x"] - base.RETAINER_LEFT,
            )
    mesh_records = {}
    for n, s in parts(selected).items():
        path = output / "stl" / (n + ".stl")
        m = trimesh.load_mesh(path)
        assert m.is_watertight and m.is_winding_consistent and m.volume > 0, n
        assert len(m.split()) == 1 and abs(m.bounds[0, 2]) < 1e-5, n
        restored = import_step(output / "step" / (n + ".step"))
        assert restored.is_valid and len(restored.solids()) == 1, n
        assert (s - restored).volume + (restored - s).volume < 1e-4, n
        mesh_records[n] = {
            "quantity": 2 if "eye" in n else 1,
            "bounds_mm": m.extents.tolist(),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }
    report = {
        "status": "passed",
        "selected": selected,
        "physical_fit_tested": False,
        "checks": [
            "valid single solids",
            "printed part/magnet collisions",
            "electronics clearance",
            "LED retainer removal",
            "blind backing",
            "maximum tolerance magnet insertion",
            "round2 teardrop .8 mm surrounding wall",
            "round2 opposing magnet faces touch at the mating plane",
            "straight release",
            "no external podium",
            "coplanar feet",
            "STEP roundtrip of production print parts",
            "watertight printable meshes",
        ],
        "variants": records,
        "parts": mesh_records,
    }
    (output / "validation.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def export_all(output=OUT, selected="round2"):
    from build123d import export_step, export_stl

    output.mkdir(parents=True, exist_ok=True)
    (output / "stl").mkdir(exist_ok=True)
    (output / "step").mkdir(exist_ok=True)
    for n, s in parts(selected).items():
        export_step(s, output / "step" / (n + ".step"))
        export_stl(
            base.print_pose(n, s),
            output / "stl" / (n + ".stl"),
            tolerance=0.02,
            angular_tolerance=0.05,
        )
    report = validate(output, selected)
    print(
        "Validated",
        len(report["variants"]),
        "variants and",
        len(report["parts"]),
        "printable parts",
    )


if __name__ == "__main__":
    export_all()
