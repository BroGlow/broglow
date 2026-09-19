#!/usr/bin/env python3
"""Draw original tray and macOS app icons. Run with --check to verify assets."""

import argparse
from pathlib import Path
import struct
import zlib

SIZE = 36
COLORS = {
    "red": (255, 69, 58),
    "yellow": (255, 159, 10),
    "green": (52, 199, 89),
    "none": (142, 142, 147),
}


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
        if (x - 18) ** 2 + (y - center) ** 2 <= 3.25**2:
            return (
                COLORS[color]
                if state in (color, "app")
                else COLORS["none"] if state == "none" else (65, 74, 87)
            )
    return (31, 41, 55)


def render(state, size=SIZE):
    raw = bytearray()
    for y in range(size):
        raw.append(0)  # PNG scanline filter: None.
        for x in range(size):
            colors = [
                sample(
                    (x + (sx + 0.5) / 4) * SIZE / size,
                    (y + (sy + 0.5) / 4) * SIZE / size,
                    state,
                )
                for sy in range(4)
                for sx in range(4)
            ]
            opaque = [color for color in colors if color is not None]
            raw.extend(
                (
                    *[round(sum(c[i] for c in opaque) / len(opaque)) for i in range(3)],
                    round(255 * len(opaque) / 16),
                )
                if opaque
                else (0, 0, 0, 0)
            )

    def chunk(kind, data):
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data))
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">2I5B", size, size, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    assets = {f"broglow-{state}.png": render(state) for state in COLORS}
    icons = bytearray()
    for kind, size in ((b"ic07", 128), (b"ic08", 256), (b"ic09", 512)):
        png = render("app", size)
        icons.extend(kind + struct.pack(">I", len(png) + 8) + png)
    assets["BroGlow.icns"] = b"icns" + struct.pack(">I", len(icons) + 8) + icons
    for name, pixels in assets.items():
        path = Path(__file__).parent / name
        if args.check:
            assert (
                path.read_bytes() == pixels
            ), f"Regenerate {path.name} with {__file__}"
        else:
            path.write_bytes(pixels)
    print("PASS: four original 36x36 status icons and the macOS app icon")
