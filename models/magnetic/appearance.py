"""Assembled web view: printable parts, magnets and visible electronics, not a print file."""

from build123d import Compound
from refined import base, magnets, parts


def gen_step():
    e = base.electronics()
    return Compound(
        children=list(parts().values())
        + [base.eye(12.5)]
        + [e[k] for k in ("pcb", "usb", "led_0", "led_1", "led_2")]
        + list(magnets().values()),
        label="Appearance_magnetic_with_REFERENCE_hardware_do_not_print",
    )
