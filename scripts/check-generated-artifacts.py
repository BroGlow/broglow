"""Reject top-level docs, CAD exports, archives, disposable previews and inspection JSON."""

from pathlib import Path
import re
import subprocess


def is_generated(path):
    return bool(
        path.lower().startswith("docs/")
        or re.search(r"\.(zip|stl|step|stp|3mf)$", path, re.I)
        or path.lower().startswith("models/broglow/preview/")
        or re.fullmatch(
            r"models/broglow/(?:cad|coupon|usb-coupon)-inspection\.json",
            path,
            re.I,
        )
    )


def main():
    paths = (
        subprocess.check_output(
            ["git", "ls-files", "-z"], cwd=Path(__file__).resolve().parents[1]
        )
        .decode()
        .split("\0")
    )
    forbidden = [p for p in paths if is_generated(p)]
    if forbidden:
        raise SystemExit(
            "Top-level docs and generated files must stay outside Git; use site/guide/ for documentation and dist/enclosure/ for exports:\n"
            + "\n".join(forbidden)
        )
    print(
        "No top-level docs, generated STL, STEP/STP, 3MF, ZIP, CAD previews or disposable JSON are tracked."
    )


if __name__ == "__main__":
    main()
