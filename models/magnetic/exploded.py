"""Assembly explorer: every magnetic part separated, positions from the printable assembly."""

from build123d import Compound
from refined import base, magnets, parts


def gen_step():
    offsets = {
        "01_mascot_magnetic_orange": (-5, 0, 0),
        "02_eye_black_x2": (-5, -12, 0),
        "02_eye_right_black": (-5, -12, 0),
        "03_tower_magnetic_black": (15, 0, 0),
        "04_rear_cover_black": (40, 18, 0),
        "05_LED_retainer_black": (38, 0, 10),
    }
    children = [
        s.translate(offsets[n])
        for n, s in (parts() | {"02_eye_right_black": base.eye(12.5)}).items()
    ]
    e = base.electronics()
    for n in ("pcb", "usb", "led_0", "led_1", "led_2"):
        children.append(
            e[n].translate((15, 35, 0) if n in ("pcb", "usb") else (15, -14, 0))
        )
    for n, s in magnets().items():
        children.append(s.translate((-5, 0, 0) if "mascot" in n else (15, 0, 0)))
    return Compound(
        children=children, label="Exploded_magnetic_with_REFERENCE_hardware"
    )
