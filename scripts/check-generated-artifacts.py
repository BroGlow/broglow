"""Reject CAD exports, archives and obsolete enclosure inspection/render JSON."""
from pathlib import Path
import re
import subprocess


def is_generated(path):
    return bool(re.search(r'\.(zip|stl|step|stp)$', path, re.I) or re.fullmatch(
        r'models/clawlight/(?:(?:cad|coupon|usb-coupon)-inspection\.json'
        r'|preview/[^/]*snapshot-job\.json)', path, re.I))


def main():
    paths = subprocess.check_output(
        ['git', 'ls-files', '-z'], cwd=Path(__file__).resolve().parents[1]
    ).decode().split('\0')
    forbidden = [p for p in paths if is_generated(p)]
    if forbidden:
        raise SystemExit('Generated files must stay outside Git; use dist/enclosure/:\n'
                         + '\n'.join(forbidden))
    print('No generated STL, STEP/STP, ZIP or disposable CAD JSON files are tracked.')


if __name__ == '__main__':
    main()
