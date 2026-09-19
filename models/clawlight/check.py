"""Runnable CAD/mesh checks. Run AFTER clawlight.py exports updated STLs."""

import itertools
import json
from math import radians, sqrt, tan
from pathlib import Path

import numpy as np
import trimesh
from build123d import Align, Axis, Cone, Cylinder, Face, GeomType, Plane, Wire, extrude
import clawlight as m
import hand_joint as j
import usb_coupon as u


def overlap(a, b):
    c = a.intersect(b)
    if c is None:
        return 0.0
    return c.volume if hasattr(c, "volume") else sum(s.volume for s in c)


def clear(a, b, message):
    v = overlap(a, b)
    assert v < 1e-5, f"{message}: {v:.6f} mm3 collision"


def cylinder_z(x, y, z0, z1, diameter):
    return Cylinder(
        diameter / 2, z1 - z0, align=(Align.CENTER, Align.CENTER, Align.MIN)
    ).translate((x, y, z0))


def screw_head_z(x, y, rear, diameter):
    """Physical 80-degree head, including the recess caused by seat clearance."""
    slope = tan(radians(m.SCREW_HEAD_ANGLE_DEG / 2))
    seating = (m.COUNTERSINK_DIAMETER - diameter) / 2 / slope
    height = (diameter - m.SCREW_DIAMETER) / 2 / slope
    return Cone(
        m.SCREW_DIAMETER / 2,
        diameter / 2,
        height,
        align=(Align.CENTER, Align.CENTER, Align.MIN),
    ).translate((x, y, rear - seating - height))


def screw_checks(parts, report):
    shell = parts["04_tower_black"]
    hardware = []
    for name, shape in m.electronics().items():
        if name in ("pcb", "top", "underside", "usb"):
            b = shape.bounding_box()
            shape = m.box(
                b.min.X - m.PCB_SIDE_CLEARANCE,
                b.max.X + m.PCB_SIDE_CLEARANCE,
                b.min.Y - m.PCB_END_CLEARANCE,
                b.max.Y + m.PCB_END_CLEARANCE,
                b.min.Z,
                b.max.Z + m.PCB_TOP_CLEARANCE,
            )
        hardware.append((name, shape))
    longest = m.SCREW_LENGTH + m.SCREW_LENGTH_TOLERANCE
    shortest = m.SCREW_LENGTH - m.SCREW_LENGTH_TOLERANCE
    slope = tan(radians(m.SCREW_HEAD_ANGLE_DEG / 2))
    assert 0 < m.SCREW_PILOT_DIAMETER < m.SCREW_DIAMETER < m.SCREW_CLEARANCE
    assert (
        m.COUNTERSINK_DIAMETER > m.SCREW_HEAD_DIAMETER_MAX >= m.SCREW_HEAD_DIAMETER_MIN
    )
    assert (
        abs(
            m.COUNTERSINK_DEPTH
            - (m.COUNTERSINK_DIAMETER - m.SCREW_CLEARANCE) / 2 / slope
        )
        < 1e-8
    )
    assert (
        abs(
            m.MAX_HEAD_SEATING_DEPTH
            - (m.COUNTERSINK_DIAMETER - m.SCREW_HEAD_DIAMETER_MIN) / 2 / slope
        )
        < 1e-8
    )
    report["screws"] = {
        "standard": "DIN 7982 ST2.2 x 6.5, countersunk Phillips",
        "head_angle_degrees": m.SCREW_HEAD_ANGLE_DEG,
        "closures": {},
    }
    for name, locations, mount, rear, depth in (
        ("05_rear_cover_black", m.COVER_SCREWS, m.REAR, m.DEPTH, m.COVER_PILOT_DEPTH),
        (
            "06_LED_retainer_black",
            m.LED_SCREWS,
            m.RETAINER_MOUNT_Y,
            m.RETAINER_BACK_Y,
            m.RETAINER_PILOT_DEPTH,
        ),
    ):
        plate = parts[name]
        floor = mount - depth
        worst_tip = rear - longest - m.MAX_HEAD_SEATING_DEPTH
        assert floor >= m.SCREW_TIP_WALL - 1e-8, name + " pilot pierces front skin"
        assert worst_tip - floor >= m.SCREW_TIP_CLEARANCE - 1e-8, (
            name + " long screw bottoms out"
        )
        assert rear - mount > m.COUNTERSINK_DEPTH, (
            name + " countersink consumes plate thickness"
        )
        least_seating = (m.COUNTERSINK_DIAMETER - m.SCREW_HEAD_DIAMETER_MAX) / 2 / slope
        min_thread = (
            shortest + least_seating - (rear - mount) - m.SCREW_POINT_LENGTH_MAX
        )
        assert min_thread > 0, (
            name + " shortest screw cannot reach with full-diameter thread"
        )
        for x, z in locations:
            pilot = m.cy(x, z, floor, mount, m.SCREW_PILOT_DIAMETER)
            clear(pilot, shell, name + " blind pilot depth")
            skin = m.cy(x, z, floor - m.SCREW_TIP_WALL, floor, m.SCREW_PILOT_DIAMETER)
            assert abs(overlap(skin, shell) - skin.volume) < 1e-5, (
                name + " missing tip-protection skin"
            )
            if name == "05_rear_cover_black":
                safe_diameter = (
                    min(m.COVER_BOSS_DIAMETER, 2 * (z - m.PCB_INSERTION_CEILING))
                    if z < m.BASE_HEIGHT
                    else m.COVER_BOSS_DIAMETER
                )
                assert (
                    safe_diameter - m.SCREW_PILOT_DIAMETER
                ) / 2 >= 1.55 - 1e-8, "PCB relief thins a screw pilot wall"
                cap = m.cy(x, z, m.COVER_BOSS_FRONT, floor, safe_diameter)
                assert (
                    abs(overlap(cap, shell) - cap.volume) < 1e-5
                ), "Short screw boss has no protected blind cap"
                annulus = m.cy(x, z, floor, mount, safe_diameter) - pilot
                assert (
                    abs(overlap(annulus, shell) - annulus.volume) < 1e-5
                ), "Short screw boss lost pilot wall thickness"
                boss = m.cover_screw_boss(x, z)
                assert boss.is_valid and len(boss.solids()) == 1
                normals = [f.normal_at().Y for f in boss.faces()]
                assert all(
                    n >= -sqrt(0.5) - 1e-6 for n in normals
                ), "Screw ramp needs supports in the +Y print direction"
                assert any(
                    abs(n + sqrt(0.5)) < 1e-6 for n in normals
                ), "Screw boss has no 45-degree wall ramp"
                # The inward tip of each old post must be gone ahead of the ramp.
                inward = 1 if x < m.TOWER_X else -1
                exposed_x = x + inward * (m.COVER_BOSS_DIAMETER / 2 - 0.3)
                clear(
                    m.box(
                        exposed_x - 0.05,
                        exposed_x + 0.05,
                        m.WALL + 0.1,
                        m.COVER_BOSS_FRONT - 0.5,
                        z - 0.05,
                        z + 0.05,
                    ),
                    shell,
                    "Full-length cover post still present",
                )
            else:
                annulus = m.cy(x, z, floor, mount, m.LED_BOSS_DIAMETER) - pilot
                assert (
                    abs(overlap(annulus, shell) - annulus.volume) < 1e-5
                ), "Retainer screw boss loses its full pilot wall"
            # A continuous seating annulus below cosmetic rear edge bevels.
            ring = m.cy(x, z, rear - 0.7, rear - 0.6, m.COUNTERSINK_DIAMETER + 0.8)
            ring -= m.cy(x, z, rear - 0.71, rear - 0.59, m.COUNTERSINK_DIAMETER)
            assert abs(overlap(ring, plate) - ring.volume) < 1e-5, (
                name + " head seat breaks into an edge or lead slot"
            )
            for head_d in (m.SCREW_HEAD_DIAMETER_MIN, m.SCREW_HEAD_DIAMETER_MAX):
                head = screw_head_z(x, -z, rear, head_d).rotate(Axis.X, -90)
                seated_rear = rear - (m.COUNTERSINK_DIAMETER - head_d) / 2 / slope
                driver = m.cy(x, z, seated_rear, m.DEPTH + 30, m.SCREW_DRIVER_DIAMETER)
                for other_name, other in parts.items():
                    if (
                        name == "06_LED_retainer_black"
                        and other_name == "05_rear_cover_black"
                    ):
                        continue  # Remove the cover before servicing the internal screw.
                    clear(head, other, name + " seated head / " + other_name)
                    clear(driver, other, name + " recessed-head driver / " + other_name)
                for hardware_name, envelope in hardware:
                    clear(
                        driver,
                        envelope,
                        name + " driver / hardware float envelope " + hardware_name,
                    )
        report["screws"]["closures"][name] = {
            "pilot_floor_y_mm": floor,
            "worst_long_screw_tip_y_mm": worst_tip,
            "tip_to_pilot_floor_mm": worst_tip - floor,
            "minimum_full_diameter_thread_mm": min_thread,
        }
    assert len(m.LED_SCREWS) == 1, "Retainer needs exactly one lower screw"
    screw_count = len(m.COVER_SCREWS) + len(m.LED_SCREWS)
    report["screws"]["cover_bosses"] = {
        "count": len(m.COVER_SCREWS),
        "diameter_mm": m.COVER_BOSS_DIAMETER,
        "axial_land_length_mm": m.REAR - m.COVER_BOSS_FRONT,
        "blind_cap_mm": m.SCREW_TIP_WALL,
        "minimum_pilot_wall_mm": min(
            (
                min(m.COVER_BOSS_DIAMETER / 2, z - m.PCB_INSERTION_CEILING)
                if z < m.BASE_HEIGHT
                else m.COVER_BOSS_DIAMETER / 2
            )
            for _, z in m.COVER_SCREWS
        )
        - m.SCREW_PILOT_DIAMETER / 2,
        "lower_boss_floor_z_mm": m.PCB_INSERTION_CEILING,
        "support_ramp_degrees": 45,
        "print_growth_axis": "+Y",
        "physical_strength": "Thread dimensions retained; wall-supported bosses need a printed tightening test",
    }
    report["checks"].append(
        f"DIN 7982 min/max heads seat clear; all {screw_count} pilots retain tip clearance and blind caps; seating annuli and driver access clear; three short cover bosses preserve full pilot walls and have 45-degree ramps"
    )


