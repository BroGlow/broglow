"""Static exploded review: all positions derived from the printable assembly."""

from build123d import Compound
from clawlight import parts, electronics


def gen_step():
    offsets = {
        "01_mascot_orange": (-5, 0, 0),
        "02_eye_left_black": (-5, -12, 0),
        "03_eye_right_black": (-5, -12, 0),
        "04_tower_black": (15, 0, 0),
        "05_rear_cover_black": (40, 18, 0),
        "06_LED_retainer_black": (38, 0, 10),
    }
    children = [s.translate(offsets[n]) for n, s in parts().items()]
    e = electronics()
    for n in ("pcb", "usb", "led_0", "led_1", "led_2"):
        children.append(
            e[n].translate((15, 35, 0) if n in ("pcb", "usb") else (15, -14, 0))
        )
    return Compound(
        children=children, label="Exploded_review_with_PROVISIONAL_hardware"
    )
