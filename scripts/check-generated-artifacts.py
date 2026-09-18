"""Reject versioned CAD exports/archives, including case variants, anywhere in Git."""
from pathlib import Path
import re
import subprocess


def main():
    paths = subprocess.check_output(
        ['git', 'ls-files', '-z'], cwd=Path(__file__).resolve().parents[1]
    ).decode().split('\0')
    forbidden = [p for p in paths if re.search(r'\.(zip|stl|step|stp)$', p, re.I)]
    if forbidden:
        raise SystemExit('Generated files must be release assets:\n' + '\n'.join(forbidden))
    print('No generated STL, STEP/STP or ZIP files are tracked.')


if __name__ == '__main__':
    main()