def coupon_checks(coupons, report):
    for name, shape in coupons.items():
        assert shape.is_valid and len(shape.solids()) == 1 and shape.volume > 0, name
    report["checks"].append("Both active USB coupon parts are valid single solids")
    usb_crops = {}
    for source, name in (
        ("04_tower_black", "01_USB_base_coupon"),
        ("05_rear_cover_black", "02_USB_cover_coupon"),
    ):
        production = m.parts()[source]
        bounds = production.bounding_box()
        crop = production & m.box(
            bounds.min.X - 1,
            bounds.max.X + 1,
            bounds.min.Y - 1,
            bounds.max.Y + 1,
            bounds.min.Z - 1,
            u.COUPON_HEIGHT,
        )
        expected = m.print_pose(source, crop)
        common = overlap(expected, coupons[name])
        assert (
            abs(common - expected.volume) < 1e-5
            and abs(common - coupons[name].volume) < 1e-5
        ), ("USB coupon differs from its actual production crop: " + name)
        usb_crops[name] = crop
    clear(*usb_crops.values(), "USB coupon assembly alignment")
    for crop in usb_crops.values():
        for electronics in m.electronics().values():
            clear(crop, electronics, "Electronics / USB fit fixture")
    report["checks"].append(
        "Both USB fixture pieces exactly match production crops, including integral PCB guides; electronics align without interference"
    )


def shoulder_checks(shell, report):
    # Inspect a continuous 2 mm strip immediately inside each exterior shoulder
    # chamfer. The former cavity broke through this exact region.
    width = 2.0
    inset = 0.15
    start = (m.UPRIGHT_LEFT - m.TOWER_CHAMFER, m.BASE_HEIGHT)
    end = (m.UPRIGHT_LEFT, m.BASE_HEIGHT + m.TOWER_CHAMFER)

    def inward(point, distance):
        return (point[0] + distance / sqrt(2), point[1] - distance / sqrt(2))

    points = [
        inward(start, inset),
        inward(end, inset),
        inward(end, inset + width),
        inward(start, inset + width),
    ]
    volumes = []
    for mirrored in (False, True):
        profile = [(2 * m.TOWER_X - x, z) if mirrored else (x, z) for x, z in points]
        strip = m.profile(profile, m.WALL, m.REAR)
        assert (
            abs(overlap(strip, shell) - strip.volume) < 1e-5
        ), "Tower shoulder has a gap or loses its continuous 2 mm reinforcement"
        volumes.append(strip.volume)
    report["tower_shoulders"] = {
        "continuous_strip_width_mm": width,
        "verified_depth_y_mm": [m.WALL, m.REAR],
        "solid_probe_volumes_mm3": volumes,
    }
    report["checks"].append(
        "Both formerly open shoulders contain continuous 2 mm reinforced strips from the front wall to the rear seam"
    )


def cover_checks(cover, report):
    # Check the seated PCB and both end-stop extremes, not only nominal flush.
    projection = m.USB_REAR - m.DEPTH
    minimum = projection - m.PCB_END_CLEARANCE
    maximum = projection + m.PCB_END_CLEARANCE
    assert (
        minimum >= -1e-8 and maximum <= 0.5 + 1e-8
    ), "USB must stay flush to 0.5 mm proud throughout endplay"
    assert abs(cover.bounding_box().max.Y - m.DEPTH) < 1e-8
    assert abs(m.DEPTH - m.COVER_POCKET_BACK - m.COVER_INNER_SKIN) < 1e-8
    assert m.COVER_INNER_SKIN >= 1 - 1e-8, "Flat cover skin is thinner than 1 mm"
    # Inspect the whole lower skin except the intentional aperture/bevel band.
    skin = m.box(
        m.COVER_POCKET_LEFT + 0.5,
        m.COVER_POCKET_RIGHT - 0.5,
        m.COVER_POCKET_BACK,
        m.DEPTH,
        m.COVER_POCKET_BOTTOM + 0.5,
        m.COVER_POCKET_TOP - 0.5,
    )
    skin -= m.box(
        m.TOWER_X - m.USB_OPENING_WIDTH / 2 - m.USB_ENTRY_CHAMFER,
        m.TOWER_X + m.USB_OPENING_WIDTH / 2 + m.USB_ENTRY_CHAMFER,
        m.COVER_POCKET_BACK - 0.1,
        m.DEPTH + 0.1,
        m.USB_CENTER_Z - m.USB_OPENING_HEIGHT / 2 - m.USB_ENTRY_CHAMFER,
        m.USB_CENTER_Z + m.USB_OPENING_HEIGHT / 2 + m.USB_ENTRY_CHAMFER,
    )
    assert (
        abs(overlap(skin, cover) - skin.volume) < 1e-5
    ), "Flat external USB cover skin contains an unintended pocket or break"
    # Probe removed edge material and the preserved USB throat behind its bevel.
    usb_top = m.USB_CENTER_Z + m.USB_OPENING_HEIGHT / 2
    clear(
        m.box(
            m.TOWER_X - 0.5,
            m.TOWER_X + 0.5,
            m.COVER_POCKET_BACK + 0.025,
            m.COVER_POCKET_BACK + 0.05,
            usb_top + 0.025,
            usb_top + 0.05,
        ),
        cover,
        "Inner USB chamfer missing",
    )
    throat = m.box(
        m.TOWER_X - 0.5,
        m.TOWER_X + 0.5,
        m.COVER_POCKET_BACK + 0.25,
        m.COVER_POCKET_BACK + 0.30,
        usb_top + 0.025,
        usb_top + 0.05,
    )
    assert (
        abs(overlap(throat, cover) - throat.volume) < 1e-6
    ), "Chamfer enlarges the USB throat"
    for x0, x1 in (
        (m.BASE_LEFT + 0.025, m.BASE_LEFT + 0.05),
        (m.COVER_POCKET_LEFT - 0.05, m.COVER_POCKET_LEFT - 0.025),
    ):
        clear(
            m.box(x0, x1, m.REAR + 0.025, m.REAR + 0.05, 3, 3.1),
            cover,
            "Inner cover edge chamfer missing",
        )
    for side in (-1, 1):
        edge = m.TOWER_X + side * m.PCB_WIDTH / 2
        x0, x1 = sorted(
            (edge - side * 0.1, edge - side * (m.PCB_REAR_STOP_INSET - 0.1))
        )
        stop = m.box(
            x0,
            x1,
            m.PCB_REAR_STOP_FRONT,
            m.DEPTH,
            m.PCB_BOTTOM + 0.05,
            m.PCB_TOP - 0.05,
        )
        assert (
            abs(overlap(stop, cover) - stop.volume) < 1e-5
        ), "Cover-mounted PCB stop has no solid backing to the exterior skin"
    posed = m.print_pose("05_rear_cover_black", cover)
    for face in posed.faces():
        if face.bounding_box().max.Z > 1e-6:
            assert (
                face.normal_at().Z >= -sqrt(0.5) - 1e-6
            ), "Flat cover has a downward face requiring support above its print bed"
    report["flat_cover"] = {
        "skin_mm": m.COVER_INNER_SKIN,
        "inner_edge_chamfer_mm": m.COVER_INNER_CHAMFER,
        "usb_entry_chamfer_both_sides_mm": m.USB_ENTRY_CHAMFER,
        "skin_probe_volume_mm3": skin.volume,
        "pcb_stop_backing_thickness_mm": m.DEPTH - m.PCB_REAR_STOP_FRONT,
        "usb_face_offset_mm": m.DEPTH - m.USB_REAR,
        "usb_projection_range_mm": [minimum, maximum],
    }
    report["checks"].append(
        "Cover has a continuous 1 mm skin and solid PCB stops; modeled USB stays flush to 0.5 mm proud across endplay (physical axial discrepancy unresolved); above-bed downward faces meet the 45-degree limit"
    )


