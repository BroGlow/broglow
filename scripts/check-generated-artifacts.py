"""Reject CAD exports, archives, disposable previews, inspection JSON and retired media."""

from pathlib import Path
import re
import subprocess

RETIRED_MEDIA = {
    "site/assets/assembly.png",
    "site/vendor/model-viewer-4.3.1.min.js",
    "references/clawlight/claw-light-ar.usdz",
    "references/clawlight/claw-light-ar.glb",
    "references/clawlight/store-photo.jpg",
    "references/clawlight/product-photo.jpg",
    "references/clawlight/ar-reference-views.png",
    "docs/screenshots/opencode-tui.png",
    "docs/screenshots/opencode-popover.png",
}


def is_generated(path):
    return bool(
        path.lower() in RETIRED_MEDIA
        or re.search(r"\.(zip|stl|step|stp)$", path, re.I)
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
            "Generated files and retired media must stay outside Git; use dist/enclosure/ for exports:\n"
            + "\n".join(forbidden)
        )
    print(
        "No generated STL, STEP/STP, ZIP, CAD previews, disposable JSON or retired media are tracked."
    )


if __name__ == "__main__":
    main()
