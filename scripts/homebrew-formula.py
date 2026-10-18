#!/usr/bin/env python3
"""Render the Homebrew formula for a release from its Mac archive checksums.

The formula installs the prebuilt Mac binaries of one release; the Homebrew tap
workflow publishes it to the separate tap repository as Formula/broglow.rb.
"""

import argparse
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {"arm": "aarch64-apple-darwin", "intel": "x86_64-apple-darwin"}
FORMULA = """\
class Broglow < Formula
  desc "Desk traffic light and menu bar monitor for local coding-agent sessions"
  homepage "{homepage}"
  license "MIT"

  depends_on :macos

  on_macos do
    on_arm do
      url "{url_base}broglow-v{version}-aarch64-apple-darwin.tar.gz"
      sha256 "{sha_arm}"
    end
    on_intel do
      url "{url_base}broglow-v{version}-x86_64-apple-darwin.tar.gz"
      sha256 "{sha_intel}"
    end
  end

  def install
    bin.install "bin/broglow"
    pkgshare.install "firmware", "site"
    doc.install Dir["*.md"], "RUST-STANDARD-LIBRARY-COPYRIGHT.html"
  end

  def caveats
    <<~EOS
      Finish setup once (registers the coding-agent hooks, installs
      ~/Applications/BroGlow.app and starts it at login); repeat after upgrades:
        broglow install
      Firmware and manuals: #{{pkgshare}}
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{{bin}}/broglow --version")
  end
end
"""


def checksum(path, archive):
    """The sha256sum line written next to each release archive."""
    match = re.fullmatch(
        rf"([0-9a-f]{{64}})  {re.escape(archive)}\n?", path.read_text()
    )
    if not match:
        raise SystemExit(f"Not a sha256sum line for {archive}: {path}")
    return match[1]


def render(version, checksums, homepage, url_base):
    digests = {}
    for key, target in TARGETS.items():
        archive = f"broglow-v{version}-{target}.tar.gz"
        digests[key] = checksum(checksums / f"{archive}.sha256", archive)
    return FORMULA.format(
        homepage=homepage,
        version=version,
        url_base=url_base,
        sha_arm=digests["arm"],
        sha_intel=digests["intel"],
    )


def main():
    package = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]
    repository = package["repository"].rstrip("/")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", default=package["version"])
    parser.add_argument(
        "--checksums",
        type=Path,
        default=ROOT / "dist",
        help="Directory holding broglow-vVERSION-*-apple-darwin.tar.gz.sha256",
    )
    parser.add_argument(
        "--url-base",
        help="Archive location; defaults to the release's download URL.",
    )
    parser.add_argument(
        "--output", type=Path, default=ROOT / "dist/homebrew/broglow.rb"
    )
    args = parser.parse_args()
    url_base = args.url_base or f"{repository}/releases/download/v{args.version}/"
    text = render(args.version, args.checksums, repository, url_base)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(text)
    print(f"Formula: {args.output}")


if __name__ == "__main__":
    main()