def hand_joint_checks(parts, report):
    mascot = parts["01_mascot_orange"]
    shell = parts["04_tower_black"]
    assert (
        abs(j.JOINT_X - m.UPRIGHT_LEFT) < 1e-8
    ), "Hand joint datum misses tower mating face"
    assert j.SOCKET_DIAGONAL > j.PIN_DIAGONAL > 0
    assert j.SOCKET_DEPTH > j.PIN_PROJECTION > 0
    pins = j.hand_pins()
    assert len(pins.solids()) == len(j.JOINT_CENTERS_YZ)
    assert (
        abs(mascot.bounding_box().max.Y - m.MASCOT_DEPTH) < 1e-6
    ), "Mascot rear datum does not match its independent depth"
    rear_pin_margin = (
        m.MASCOT_DEPTH - m.BED_CHAMFER * m.MASCOT_SCALE - pins.bounding_box().max.Y
    )
    assert (
        rear_pin_margin >= 0.5
    ), "Slim mascot rear bevel is too close to the hand-pin haunch"
    for pin in pins.solids():
        assert (
            abs(overlap(pin, mascot) - pin.volume) < 1e-5
        ), "Integral hand pin is not fully joined to mascot"
        clear(pin, shell, "Hand pin / tower socket")
        for face in pin.rotate(Axis.X, -90).faces():
            assert (
                abs(face.normal_at().Z) <= sqrt(0.5) + 1e-6
            ), "Hand pin or tip bevel requires supports"
    for y, z in j.JOINT_CENTERS_YZ:
        straight_core = (
            Cylinder(
                j.PIN_DIAGONAL / 6,
                j.PIN_PROJECTION,
                align=(Align.CENTER, Align.CENTER, Align.MIN),
            )
            .rotate(Axis.Y, 90)
            .translate((j.JOINT_X, y, z))
        )
        assert (
            abs(overlap(straight_core, mascot) - straight_core.volume) < 1e-5
        ), "Hand pin does not retain a straight core normal to the hand face"
        hole = j.diamond_socket(
            j.JOINT_X, j.JOINT_X + j.SOCKET_DEPTH, y, z, j.SOCKET_DIAGONAL
        )
        clear(hole, shell, "Straight hand socket depth")
        # Probe real solid behind the blind end, along the same insertion axis.
        floor = j.diamond_socket(
            j.JOINT_X + j.SOCKET_DEPTH,
            j.JOINT_X + j.SOCKET_DEPTH + 0.4,
            y,
            z,
            j.PIN_DIAGONAL / 2,
        )
        assert (
            abs(overlap(floor, shell) - floor.volume) < 1e-5
        ), "Hand socket has no 0.4 mm solid blind floor"
    for distance in (0, 0.3, 0.6, 1.2, j.PIN_PROJECTION, j.SOCKET_DEPTH + 0.2):
        clear(
            mascot.translate((-distance, 0, 0)),
            shell,
            "Straight hand-joint insertion path",
        )
    # The rear socket starts inside the cavity when the tower prints front down.
    # Check a continuous solid strip from the wall to its leading edge, then
    # inspect the local exposed faces in the actual +Y printing direction.
    rear_y, rear_z = max(j.JOINT_CENTERS_YZ, key=lambda yz: yz[0])
    wall_x = m.UPRIGHT_LEFT + m.UPRIGHT_WALL
    tip_x = j.JOINT_X + j.SOCKET_DEPTH + m.HAND_SOCKET_WALL
    radius = (j.SOCKET_DIAGONAL + 2 * m.HAND_SOCKET_WALL) / 2
    leading_y = rear_y - radius
    support_y = leading_y - (tip_x - wall_x) - m.HAND_SOCKET_GUSSET_OVERLAP
    assert m.HAND_SOCKET_GUSSET_OVERLAP > 0 and tip_x > wall_x
    x0 = wall_x - m.HAND_SOCKET_GUSSET_OVERLAP / 2
    x1 = tip_x - 0.05
    y0 = leading_y - (tip_x - x0) + 0.15
    y1 = leading_y - (tip_x - x1) + 0.15
    strip_face = Face(
        Wire.make_polygon(
            [
                (x0, y0, rear_z - 0.1),
                (x1, y1, rear_z - 0.1),
                (x1, y1 + 0.2, rear_z - 0.1),
                (x0, y0 + 0.2, rear_z - 0.1),
            ],
            close=True,
        )
    )
    strip = extrude(strip_face, amount=0.2, dir=(0, 0, 1))
    assert (
        abs(overlap(strip, shell) - strip.volume) < 1e-5
    ), "Rear socket boss has no continuous leading support from the side wall"
    support_faces = []
    for face in shell.faces():
        bounds = face.bounding_box()
        if (
            bounds.min.X >= wall_x - m.HAND_SOCKET_GUSSET_OVERLAP - 1e-6
            and bounds.max.X <= tip_x + 1e-6
            and bounds.min.Y >= support_y - 1e-6
            and bounds.max.Y <= rear_y + radius + (tip_x - j.JOINT_X) + 1e-6
            and bounds.min.Z >= rear_z - radius - 1e-6
            and bounds.max.Z <= rear_z + radius + 1e-6
        ):
            support_faces.append(face)
    support_normals = [face.normal_at().Y for face in support_faces]
    assert support_normals and all(
        n >= -sqrt(0.5) - 1e-6 for n in support_normals
    ), "Rear socket boss contains a front-down overhang steeper than 45 degrees"
    assert any(
        abs(n + sqrt(0.5)) < 1e-6 for n in support_normals
    ), "Rear socket leading 45-degree gusset face missing"
    report["hand_joint"] = {
        "mascot_depth_mm": m.MASCOT_DEPTH,
        "rear_pin_margin_before_chamfer_mm": rear_pin_margin,
        "integral_pin_count": len(j.JOINT_CENTERS_YZ),
        "pin_diagonal_mm": j.PIN_DIAGONAL,
        "socket_diagonal_mm": j.SOCKET_DIAGONAL,
        "front_guide_face_clearance_mm": (j.SOCKET_DIAGONAL - j.PIN_DIAGONAL)
        / (2 * sqrt(2)),
        "tip_clearance_x_mm": j.SOCKET_DEPTH - j.PIN_PROJECTION,
        "insertion_direction": [1, 0, 0],
        "withdrawal_direction": [-1, 0, 0],
        "rear_socket_gusset": {
            "continuous_wall_to_boss_probe_volume_mm3": strip.volume,
            "local_faces_checked": len(support_faces),
            "minimum_print_direction_normal": min(support_normals),
        },
        "physical_fit": "User tested the 3.3 mm coupon: locating fit, not friction retention; optional glue for permanent attachment. Simultaneous two-pin assembly remains unverified.",
    }
    report["checks"].append(
        "Integral straight hand pins clear both blind sockets and the straight-X insertion path; blind floors and support-free pin faces checked"
    )
    report["checks"].append(
        "Rear socket gusset has a continuous solid support strip from the wall; its local front-down faces have at most 45-degree overhangs"
    )


