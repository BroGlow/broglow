#!/usr/bin/env python3
"""The C3 firmware's own version, locked to its sources.

`VERSION` holds the version the firmware reports: the date of its last change
as YYYY.MM.DD, plus .1, .2, ... for further changes on the same day, so it
reads as a date, never as an app version, and sorts in order (up to nine bumps
a day). `version-lock.json` records the sources that version was built from.
The firmware host test fails when the sources change without a new version, so
the app never offers a pointless update and never misses a real one.

    firmware-version.py            print the version
    firmware-version.py check      exit 1 when the lock is stale
    firmware-version.py bump       set today's date as the version and lock it
    firmware-version.py record     write the lock for the current VERSION
"""

import datetime
import hashlib
import json
from pathlib import Path
import sys

C3 = Path(__file__).resolve().parents[1] / "firmware/esp32-c3-supermini"
VERSION = C3 / "VERSION"
LOCK = C3 / "version-lock.json"
# What changes the shipped image: the sketch and the build configuration.
SOURCES = ("src/main.cpp", "platformio.ini")


def version():
    return VERSION.read_text().strip()


def sources_sha256():
    digest = hashlib.sha256()
    for name in SOURCES:
        digest.update(name.encode())
        digest.update(b"\0")
        digest.update((C3 / name).read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def read_lock():
    return json.loads(LOCK.read_text())


def stale():
    """None when the lock matches; otherwise the reason."""
    lock = read_lock()
    if lock["version"] != version():
        return (
            f"VERSION is {version()} but version-lock.json records {lock['version']}; "
            "run scripts/firmware-version.py record"
        )
    if lock["sources_sha256"] != sources_sha256():
        return (
            f"Firmware sources changed since version {lock['version']} was recorded; "
            "set a new version in firmware/esp32-c3-supermini/VERSION, then run "
            "scripts/firmware-version.py record"
        )
    return None


def record(force=False):
    current = version()
    if LOCK.is_file() and not force:
        lock = read_lock()
        if lock["version"] == current and lock["sources_sha256"] != sources_sha256():
            raise SystemExit(
                f"Version {current} is already recorded for different sources; "
                "bump VERSION first (or pass --force to re-record it)."
            )
    LOCK.write_text(
        json.dumps({"version": current, "sources_sha256": sources_sha256()}, indent=2)
        + "\n"
    )
    print(f"Recorded firmware {current} for {', '.join(SOURCES)}")


def next_version(current, today):
    """Today's date, or the next same-day suffix when today is already used."""
    base = today.strftime("%Y.%m.%d")
    if current == base:
        return f"{base}.1"
    if current.startswith(base + "."):
        return f"{base}.{int(current[len(base) + 1 :]) + 1}"
    return base


def bump(today=None):
    new = next_version(version(), today or datetime.date.today())
    VERSION.write_text(new + "\n")
    record()


def main(argv):
    command = argv[1] if len(argv) > 1 else "print"
    if command == "print":
        print(version())
    elif command == "bump":
        bump()
    elif command == "check":
        reason = stale()
        if reason:
            raise SystemExit(reason)
        print(f"Firmware {version()} matches its sources")
    elif command == "record":
        record(force="--force" in argv[2:])
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
