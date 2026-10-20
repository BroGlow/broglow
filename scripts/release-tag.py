"""Commit matching Cargo versions and create a local annotated release tag."""

import argparse
from pathlib import Path
import re
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
STABLE_VERSION = r"([1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"


def git(*args: str) -> str:
    result = subprocess.run(["git", *args], cwd=ROOT, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip())
    return result.stdout.strip()


def version_numbers(version: str) -> tuple[int, ...]:
    if not re.fullmatch(STABLE_VERSION, version):
        raise RuntimeError(f"Expected a stable version starting at 1.0.0: {version}")
    return tuple(map(int, version.split(".")))


def replace_version(text: str, section: re.Match[str], version: str) -> str:
    updated, count = re.subn(
        r"(?m)^([ \t]*version[ \t]*=[ \t]*)([\"'])([^\"'\r\n]+)([\"'])",
        lambda match: f"{match[1]}{match[2]}{version}{match[4]}",
        section[0],
    )
    if count != 1:
        raise RuntimeError("Expected one literal version in each Cargo package table.")
    return text[: section.start()] + updated + text[section.end() :]


def cargo_versions(manifest: str, lock: str) -> str:
    package = tomllib.loads(manifest)["package"]
    locked = [
        entry
        for entry in tomllib.loads(lock)["package"]
        if entry["name"] == "broglow" and "source" not in entry
    ]
    if package["name"] != "broglow" or len(locked) != 1:
        raise RuntimeError("Expected the root broglow package in both Cargo files.")
    version = package["version"]
    version_numbers(version)
    if locked[0]["version"] != version:
        raise RuntimeError("Cargo.toml and Cargo.lock versions must already match.")
    return version


def release(tag: str) -> None:
    if not re.fullmatch(f"v{STABLE_VERSION}", tag):
        raise RuntimeError("Release tag must be stable vX.Y.Z, starting at v1.0.0.")
    if Path(git("rev-parse", "--show-toplevel")).resolve() != ROOT:
        raise RuntimeError("Run this helper from its source Git checkout.")
    if git("status", "--porcelain", "--untracked-files=all"):
        raise RuntimeError("Commit or remove all source changes before creating a tag.")
    if git("tag", "--list", tag):
        raise RuntimeError(f"Tag {tag} already exists; existing tags are never moved.")

    paths = (ROOT / "Cargo.toml", ROOT / "Cargo.lock")
    originals = tuple(path.read_bytes() for path in paths)
    manifest, lock = (data.decode("utf-8") for data in originals)
    current = cargo_versions(manifest, lock)
    version = tag[1:]
    if version_numbers(version) < version_numbers(current):
        raise RuntimeError(
            f"Refusing to lower Cargo version from {current} to {version}."
        )

    changed = version != current
    if changed:
        manifest_section = re.search(
            r"(?ms)^\[package\][^\r\n]*\r?\n.*?(?=^\[|\Z)", manifest
        )
        lock_sections = [
            section
            for section in re.finditer(
                r"(?ms)^\[\[package\]\][^\r\n]*\r?\n.*?(?=^\[\[package\]\]|\Z)",
                lock,
            )
            if any(
                entry["name"] == "broglow" and "source" not in entry
                for entry in tomllib.loads(section[0])["package"]
            )
        ]
        if manifest_section is None or len(lock_sections) != 1:
            raise RuntimeError(
                "Could not locate the root Cargo package version tables."
            )
        manifest = replace_version(manifest, manifest_section, version)
        lock = replace_version(lock, lock_sections[0], version)
        cargo_versions(manifest, lock)
        for path, text in zip(paths, (manifest, lock)):
            path.write_bytes(text.encode("utf-8"))

    try:
        subprocess.run(
            [
                "cargo",
                "metadata",
                "--offline",
                "--locked",
                "--no-deps",
                "--format-version",
                "1",
            ],
            cwd=ROOT,
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        if changed:
            for path, original in zip(paths, originals):
                path.write_bytes(original)
        raise RuntimeError(
            "Offline Cargo validation failed; original versions restored. "
            "No commit or tag was created."
        ) from error

    commit = None
    try:
        if changed:
            git(
                "commit",
                "--only",
                "-m",
                f"Release {tag}",
                "--",
                "Cargo.toml",
                "Cargo.lock",
            )
            commit = git("rev-parse", "HEAD")
        if git("status", "--porcelain", "--untracked-files=all"):
            raise RuntimeError(
                "Checkout changed during validation or commit; inspect it."
            )
        git("tag", "-a", tag, "-m", f"BroGlow {tag}")
    except (RuntimeError, OSError) as error:
        state = (
            f"Version commit {commit} remains; no tag was created."
            if commit
            else "No tag was created; inspect the checkout before retrying."
        )
        raise RuntimeError(f"{error}\n{state}") from error
    print(f"Created local annotated tag {tag} at {git('rev-parse', 'HEAD')}.")
    print("Nothing was pushed.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag", help="Stable release tag, for example v1.0.2")
    args = parser.parse_args()
    try:
        release(args.tag)
    except (RuntimeError, OSError, KeyError, tomllib.TOMLDecodeError) as error:
        print(f"Release tag failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