def pcb_guide_checks(parts, report):
    shell = parts["04_tower_black"]
    cover = parts["05_rear_cover_black"]
    guides = m.pcb_guides()
    electronics = m.electronics()
    pcb = electronics["pcb"]
    left, right = m.TOWER_X - m.PCB_WIDTH / 2, m.TOWER_X + m.PCB_WIDTH / 2
    assert (
        guides.is_valid and len(guides.solids()) == 2
    ), "Integral PCB guides must form two valid rails"
    assert (
        abs(overlap(guides, shell) - guides.volume) < 1e-5
    ), "PCB guides are not wholly integrated into the base"
    assert abs(m.BASE_FLOOR_THICKNESS - 2) < 1e-8
    assert abs(m.PCB_BOTTOM - m.BASE_FLOOR_THICKNESS - m.PCB_FLOOR_GAP) < 1e-8
    underside_gap = m.PCB_BOTTOM - m.PCB_UNDERSIDE - m.BASE_FLOOR_THICKNESS
    assert (
        underside_gap >= 0.2 - 1e-8
    ), "Provisional underside components lose their 0.2 mm floor clearance"
    floor = m.box(
        m.INNER_LEFT, m.INNER_RIGHT, m.WALL, m.REAR, 0, m.BASE_FLOOR_THICKNESS
    )
    assert (
        abs(overlap(floor, shell) - floor.volume) < 1e-5
    ), "Base floor is not continuous and flat"
    corner_end = m.PCB_FRONT + m.PCB_CORNER_OVERLAP
    # Above the flat floor, exclude the intentional side and corner bearing shelves.
    for x0, x1, y0, y1 in (
        (
            left + m.PCB_RAIL_LOWER_OVERLAP + 0.01,
            right - m.PCB_RAIL_LOWER_OVERLAP - 0.01,
            corner_end + 0.01,
            m.REAR,
        ),
        (
            left + m.PCB_CORNER_LENGTH + 0.01,
            right - m.PCB_CORNER_LENGTH - 0.01,
            m.WALL + 0.01,
            corner_end + 0.01,
        ),
    ):
        clear(
            m.box(x0, x1, y0, y1, m.BASE_FLOOR_THICKNESS, m.BASE_FLOOR_THICKNESS + 0.1),
            shell,
            "Central base floor has an unintended raised shelf",
        )
    for face in guides.faces():
        # The leading ends at Y <= WALL are embedded in the front wall.
        if face.bounding_box().max.Y > m.WALL + 1e-6:
            assert (
                face.normal_at().Y >= -sqrt(0.5) - 1e-6
            ), "Exposed PCB guide face needs support when printed in +Y"

    # Material above the old flat rail roof must really be added, with a
    # positive wall intersection and a broad 45-degree face, not an edge bevel.
    tip = left + m.PCB_RAIL_UPPER_OVERLAP
    x0 = m.INNER_LEFT + 0.05
    triangle = m.profile(
        [
            (x0, m.PCB_RAIL_TOP + 0.05),
            (tip - 0.15, m.PCB_RAIL_TOP + 0.05),
            (x0, m.PCB_RAIL_TOP + tip - x0 - 0.05),
        ],
        corner_end + 0.2,
        m.REAR - 0.4,
    )
    wall_bridge = m.box(
        m.INNER_LEFT - 0.05,
        m.INNER_LEFT + 0.05,
        corner_end + 0.2,
        m.REAR - 0.4,
        m.PCB_RAIL_TOP + 0.2,
        m.PCB_RAIL_TOP + 0.5,
    )
    rear_wall_bridge = m.box(
        m.INNER_LEFT - 0.05,
        m.INNER_LEFT + 0.05,
        m.REAR - 0.1,
        m.REAR,
        m.PCB_RAIL_TOP + 0.2,
        m.PCB_RAIL_TOP + 0.5,
    )
    for probe in (triangle, wall_bridge, rear_wall_bridge):
        for mirrored in (False, True):
            actual = probe.mirror(Plane.YZ.offset(m.TOWER_X)) if mirrored else probe
            assert (
                abs(overlap(actual, guides) - actual.volume) < 1e-5
            ), "Rail haunch is missing or only touches the wall"
            assert (
                abs(overlap(actual, shell) - actual.volume) < 1e-5
            ), "Rail haunch is not fused into the production base"
    haunch_faces = [
        f
        for f in guides.faces()
        if abs(abs(f.normal_at().X) - sqrt(0.5)) < 1e-6
        and abs(f.normal_at().Z - sqrt(0.5)) < 1e-6
        and f.bounding_box().size.Z > 0.6
        and f.bounding_box().size.Y > m.REAR - corner_end - 1
    ]
    assert (
        len(haunch_faces) == 2
    ), "Both side rails need broad 45-degree reinforcing faces"

    assert 2 <= m.PCB_CORNER_LENGTH <= 3
    middle_width = m.PCB_WIDTH - 2 * m.PCB_CORNER_LENGTH
    assert (
        middle_width >= 13 - 1e-8
    ), "Corner holders close the central component opening"
    clear(
        m.box(
            left + m.PCB_CORNER_LENGTH,
            right - m.PCB_CORNER_LENGTH,
            m.WALL + 0.01,
            corner_end,
            m.PCB_TOP + m.PCB_TOP_CLEARANCE,
            m.PCB_TOP + m.PCB_TOP_HEIGHT,
        ),
        shell,
        "Central 13 mm component opening is blocked",
    )
    # These probes lie beyond the side roofs, so the side rails
    # alone cannot satisfy the corner-retention assertions.
    corner_patches = []
    for mirrored in (False, True):
        bearing = m.box(
            left + 0.5,
            left + m.PCB_CORNER_LENGTH - 0.2,
            m.PCB_FRONT + m.PCB_END_CLEARANCE + 0.01,
            corner_end - m.PCB_CORNER_ENTRY_CHAMFER - 0.01,
            m.PCB_TOP + m.PCB_TOP_CLEARANCE,
            m.PCB_TOP + m.PCB_TOP_CLEARANCE + 0.1,
        )
        corner = m.box(
            left + 0.5,
            left + m.PCB_CORNER_LENGTH - 0.2,
            m.PCB_FRONT,
            m.PCB_FRONT + m.PCB_CORNER_OVERLAP,
            m.PCB_BOTTOM,
            m.PCB_TOP,
        )
        if mirrored:
            bearing = bearing.mirror(Plane.YZ.offset(m.TOWER_X))
            corner = corner.mirror(Plane.YZ.offset(m.TOWER_X))
        assert (
            abs(overlap(bearing, shell) - bearing.volume) < 1e-5
        ), "Far-corner lip lacks a flat capture land at maximum rear endplay"
        for dx, dy, dz in itertools.product(
            (-m.PCB_SIDE_CLEARANCE, m.PCB_SIDE_CLEARANCE),
            (-m.PCB_END_CLEARANCE, m.PCB_END_CLEARANCE),
            (-0.1, m.PCB_TOP_CLEARANCE + 0.1),
        ):
            assert (
                overlap(corner.translate((dx, dy, dz)), shell) > 1e-5
            ), "A far corner escapes its upper or lower retaining lip"
        # Verify that the mouth chamfer opens both lips yet preserves material behind it.
        for z0, z1 in (
            (m.PCB_BOTTOM - 0.05, m.PCB_BOTTOM - 0.025),
            (
                m.PCB_TOP + m.PCB_TOP_CLEARANCE + 0.025,
                m.PCB_TOP + m.PCB_TOP_CLEARANCE + 0.05,
            ),
        ):
            mouth = m.box(
                left + 1, left + 1.2, corner_end - 0.05, corner_end - 0.025, z0, z1
            )
            land = m.box(
                left + 1, left + 1.2, corner_end - 0.25, corner_end - 0.2, z0, z1
            )
            if mirrored:
                mouth = mouth.mirror(Plane.YZ.offset(m.TOWER_X))
                land = land.mirror(Plane.YZ.offset(m.TOWER_X))
            clear(mouth, shell, "Far-corner slot mouth chamfer missing")
            assert (
                abs(overlap(land, shell) - land.volume) < 1e-5
            ), "Far-corner chamfer removes the bearing behind its lead-in"
        corner_patches.append(bearing.volume)

    # Only the two explicitly assumed bare front corners may change the
    # provisional top envelope; its measured PCB and 4 mm central height remain.
    original_top = m.box(
        left + 0.5,
        right - 0.5,
        m.PCB_FRONT + 0.5,
        m.PCB_REAR - 3,
        m.PCB_TOP,
        m.PCB_TOP + m.PCB_TOP_HEIGHT,
    )
    top = electronics["top"]
    assert (
        abs(overlap(top, original_top) - top.volume) < 1e-5
    ), "Top proxy added material beyond its original envelope"
    relieved_width = m.PCB_CORNER_CLEAR_WIDTH
    assert (
        abs(relieved_width - 2.7) < 1e-8
    ), "Do not enlarge the provisional bare PCB patches to fit the holders"
    expected_removed = (
        2 * (relieved_width - 0.5) * (m.PCB_CORNER_CLEAR_DEPTH - 0.5) * m.PCB_TOP_HEIGHT
    )
    assert (
        abs(original_top.volume - top.volume - expected_removed) < 1e-5
    ), "Top component proxy changed outside its two declared corner reliefs"
    for x0, x1 in (
        (left + 0.5, left + relieved_width),
        (right - relieved_width, right - 0.5),
    ):
        clear(
            m.box(
                x0,
                x1,
                m.PCB_FRONT + 0.5,
                m.PCB_FRONT + m.PCB_CORNER_CLEAR_DEPTH,
                m.PCB_TOP,
                m.PCB_TOP + m.PCB_TOP_HEIGHT,
            ),
            top,
            "Declared bare corner still has a component proxy",
        )
    center = m.box(
        left + relieved_width,
        right - relieved_width,
        m.PCB_FRONT + 0.5,
        m.PCB_REAR - 3,
        m.PCB_TOP,
        m.PCB_TOP + m.PCB_TOP_HEIGHT,
    )
    assert (
        abs(overlap(center, top) - center.volume) < 1e-5
    ), "Central top-component envelope was reduced"

    # The bare PCB can move +/-0.6 mm, but wider roofs require centered
    # insertion: the unchanged top envelope starts 0.5 mm in from each edge.
    # Keep 0.05 mm between that envelope and the lips; do not shrink hardware
    # proxies or claim that the whole assembly clears the full bare-board float.
    insertion_side_play = min(
        m.PCB_SIDE_CLEARANCE, 0.5 - m.PCB_RAIL_UPPER_OVERLAP - 0.05
    )
    assert insertion_side_play >= 0.15 - 1e-8
    # Sweep the actual axis-aligned envelopes, including their corner reliefs.
    # On each +Y ray, the initial solid plus its outward rear faces swept
    # forward is the complete translation volume; no bounding-box substitution.
    sweeps = 0
    swept_faces = 0
    for name in ("pcb", "top", "underside", "usb"):
        shape = electronics[name]
        assert all(
            f.geom_type == GeomType.PLANE
            and max(abs(v) for v in f.normal_at()) > 1 - 1e-6
            for f in shape.faces()
        ), ("PCB insertion envelope must have axis-aligned planar faces: " + name)
        rear_faces = [f for f in shape.faces() if f.normal_at().Y > 1 - 1e-6]
        assert rear_faces, "Insertion envelope has no rear boundary: " + name
        for dx, dz in itertools.product(
            (-insertion_side_play, 0, insertion_side_play), (0, m.PCB_TOP_CLEARANCE)
        ):
            offset = (dx, -m.PCB_END_CLEARANCE, dz)
            clear(
                shape.translate(offset), shell, "Fully seated insertion pose / " + name
            )
            for face in rear_faces:
                sweep = extrude(
                    face.translate(offset),
                    amount=30 + m.PCB_END_CLEARANCE,
                    dir=(0, 1, 0),
                )
                assert sweep.is_valid, "PCB insertion sweep is invalid: " + name
                clear(sweep, shell, "Continuous open-cover insertion / " + name)
                swept_faces += 1
            sweeps += 1
    # Closed-cover tests use seated X/Z; rail float is not extra USB clearance.
    for dy in (-m.PCB_END_CLEARANCE, m.PCB_END_CLEARANCE):
        for name in ("pcb", "top", "underside", "usb"):
            moved = electronics[name].translate((0, dy, 0))
            for shape in (shell, cover):
                clear(moved, shape, "Seated PCB endplay / " + name)
    for dx, dy, dz in itertools.product(
        (-m.PCB_SIDE_CLEARANCE, m.PCB_SIDE_CLEARANCE),
        (-m.PCB_END_CLEARANCE, m.PCB_END_CLEARANCE),
        (-0.1, m.PCB_TOP_CLEARANCE + 0.1),
    ):
        assert (
            overlap(pcb.translate((dx, dy, dz)), shell) > 1e-5
        ), "PCB escapes lower or upper guide capture"
    for dx in (-m.PCB_SIDE_CLEARANCE - 0.1, m.PCB_SIDE_CLEARANCE + 0.1):
        assert (
            overlap(pcb.translate((dx, 0, 0)), shell) > 1e-5
        ), "PCB lateral guide stop missing"
    assert (
        overlap(pcb.translate((0, -0.35, 0)), shell) > 1e-5
    ), "PCB far-end stops missing"
    assert (
        overlap(pcb.translate((0, 0.35, 0)), cover) > 1e-5
    ), "Cover does not stop PCB withdrawal"

    lower_capture = m.PCB_RAIL_LOWER_OVERLAP - m.PCB_SIDE_CLEARANCE
    assert lower_capture >= 0.2 - 1e-8
    # Retain the printed sliding clearance beneath wider upper lips. The
    # narrower opening ABOVE the board is intentional: it now stops lifting.
    throat_width = m.PCB_WIDTH - 2 * m.PCB_RAIL_UPPER_OVERLAP
    assert abs(m.PCB_RAIL_UPPER_OVERLAP - 0.3) < 1e-8
    assert abs(throat_width - 17.4) < 1e-8
    assert m.PCB_WIDTH + 2 * m.PCB_SIDE_CLEARANCE >= 19.2 - 1e-8
    assert m.PCB_THICKNESS + m.PCB_TOP_CLEARANCE >= 1.1 - 1e-8
    clear(
        m.box(
            left - m.PCB_SIDE_CLEARANCE,
            right + m.PCB_SIDE_CLEARANCE,
            corner_end + 0.1,
            m.REAR,
            m.PCB_BOTTOM,
            m.PCB_TOP + m.PCB_TOP_CLEARANCE,
        ),
        shell,
        "Proven 19.2 x 1.1 mm sliding channel was narrowed",
    )
    for side in (-1, 1):
        # Check real bearing lands at maximum lateral PCB travel behind the lead-in.
        def band(inset0, inset1, y0, y1, z0, z1):
            edge = left if side == -1 else right
            x0, x1 = sorted((edge - side * inset0, edge - side * inset1))
            return m.box(x0, x1, y0, y1, z0, z1)

        land = band(
            m.PCB_SIDE_CLEARANCE + 0.01,
            m.PCB_RAIL_LOWER_OVERLAP - 0.01,
            corner_end + 0.1,
            m.REAR - m.PCB_RAIL_ENTRY_CHAMFER - 0.1,
            m.PCB_BOTTOM - 0.1,
            m.PCB_BOTTOM,
        )
        assert (
            abs(overlap(land, shell) - land.volume) < 1e-5
        ), "PCB lower bearing is missing or chamfered away"
        upper_land = band(
            insertion_side_play + 0.01,
            m.PCB_RAIL_UPPER_OVERLAP - 0.01,
            corner_end + 0.1,
            m.REAR - m.PCB_RAIL_ENTRY_CHAMFER - 0.1,
            m.PCB_TOP + m.PCB_TOP_CLEARANCE,
            m.PCB_TOP + m.PCB_TOP_CLEARANCE + 0.1,
        )
        assert (
            abs(overlap(upper_land, shell) - upper_land.volume) < 1e-5
        ), "Upper rail does not capture the centered PCB edge"
        # Probe well behind the far-corner holders so they cannot hide missing rails.
        edge = band(0, 0.4, corner_end + 2, m.REAR - 1, m.PCB_BOTTOM, m.PCB_TOP)
        for dx in (-insertion_side_play, 0, insertion_side_play):
            clear(
                edge.translate((dx, 0, 0)),
                shell,
                "PCB edge is pinched by the new upper rail",
            )
            assert (
                overlap(edge.translate((dx, 0, m.PCB_TOP_CLEARANCE + 0.1)), shell)
                > 1e-5
            ), "PCB edge lifts past its upper rail"
            # Raising USB-C pivots the board about its far edge. Use only the
            # long edge strip so the far-corner holders cannot satisfy this check.
            tilted = edge.translate((dx, 0, 0)).rotate(
                Axis((m.TOWER_X, m.PCB_FRONT, m.PCB_TOP), (1, 0, 0)), 2
            )
            assert overlap(tilted, shell) > 1e-5, "USB-end tilt escapes the upper rail"
        # The mouth opens both slot lips and returns to full lands behind the bevel.
        for inset0, inset1, z0, z1 in (
            (0.15, 0.2, m.PCB_BOTTOM - 0.05, m.PCB_BOTTOM - 0.025),
            (
                0.15,
                0.2,
                m.PCB_TOP + m.PCB_TOP_CLEARANCE + 0.025,
                m.PCB_TOP + m.PCB_TOP_CLEARANCE + 0.05,
            ),
        ):
            clear(
                band(inset0, inset1, m.REAR - 0.05, m.REAR - 0.025, z0, z1),
                shell,
                "PCB slot entry chamfer missing",
            )
            behind = band(
                inset0,
                inset1,
                m.REAR - m.PCB_RAIL_ENTRY_CHAMFER - 0.1,
                m.REAR - m.PCB_RAIL_ENTRY_CHAMFER - 0.05,
                z0,
                z1,
            )
            assert (
                abs(overlap(behind, shell) - behind.volume) < 1e-6
            ), "PCB entry chamfer consumes the bearing land behind it"
    pocket = m.box(
        m.COVER_POCKET_LEFT,
        m.COVER_POCKET_RIGHT,
        m.REAR,
        m.COVER_POCKET_BACK,
        m.COVER_POCKET_BOTTOM,
        m.COVER_POCKET_TOP,
    )
    clear(pocket, cover, "Cover pocket still has projecting tabs")
    assert (
        abs(shell.bounding_box().max.Y - m.REAR) < 1e-6
    ), "Base protrudes beyond the cover seam"
    report["flat_base_floor_mm"] = m.BASE_FLOOR_THICKNESS
    report["pcb_guides"] = {
        "pcb_bottom_z_mm": m.PCB_BOTTOM,
        "pcb_to_floor_mm": m.PCB_FLOOR_GAP,
        "provisional_underside_to_floor_mm": underside_gap,
        "side_clearance_mm": m.PCB_SIDE_CLEARANCE,
        "top_clearance_mm": m.PCB_TOP_CLEARANCE,
        "slot_height_mm": m.PCB_THICKNESS + m.PCB_TOP_CLEARANCE,
        "upper_throat_width_mm": throat_width,
        "channel_width_mm": m.PCB_WIDTH + 2 * m.PCB_SIDE_CLEARANCE,
        "upper_edge_overlap_mm": m.PCB_RAIL_UPPER_OVERLAP,
        "component_safe_insertion_side_play_mm": insertion_side_play,
        "minimum_upper_capture_with_centered_insertion_mm": m.PCB_RAIL_UPPER_OVERLAP
        - insertion_side_play,
        "usb_end_upward_tilt_blocked_at_degrees": 2,
        "vertical_retention": "Both upper rails catch centered PCB edges; rear cover locates USB. Full +/-0.6 mm bare-board float is not component-safe with the wider lips",
        "wall_haunch_angle_degrees": 45,
        "added_triangle_probe_volume_per_side_mm3": triangle.volume,
        "positive_wall_bridge_probe_volume_per_side_mm3": wall_bridge.volume,
        "rear_seam_wall_bridge_probe_volume_per_side_mm3": rear_wall_bridge.volume,
        "far_corner_holder_count": 2,
        "far_corner_length_across_board_mm": m.PCB_CORNER_LENGTH,
        "far_corner_nominal_overlap_y_mm": m.PCB_CORNER_OVERLAP,
        "far_corner_full_land_at_rear_limit_mm": m.PCB_CORNER_OVERLAP
        - m.PCB_END_CLEARANCE
        - m.PCB_CORNER_ENTRY_CHAMFER,
        "central_front_opening_mm": middle_width,
        "bare_corner_component_assumption_mm": [
            relieved_width,
            m.PCB_CORNER_CLEAR_DEPTH,
        ],
        "top_component_proxy_removed_volume_mm3": expected_removed,
        "lower_bearing_at_lateral_limit_mm": lower_capture,
        "rear_entry_chamfer_mm": m.PCB_RAIL_ENTRY_CHAMFER,
        "endplay_mm": [-m.PCB_END_CLEARANCE, m.PCB_END_CLEARANCE],
        "continuous_hardware_sweeps": sweeps,
        "swept_rear_faces": swept_faces,
        "insertion_translation_y_mm": [-m.PCB_END_CLEARANCE, 30],
        "physical_fit": "User printed the extended-rail coupon and accepted it as a good fit on 2026-09-19; model set finalized. Upper overlap is 0.3 mm per edge, channel 19.2 mm, slot 1.1 mm. Earlier wide coupon admitted USB-end lifting. This result applies to the tested board; component/solder envelopes remain provisional",
    }
    report["checks"].append(
        "Flat 2 mm floor, 45-degree wall braces, 0.3 mm upper edge overlaps, unchanged 19.2 x 1.1 mm sliding channel and independent lift / 2-degree USB-end tilt stops on both rails verified"
    )
    report["checks"].append(
        "All PCB/component/USB envelopes clear centered rear insertion at X=0 and +/-0.15 mm, across vertical slot play; seated endplay clears the closed cover. Full +/-0.6 mm bare-board side play is not a component-clearance claim"
    )


