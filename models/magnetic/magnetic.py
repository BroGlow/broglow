"""Shared geometry for the Ø3×2 magnetic enclosure (millimetres)."""

from math import sqrt
from pathlib import Path
import sys
from build123d import Align, Axis, Cylinder, Face, Wire, extrude

SOURCE = Path(__file__).resolve().parent
sys.path.insert(0, str(SOURCE.parent / "broglow"))
import broglow as base
from check import clear, overlap

OUT = SOURCE.parents[1] / "dist/enclosure/magnetic"
ROOF_BRIDGE = 0.4


def cx(x0, x1, y, z, diameter):
    return (
        Cylinder(diameter / 2, x1 - x0, align=(Align.CENTER, Align.CENTER, Align.MIN))
        .rotate(Axis.Y, 90)
        .translate((x0, y, z))
    )


def pocket(x0, x1, y, z, print_up, diameter=3.05, roof_bridge=ROOF_BRIDGE):
    """Round magnet seat with a small printable roof, not a locating key."""
    r = diameter / 2
    tangent = r / sqrt(2)
    peak = r * sqrt(2) - roof_bridge / 2
    yz = (
        (0, -r),
        (tangent, -tangent),
        (peak, -roof_bridge / 2),
        (peak, roof_bridge / 2),
        (tangent, tangent),
        (0, r),
    )
    roof = Face(
        Wire.make_polygon(
            [(x0, y + print_up * yy, z + zz) for yy, zz in yz], close=True
        )
    )
    return cx(x0, x1, y, z, diameter) + extrude(roof, amount=x1 - x0, dir=(1, 0, 0))
