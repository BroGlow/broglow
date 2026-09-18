#!/usr/bin/env python3
"""Draw original geometric status icons. Run with --check to verify committed PNGs."""
import argparse
from pathlib import Path
import struct
import zlib

SIZE = 36
COLORS = {"red": (255, 69, 58), "yellow": (255, 159, 10),
          "green": (52, 199, 89), "none": (142, 142, 147)}


def rounded_body(x, y, inset=0):
    radius = 5 - inset
    dx = max(14 - x, 0, x - 22)
    dy = max(7 - y, 0, y - 29)
    return dx * dx + dy * dy <= radius * radius


def sample(x, y, state):
    if not rounded_body(x, y):
        return None
    if not rounded_body(x, y, 1.25):
        return (170, 178, 189)
    for color, center in (("red", 10), ("yellow", 18), ("green", 26)):
        if (x - 18) ** 2 + (y - center) ** 2 <= 3.25 ** 2:
            return COLORS[color] if state == color else COLORS["none"] if state == "none" else (65, 74, 87)
    return (31, 41, 55)


def render(state):
    raw = bytearray()
    for y in range(SIZE):
        raw.append(0)  # PNG scanline filter: None.
        for x in range(SIZE):
            colors = [sample(x + (sx + .5) / 4, y + (sy + .5) / 4, state)
                      for sy in range(4) for sx in range(4)]
            opaque = [color for color in colors if color is not None]
            raw.extend((*[round(sum(c[i] for c in opaque) / len(opaque)) for i in range(3)],
                        round(255 * len(opaque) / 16)) if opaque else (0, 0, 0, 0))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">2I5B", SIZE, SIZE, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b""))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    for state in COLORS:
        path = Path(__file__).parent / f"clawd-{state}.png"
        pixels = render(state)
        if args.check:
            assert path.read_bytes() == pixels, f"Regenerate {path.name} with {__file__}"
        else:
            path.write_bytes(pixels)
    print("PASS: four original 36x36 RGBA status icons")