def compact_led_checks(parts, report):
    shell = parts["04_tower_black"]
    cover = parts["05_rear_cover_black"]
    retainer = parts["06_LED_retainer_black"]
    electronics = m.electronics()
    assert (
        m.LED_DIAMETERS == (4.75,) * 3 and m.LED_BORE_DIAMETERS == (5.1,) * 3
    ), "Compact body must retain the measured LEDs and fitted bores"
    assert (m.PCB_WIDTH, m.PCB_LENGTH, m.PCB_THICKNESS) == (
        18,
        22.4,
        0.7,
    ), "Compact body must retain the measured PCB"
    assert (
        m.LED_TOTAL_LENGTH == 8.5
        and abs(m.LED_BARREL_LENGTH - (8.5 - m.LED_FLANGE_THICKNESS)) < 1e-8
    )
    assert m.UPRIGHT_WIDTH <= 14 and m.TOWER_HEIGHT <= 45.5 and m.MASCOT_DEPTH <= 20.5
    assert np.allclose(np.diff(m.LED_Z), 6.5), "LED stack pitch changed"
    red_margin = m.TOWER_HEIGHT - m.LED_Z[-1] - m.LED_DIAMETERS[-1] / 2
    assert (
        0 < red_margin <= m.LED_DIAMETERS[-1]
    ), "Red LED must remain close to the tower top"
    assert (
        m.LED_Z[-1] > parts["01_mascot_orange"].bounding_box().max.Z
    ), "Red LED fell below the mascot head"
    flange_gaps = [
        b - a - (d0 + d1) / 2
        for a, b, d0, d1 in zip(m.LED_Z, m.LED_Z[1:], m.LED_FLANGES, m.LED_FLANGES[1:])
    ]
    mouth_webs = [
        b - a - (d0 + d1) / 2 - 2 * m.LED_BORE_LEAD_IN
        for a, b, d0, d1 in zip(
            m.LED_Z, m.LED_Z[1:], m.LED_BORE_DIAMETERS, m.LED_BORE_DIAMETERS[1:]
        )
    ]
    assert (
        min(flange_gaps) >= 0.6 - 1e-8 and min(mouth_webs) >= 0.6 - 1e-8
    ), "Adjacent LED flanges or entry mouths are too close"
    assert (
        abs(cover.bounding_box().min.Y - m.REAR) < 1e-6
    ), "Long LED fingers remain on the rear cover"
    socket_gaps = []
    for y, z in j.JOINT_CENTERS_YZ:
        boss = j.diamond_socket(
            j.JOINT_X,
            j.JOINT_X + j.SOCKET_DEPTH + m.HAND_SOCKET_WALL,
            y,
            z,
            j.SOCKET_DIAGONAL + 2 * m.HAND_SOCKET_WALL,
        )
        socket_gaps.append(boss.distance_to(retainer))
    assert (
        min(socket_gaps) >= 0.25 - 1e-6
    ), "Retainer is too close to the hand-socket haunches"

    bearing_areas = []
    projection_ranges = []
    for i, (z, flange, bore) in enumerate(
        zip(m.LED_Z, m.LED_FLANGES, m.LED_BORE_DIAMETERS)
    ):
        led = electronics[f"led_{i}"]
        bounds = led.bounding_box()
        assert (
            abs(bounds.size.Y - m.LED_TOTAL_LENGTH) < 1e-6
        ), "LED model omits or double-counts its flange thickness"
        projection_ranges.append([-bounds.min.Y - m.LED_AXIAL_CLEARANCE, -bounds.min.Y])
        assert (
            3 - 1e-8
            <= projection_ranges[-1][0]
            <= projection_ranges[-1][1]
            <= 3.5 + 1e-8
        )
        assert abs(m.LED_SUPPORT_Y - bounds.max.Y - m.LED_AXIAL_CLEARANCE) < 1e-6
        # The narrow but complete annulus must end at a flat, unchamfered shoulder.
        ring = m.cy(m.TOWER_X, z, m.LED_SEAT_Y - 0.1, m.LED_SEAT_Y, flange)
        ring -= m.cy(m.TOWER_X, z, m.LED_SEAT_Y - 0.11, m.LED_SEAT_Y + 0.01, bore)
        assert (
            abs(overlap(ring, shell) - ring.volume) < 1e-6
        ), "LED collar lacks its full flat flange-bearing ring"
        clear(
            ring.translate((0, 0.1, 0)),
            shell,
            "LED collar extends behind its flange seat",
        )
        clear(
            m.cy(m.TOWER_X, z, 0, m.LED_SEAT_Y, bore),
            shell,
            "Fitted LED bore is obstructed",
        )
        for dy in (0, m.LED_AXIAL_CLEARANCE):
            for part in (shell, retainer, cover):
                clear(led.translate((0, dy, 0)), part, "LED axial-play envelope")
        assert (
            overlap(led.translate((0, -0.1, 0)), shell) > 1e-5
        ), "Front collar does not stop the LED flange"
        radial = (bore - m.LED_DIAMETERS[i]) / 2
        for dx, dz in ((0, 0), (-radial, 0), (radial, 0), (0, -radial), (0, radial)):
            assert (
                overlap(led.translate((dx, m.LED_AXIAL_CLEARANCE + 0.1, dz)), retainer)
                > 1e-5
            ), "Retainer pad misses the LED flange at radial travel limit"
        tip = next(
            f
            for f in retainer.faces()
            if abs(f.bounding_box().min.Y - m.LED_SUPPORT_Y) < 1e-6
            and abs(f.bounding_box().max.Y - m.LED_SUPPORT_Y) < 1e-6
            and abs(f.center().Z - z) < 1e-6
        )
        disk = next(
            f
            for f in m.cy(
                m.TOWER_X, z, m.LED_SUPPORT_Y, m.LED_SUPPORT_Y + 0.1, flange
            ).faces()
            if abs(f.bounding_box().min.Y - m.LED_SUPPORT_Y) < 1e-6
            and abs(f.bounding_box().max.Y - m.LED_SUPPORT_Y) < 1e-6
        )
        contact = tip.intersect(disk)
        area = (
            contact.area
            if hasattr(contact, "area")
            else sum(face.area for face in contact)
        )
        assert (
            area > 1
        ), "Chamfer leaves too little LED flange bearing on the retainer pad"
        bearing_areas.append(area)
        core = m.box(
            m.RETAINER_LEFT + 0.25,
            m.RETAINER_RIGHT - 0.25,
            m.LED_SUPPORT_Y + 0.25,
            m.RETAINER_BACK_Y - 0.25,
            z - 1.2,
            z + 1.2,
        )
        assert (
            abs(overlap(core, retainer) - core.volume) < 1e-5
        ), "Short LED pad is not continuously joined to the retainer spine"

    roof = m.TOWER_HEIGHT - m.WALL
    key = retainer & m.box(
        m.RETAINER_LEFT - 1,
        m.RETAINER_RIGHT + 1,
        m.RETAINER_MOUNT_Y - 1,
        m.RETAINER_BACK_Y + 1,
        roof,
        m.RETAINER_TOP + 1,
    )
    assert (
        key.is_valid
        and key.volume > 0
        and abs(key.bounding_box().size.Z - m.RETAINER_KEY_ENGAGEMENT) < 1e-6
    )
    clear(key, shell, "Roof tab does not fit its pocket")
    pocket = m.retainer_key_pocket()
    skin = m.box(
        m.RETAINER_LEFT,
        m.RETAINER_RIGHT,
        m.RETAINER_MOUNT_Y,
        m.RETAINER_BACK_Y,
        m.RETAINER_TOP + m.RETAINER_KEY_CLEARANCE,
        m.TOWER_HEIGHT,
    )
    assert (
        skin.bounding_box().size.Z >= 1
        and abs(overlap(skin, shell) - skin.volume) < 1e-5
    ), "Roof key pocket loses its continuous outer skin"
    clear(pocket, shell, "Roof key pocket is obstructed")
    key_stop = 0.5
    # The rear pocket wall slopes at 45 degrees; use enough displacement to
    # reach it below the pocket ceiling, rather than just its nominal clearance.
    assert (
        overlap(key.translate((0, 1, 0)), shell) > 1e-5
    ), "Top tab does not stop rearward pull"
    assert (
        overlap(key.translate((0, -key_stop, 0)), shell) > 1e-5
    ), "Top tab has no front restraint"
    for dx in (-key_stop, key_stop):
        assert (
            overlap(key.translate((dx, 0, 0)), shell) > 1e-5
        ), "Roof key lacks a lateral stop"
    screw_x, screw_z = m.LED_SCREWS[0]
    for direction in ((0, 1, 0), (1, 0, 0)):
        pivot = Axis((screw_x, m.RETAINER_MOUNT_Y, screw_z), direction)
        for angle in (-2, 2):
            assert (
                overlap(key.rotate(pivot, angle), shell) > 1e-5
            ), "Top key does not restrain rotation about the lower screw"
    shaft = m.cy(
        screw_x, screw_z, m.RETAINER_MOUNT_Y, m.RETAINER_BACK_Y, m.SCREW_DIAMETER
    )
    assert (
        overlap(shaft, retainer.translate((0, 0, -m.RETAINER_RELEASE_DROP))) > 1e-5
    ), "Installed screw does not prevent key disengagement"

    # Fill the removed screw hole solely for a conservative service envelope.
    # Its exterior is then entirely planar, so sweeping each forward-facing
    # face proves continuous translation without position sampling.
    service = retainer + m.cy(
        screw_x, screw_z, m.RETAINER_MOUNT_Y, m.RETAINER_BACK_Y, m.COUNTERSINK_DIAMETER
    )
    assert all(
        f.geom_type == GeomType.PLANE for f in service.faces()
    ), "Service envelope must have a polygonal exterior"
    obstacles = [
        shape
        for name, shape in parts.items()
        if name not in ("05_rear_cover_black", "06_LED_retainer_black")
    ]
    obstacles.extend(electronics.values())
    for z in m.LED_Z:
        for dz in (-1.27, 1.27):
            obstacles.append(
                m.cy(m.TOWER_X, z + dz, m.LED_SEAT_Y + m.LED_FLANGE_THICKNESS, 18, 0.7)
            )

    def sweep_clear(shape, amount, direction, obstacles, message):
        faces = [
            f
            for f in shape.faces()
            if sum(a * b for a, b in zip(f.normal_at(), direction)) > 1e-7
        ]
        assert faces, message + " has no sweep faces"
        for obstacle in obstacles:
            clear(shape, obstacle, message + " initial pose")
        for face in faces:
            sweep = extrude(face, amount=amount, dir=direction)
            assert sweep.is_valid, message + " sweep is invalid"
            for obstacle in obstacles:
                clear(sweep, obstacle, message)
        return len(faces)

    drop_faces = sweep_clear(
        service,
        m.RETAINER_RELEASE_DROP,
        (0, 0, -1),
        obstacles,
        "Continuous retainer key release including LED leads",
    )
    dropped = service.translate((0, 0, -m.RETAINER_RELEASE_DROP))
    assert (
        dropped.bounding_box().max.Z < roof
    ), "Retainer cannot clear the roof after its release drop"
    travel = 40
    retainer_faces = sweep_clear(
        dropped,
        travel,
        (0, 1, 0),
        obstacles,
        "Continuous lowered-retainer withdrawal including LED leads",
    )
    cover_obstacles = [
        shape for name, shape in parts.items() if name != "05_rear_cover_black"
    ] + list(electronics.values())
    cover_faces = sweep_clear(
        cover, travel, (0, 1, 0), cover_obstacles, "Continuous rear-cover withdrawal"
    )
    posed = m.print_pose("06_LED_retainer_black", retainer)
    for face in posed.faces():
        if face.bounding_box().max.Z > 1e-6:
            assert (
                face.normal_at().Z >= -sqrt(0.5) - 1e-6
            ), "Retainer needs support above its flat rear print face"
    # All roof-pocket faces are exposed in front-down tower printing.
    pocket_faces = [
        f
        for f in shell.faces()
        if f.bounding_box().min.X >= m.RETAINER_LEFT - m.RETAINER_KEY_CLEARANCE - 1e-6
        and f.bounding_box().max.X <= m.RETAINER_RIGHT + m.RETAINER_KEY_CLEARANCE + 1e-6
        and f.bounding_box().min.Y
        >= m.RETAINER_MOUNT_Y - m.RETAINER_KEY_CLEARANCE - 1e-6
        and f.bounding_box().min.Z >= roof - 1e-6
    ]
    assert pocket_faces and all(
        f.normal_at().Y >= -sqrt(0.5) - 1e-6 for f in pocket_faces
    ), "Roof pocket has an unsupported rear closure"
    assert any(
        abs(f.normal_at().Y + sqrt(0.5)) < 1e-6 for f in pocket_faces
    ), "Roof pocket rear support ramp is missing"
    report["compact_layout"] = {
        "mascot_body_width_height_depth_mm": [
            50 * m.MASCOT_SCALE,
            55 * m.MASCOT_SCALE,
            m.MASCOT_DEPTH,
        ],
        "base_width_height_depth_mm": [m.BASE_WIDTH, m.BASE_HEIGHT, m.DEPTH],
        "tower_width_height_mm": [m.UPRIGHT_WIDTH, m.TOWER_HEIGHT],
        "led_center_z_mm": list(m.LED_Z),
        "led_pitch_mm": m.LED_PITCH,
        "red_tip_to_tower_top_mm": red_margin,
        "minimum_front_mouth_web_mm": min(mouth_webs),
        "minimum_adjacent_flange_gap_mm": min(flange_gaps),
    }
    report["led_capture"] = {
        "measured_total_plastic_length_mm": m.LED_TOTAL_LENGTH,
        "barrel_without_flange_mm": m.LED_BARREL_LENGTH,
        "flange_thickness_mm": m.LED_FLANGE_THICKNESS,
        "front_flange_seat_y_mm": m.LED_SEAT_Y,
        "retainer_pad_face_y_mm": m.LED_SUPPORT_Y,
        "axial_play_mm": m.LED_AXIAL_CLEARANCE,
        "protrusion_ranges_mm": projection_ranges,
        "short_pad_count": 3,
        "internal_LED_screws": len(m.LED_SCREWS),
        "separate_retainer_parts": 1,
        "flat_chamfered_pad_bearing_area_mm2": bearing_areas,
        "minimum_hand_socket_clearance_mm": min(socket_gaps),
        "roof_key_engagement_mm": m.RETAINER_KEY_ENGAGEMENT,
        "roof_skin_above_pocket_mm": skin.bounding_box().size.Z,
        "key_release_drop_mm": m.RETAINER_RELEASE_DROP,
        "key_rotation_stop_test_degrees": [-2, 2],
        "continuous_retainer_withdrawal_mm": travel,
        "retainer_service_swept_faces": {
            "drop": drop_faces,
            "rearward": retainer_faces,
        },
        "continuous_cover_withdrawal_mm": travel,
        "cover_swept_faces": cover_faces,
        "physical_fit": "Separate keyed retainer, collar fit and retention strength are unprinted; wire insulation and routing remain provisional",
    }
    report["checks"].append(
        "Measured 8.5 mm total LED length includes the flange; flat front collar rings and short retainer pads limit protrusion to 3.25–3.5 mm across LED radial travel"
    )
    report["checks"].append(
        "One lower screw prevents release while the rigid roof key blocks pull and rotation; roof skin, printable pocket ramp, hand-haunch clearance and continuous drop-then-rearward service motion including lead envelopes verified"
    )
    report["checks"].append(
        "Rear cover has no LED fingers; both cover and separate retainer print rear-face down without above-bed overhangs exceeding 45 degrees"
    )


def main(check_mesh=True):
    p = m.parts()
    printable = m.print_parts()
    coupons = m.coupon_parts()
    report = {
        "status": "passed",
        "hardware_fit": "User accepted the printed extended-rail PCB coupon as a good fit and finalized all models on 2026-09-19. Complete wired-assembly fit and separate keyed LED-retainer/collar fit are not established by that coupon. Measured hardware and previously fitted LED/screw diameters are retained; component/solder envelopes remain provisional, including two assumed bare 2.7 x 1.0 mm top corner patches. Physical assembly, retention strength and USB projection require checking on this revision.",
        "mesh": {},
        "checks": [],
    }
    for n, s in p.items():
        assert s.is_valid and len(s.solids()) == 1 and s.volume > 0, n
        # Rectangular style: no spherical/toroidal fillet surfaces in printed parts.
        assert all(
            f.geom_type in (GeomType.PLANE, GeomType.CYLINDER, GeomType.CONE)
            for f in s.faces()
        ), n
        assert any(
            f.geom_type == GeomType.PLANE
            and sum(abs(v) > 1e-3 for v in f.normal_at()) >= 2
            for f in s.faces()
        ), f"{n}: missing flat bevel faces"
    report["checks"].append("All printed parts have flat chamfers; no fillet surfaces")
    left_eye = m.print_pose("02_eye_left_black", p["02_eye_left_black"])
    right_eye = m.print_pose("03_eye_right_black", p["03_eye_right_black"])
    common = overlap(left_eye, right_eye)
    assert (
        abs(common - left_eye.volume) < 1e-6 and abs(common - right_eye.volume) < 1e-6
    ), "Eyes do not share the same normalized print geometry"
    assert len(p) == 6 and len(printable) == 5 and len(coupons) == 2
    assert m.print_quantity("02_eye_black_x2") == 2
    assert sum(m.print_quantity(name) for name in printable) == len(p)
    assert (
        "02_eye_left_black" not in printable and "03_eye_right_black" not in printable
    )
    report["print_inventory"] = {
        "assembly_occurrences": len(p),
        "unique_assembly_STLs": len(printable),
        "coupon_STLs": len(coupons),
        "quantities": {name: m.print_quantity(name) for name in printable},
    }
    report["checks"].append(
        "Both eyes have identical normalized solids; one eye STL with quantity two preserves all six assembly occurrences"
    )
    # The uniformly scaled mascot prints rear down. No recess face may form a shelf;
    # both ends must instead have 45° faces with opposite Y normal signs.
    recess_faces = [
        f
        for f in p["01_mascot_orange"].faces()
        if f.bounding_box().min.X >= 29.9 * m.MASCOT_SCALE
        and f.bounding_box().min.Y > 10 * m.MASCOT_SCALE
        and f.bounding_box().max.Y < 30 * m.MASCOT_SCALE
    ]
    normals = [f.normal_at().Y for f in recess_faces]
    limit = 2**-0.5
    assert normals and all(
        abs(y) <= limit + 1e-6 for y in normals
    ), "Hand recess has a shelf steeper than 45° overhang"
    assert any(abs(y - limit) < 1e-6 for y in normals) and any(
        abs(y + limit) < 1e-6 for y in normals
    ), "Both hand recess ends need 45° ramps"
    report["checks"].append(
        "Both hand recess ends have full-depth 45-degree ramps; no horizontal recess ledges in rear-down print pose"
    )
    for (na, a), (nb, b) in itertools.combinations(p.items(), 2):
        clear(a, b, f"{na} / {nb}")
    report["checks"].append(
        f"{len(p)} valid single CAD solids; all {len(p)*(len(p)-1)//2} printed-part pairs have zero interfering volume"
    )
    for en, e in m.electronics().items():
        for pn, s in p.items():
            clear(e, s, f"{en} / {pn}")
    report["checks"].append(
        "PCB, USB-shell and LED envelopes and provisional component envelopes (top has two declared corner reliefs) clear all printed parts"
    )
    screw_checks(p, report)
    coupon_checks(coupons, report)
    hand_joint_checks(p, report)
    shoulder_checks(p["04_tower_black"], report)
    cover_checks(p["05_rear_cover_black"], report)
    pcb_guide_checks(p, report)
    compact_led_checks(p, report)
    shell = p["04_tower_black"]
    for z in m.LED_Z:
        for dz in (-1.27, 1.27):
            lead = m.cy(
                m.TOWER_X, z + dz, m.LED_SEAT_Y + m.LED_FLANGE_THICKNESS, 18, 0.7
            )
            for name, s in p.items():
                clear(lead, s, "Straight LED lead corridor " + name)
        resistor = m.cy(m.TOWER_X, z, 15, 22, 2.5)
        for name, s in p.items():
            clear(resistor, s, "Resistor envelope " + name)
    route_y1 = m.REAR - m.WIRE_REAR_CLEARANCE
    route_y0 = route_y1 - m.WIRE_CORRIDOR_DEPTH
    assert route_y1 - route_y0 >= 6, "No room for a 6 mm rear wire corridor"
    route_bottom = max(
        m.PCB_TOP + m.PCB_TOP_HEIGHT + 1, m.USB_CENTER_Z + m.USB_HEIGHT / 2 + 0.5
    )
    # Explicit provisional space for individual insulated wires; real wire
    # insulation diameters and the bundle arrangement remain unmeasured.
    route = m.box(
        m.TOWER_X - 1.65,
        m.TOWER_X + 0.95,
        route_y0,
        route_y1,
        route_bottom,
        max(m.LED_Z) + 2,
    )
    for shape in p.values():
        clear(route, shape, "Provisional central wire-routing lane")
    report["wire_corridor"] = {
        "width_depth_mm": [2.6, m.WIRE_CORRIDOR_DEPTH],
        "x_limits_mm": [m.TOWER_X - 1.65, m.TOWER_X + 0.95],
        "y_limits_mm": [route_y0, route_y1],
        "minimum_printed_part_clearance_mm": min(
            route.distance_to(shape) for shape in p.values()
        ),
        "status": "Free geometry only; actual insulated-wire diameters and bundle arrangement are unmeasured",
    }
    metal_nose = m.box(
        m.TOWER_X - m.USB_WIDTH / 2,
        m.TOWER_X + m.USB_WIDTH / 2,
        m.USB_REAR,
        m.USB_CABLE_BODY_START,
        m.USB_CENTER_Z - m.USB_HEIGHT / 2,
        m.USB_CENTER_Z + m.USB_HEIGHT / 2,
    )
    cable = m.box(
        m.TOWER_X - 6,
        m.TOWER_X + 6,
        m.USB_CABLE_BODY_START,
        m.DEPTH + 20,
        m.USB_CENTER_Z - 3.5,
        m.USB_CENTER_Z + 3.5,
    )
    for s in p.values():
        clear(metal_nose, s, "Metal USB plug-nose insertion envelope")
        clear(cable, s, "12 x 7 mm cable-overmould insertion envelope")
    report["usb_fit"] = {
        "measured_socket_width_height_mm": [m.USB_WIDTH, m.USB_HEIGHT],
        "aperture_width_height_mm": [m.USB_OPENING_WIDTH, m.USB_OPENING_HEIGHT],
        "per_side_clearance_x_z_mm": [m.USB_CLEARANCE_X, m.USB_CLEARANCE_Z],
        "corner_chamfer_mm": m.USB_CORNER_CHAMFER,
        "socket_to_exterior_face_recess_mm": m.DEPTH - m.USB_REAR,
        "user_measured_vertical_correction_mm": m.USB_SEATING_Z_OFFSET,
        "aperture_center_z_mm": m.USB_CENTER_Z,
        "entry_chamfer_mm": m.USB_ENTRY_CHAMFER,
        "provisional_cable_body_width_height_mm": [12, 7],
        "provisional_cable_body_start_y_mm": m.USB_CABLE_BODY_START,
        "assumed_exposed_metal_ahead_of_plastic_mm": m.USB_CABLE_BODY_START
        - m.USB_REAR,
        "flat_cover_skin_mm": m.COVER_INNER_SKIN,
        "cover_pcb_stop_backing_mm": m.DEPTH - m.PCB_REAR_STOP_FRONT,
        "physical_fit": "Extended-rail PCB coupon fit physically accepted by the user on 2026-09-19. The unchanged USB opening and earlier axial USB recess have not been remeasured on this geometry. Cable plastic remains unmeasured.",
    }
    report["checks"].append(
        "Lead, 2.5 x 7 mm resistor, wire, USB cable and screwdriver access envelopes clear"
    )
    for x in (-22.5, -12.5, 12.5, 22.5):
        foot = m.box(
            (x - 1) * m.MASCOT_SCALE,
            (x + 1) * m.MASCOT_SCALE,
            10 * m.MASCOT_SCALE,
            30 * m.MASCOT_SCALE,
            0,
            0.1 * m.MASCOT_SCALE,
        )
        assert abs(overlap(foot, p["01_mascot_orange"]) - foot.volume) < 1e-5
    assert abs(shell.bounding_box().min.Z) < 1e-6
    report["checks"].append("All four feet and tower pedestal contact Z=0")
    if not check_mesh:
        report["status"] = "cad-passed-stl-not-checked"
        print(json.dumps(report, indent=2))
        return report
    stl_files = sorted((m.OUT / "stl").rglob("*.stl"))
    assert {f.stem for f in stl_files} == set(printable) | set(
        coupons
    ), "STL directory has missing parts or stale exports"
    for f in stl_files:
        expected_parent = m.OUT / "stl" / ("coupons" if f.stem in coupons else "")
        assert f.parent == expected_parent, "STL is in the wrong folder: " + str(f)
        mesh = trimesh.load_mesh(f)
        assert (
            mesh.is_watertight and mesh.is_winding_consistent and mesh.is_volume
        ), f.name
        assert len(mesh.split()) == 1, f.name
        assert abs(mesh.bounds[0, 2]) < 1e-5, f.name
        n = f.stem
        if n in printable:
            assert np.allclose(
                mesh.extents,
                list(m.print_pose(n, printable[n]).bounding_box().size),
                atol=0.025,
            )
            assert abs(mesh.volume - printable[n].volume) / printable[n].volume < 0.002
        elif n in coupons:
            assert np.allclose(
                mesh.extents, list(coupons[n].bounding_box().size), atol=0.025
            )
            assert abs(mesh.volume - coupons[n].volume) / coupons[n].volume < 0.002
        else:
            raise AssertionError("Unexpected stale STL: " + f.name)
        report["mesh"][n] = {
            "watertight": True,
            "components": 1,
            "triangles": len(mesh.faces),
            "dimensions_mm": mesh.extents.round(3).tolist(),
            "bed_z_mm": float(mesh.bounds[0, 2]),
            "quantity": m.print_quantity(n) if n in printable else 1,
        }
    (m.OUT / "validation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--cad-only",
        action="store_true",
        help="Check current CAD without reading STLs or writing validation.json",
    )
    main(check_mesh=not parser.parse_args().cad_only)
