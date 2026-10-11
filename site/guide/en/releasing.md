---
title: Release and publication
---

# Release and website publishing

These are maintainer instructions for a source checkout. The `site/` and
`.github/` files referenced below are not included in prebuilt app packages.

Release version: **1.0.0**. App/firmware releases and website deployment use separate
GitHub Actions workflows. Distribute generated STL, STEP/STP and ZIP files as
release assets; keep them ignored and out of Git.

## Distribution

| Download | Contents |
| --- | --- |
| `broglow-vVERSION-aarch64-apple-darwin.tar.gz` | Apple Silicon app, C3 firmware and source, operating manual, licenses and checksums. |
| `broglow-vVERSION-x86_64-apple-darwin.tar.gz` | The same bundle, with a native Intel Mac app. |
| `broglow-vVERSION-x86_64-pc-windows-msvc.tar.gz` | Windows graphical tray app and CLI in `broglow.exe`, with USB lamp support and license; no installer, BLE, firmware or manuals. |
| `broglow-enclosure-magnetic-vVERSION.zip` | Recommended detachable enclosure: five production STL/STEP parts, assembly views, source, guide and reports; four Ø3×2 mm N52 magnets. |
| `broglow-enclosure-vVERSION.zip` | Glued alternative: STL, STEP, CAD source, print guide and validation reports. |
| Matching `.sha256` | Checksum of each archive, with its original filename. |

Mac bundles also contain `BUILD.json`, an internal `SHA256SUMS`, flash offsets
and the minimum macOS version extracted from the executable. They install the
app locally with the [operating manual](hardware.md#install-a-prebuilt-package).
Every app archive carries `THIRD-PARTY-NOTICES.md` for the Rust crates in that
target's binary and the Rust standard library notices; Mac packages add the
firmware's linked libraries. `scripts/third-party-notices.py` generates them from
`Cargo.lock`, the crate sources and the firmware linker map; packaging runs it.
Builds are not Developer ID notarized. Linux users build from source; there is
no Homebrew or crates.io publishing workflow.

The Linux tray uses GTK 3 through `tao` and `tray-icon`. The GTK 3 Rust bindings
(0.18) are unmaintained and bring `glib` 0.18, which is affected by
[RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)
(unsound `VariantStrIter`, which BroGlow does not call; fixed only in `glib` 0.20,
which GTK 3 bindings cannot use), and the unmaintained build-time crate
`proc-macro-error` ([RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html)).
Mac and Windows builds do not include these crates. No Linux binaries are published.

## Python version

Release, firmware CI and Pages use **Python 3.14**. Root Python and firmware tools
come from `pyproject.toml` + `uv.lock`; firmware builds use **PlatformIO Core 6.2.0**
and esptool **5.4.0**.

Enclosure generation requires **macOS 15 on Apple Silicon with Python 3.14**
(`macos-15` in release CI). Other platforms can produce different STL bytes;
accept new hashes only after real slicing checks on that platform.

## Before the first public release

1. Choose the project's repository and verify `git remote -v` before pushing.
   Configure the intended publication destination explicitly.
2. Publish [our own media](media.md) and include the MIT license and dependency
   notices. Review retired assets in older Git commits before making history
   public; `.gitattributes` archive exclusions do not rewrite history. Use a
   reviewed source snapshot or a separately planned history cleanup as needed.
   Rebuild release packages from current source.
3. Commit the intended app, firmware, docs and workflow changes. Check
   `git status --short`; release packaging rejects a dirty checkout. Include the
   accepted enclosure source, instructions and reviewed slicer evidence in the
   release commit; generate print packages from that source, leaving them ignored.
4. Complete the checks below. A new machine still needs the normal macOS
   approval and a real local coding-agent task to validate hook delivery.

## Build a draft release

The prepared version in `Cargo.toml` and `Cargo.lock` is **1.0.0**. The release
tag is **v1.0.0** and must point to the reviewed release commit. For later
releases, update both version entries before tagging.
Check both the source and website using the commands in
[CONTRIBUTING.md](../../../CONTRIBUTING.md#build-and-test) and
[site/README.md](../../README.md#check-changes).

After reviewing the commit and destination, tag and push explicitly. Replace
`YOUR_REMOTE` with this project's chosen remote:

```bash
git tag v1.0.0
uv run --frozen python scripts/package-macos.py --release-tag v1.0.0 --check-only
git push YOUR_REMOTE HEAD
git push YOUR_REMOTE v1.0.0
```

Push only this tag. Every pushed `v*` tag starts the **Release** workflow, and the
inherited `v0.*` tags fail its version check, so do not use `git push --tags`.

The **Release** workflow checks the tag/version and clean source, runs the
reusable CI suite, builds on native Apple Silicon and Intel runners plus Windows,
uses the Apple Silicon `macos-15` source job to install the pinned Python CAD
toolchain and generate every printable STL and all four STEP views from the selected tag. It runs full CAD/mesh checks,
regenerates dimension images, verifies exact STL hashes against the reviewed
slicer evidence, builds and checks the print packages, and attaches
both `dist/enclosure/broglow-enclosure-magnetic-vVERSION.zip` (recommended) and
`dist/enclosure/broglow-enclosure-vVERSION.zip` (glued), with their `.sha256` files.
A final clean-source check prevents generated reports from changing tracked files.
The workflow verifies every expected app/enclosure archive and checksum and
creates a **draft GitHub Release**.
CI covers Rust format/lint/tests, JavaScript integration, firmware host checks,
firmware formatting and both C3 firmware builds. Before packaging, the release
also runs the **Website** checks without deploying the site.

Before selecting **Publish release** in GitHub:

- Download the draft's correct Mac archive and matching checksum. Follow the
  included manual from extraction through installation; confirm `--version`,
  login service, Bluetooth approval, pairing and a real agent task.
- Check `BUILD.json`, archive contents and checksums. App bundles and the enclosure
  ZIP are separate downloads; exclude reference media, recovery files and local
  user settings from every artifact.
- State which machines and lamp behavior were actually tested. Local acceptance
  currently covers Apple Silicon; a successful Intel build is not an Intel
  hardware test. Keep unverified platforms and first-release limitations visible.
- Review the generated notes and choose the appropriate release/prerelease label.
  Publishing the draft is the action that makes binary downloads public.

The operating manual's [acceptance checklist](hardware.md#physical-and-background-service-acceptance)
separates builds, service delivery, physical LEDs and enclosure validation.
Never reflash a working board just to test the release workflow.

## Package locally without publishing

The recommended magnetic package is generated by default. Its standalone guide
is in `models/magnetic/GUIDE_ROUND3X2.en.md`; it includes the print inventory,
magnet bonding and source generation instructions. Run:

```bash
uv run --no-project --python 3.14 --with-requirements models/broglow/requirements.txt \
  python scripts/package-enclosure.py --variant magnetic
```

This produces `dist/enclosure/broglow-enclosure-magnetic-vVERSION.zip` and its
`.sha256`, using `dist/enclosure/magnetic-build/source/` for isolated generation.
Only the current variant and parts 01–05 are checked/exported; no coupons or
machine-specific 3MF/G-code are included. The stored slicer evidence must match
all five STL hashes. The confirmed pocket diameter is 3.05 mm; the final 2.30 mm
depth and assembled holding force remain physically unverified.

Release CI runs both `--variant magnetic` and `--variant glued`, attaching both
ZIPs and checksums to the same draft release. The following procedure describes
the older **glued alternative**; pass `--variant glued` explicitly.


Use the copy-pastable [manual generation procedure](../../../models/broglow/PRINT_GUIDE.en.md#generate-from-source)
or the equivalent [agent prompt](../../../models/broglow/PRINT_GUIDE.en.md#agent-generation).
Both run `scripts/package-enclosure.py --variant glued` on macOS 15 Apple Silicon with Python 3.14
and the pinned `models/broglow/requirements.txt`; no board, PlatformIO or agent
skill is needed.
The command generates geometry, dimensions and full mesh validation in the
isolated `dist/enclosure/source/` tree, then reuses `package_prints.py` to verify
slicer evidence and package contents. It leaves reviewed source reports intact.

Keep the entire `models/broglow/preview/` directory ignored and untracked, including
renders, dimension diagrams and render recipes. Keep selected app/site assets and
reviewed validation data in Git. Generate release diagrams under `dist/enclosure/`.

Ad hoc CAD inspection reports and revision-specific snapshot job files are not
build inputs; keep them under ignored `dist/enclosure/`, outside Git. The build
regenerates `parts.json` and `validation.json` in its isolated output tree.
Keep the reviewed `slicer-validation.json`: packaging checks its STL hashes
because generating geometry does not rerun the external slicer.

The full `dist/enclosure/broglow-enclosure-v1.0.0.zip` and matching `.sha256`
are the enclosure release assets. The same directory contains
`broglow-print-pack.zip`, `broglow-compact-STL.zip` and
`broglow-USB-fit-test.zip` for local use or optional extra release downloads.
`--output-dir PATH` selects another output directory; keep it ignored.
For CI or a release rehearsal, use a clean matching tagged checkout:

```bash
uv run --no-project --python 3.14 --with-requirements models/broglow/requirements.txt \
  python scripts/package-enclosure.py --variant glued --release-tag v1.0.0
```

The strict release mode validates tag, version and clean source before generation.
A slicer hash mismatch is a failed build: investigate the pinned toolchain and
obtain real slicer validation for changed meshes before updating reviewed evidence.
Geometry checks do not replace slicing, and neither proves physical print fit.

On the native Mac, build the app and production C3 image first:

```bash
cargo build --release --locked
uv run --frozen flash --build-only
uv run --frozen python scripts/package-macos.py
```

This produces a timestamped archive and `.sha256` under ignored `dist/`.
It does not build, install, flash or publish anything itself. Local packages may
use a dirty checkout; `BUILD.json` records that and hashes the build inputs.
Use `--release-tag vVERSION` only for a clean, matching tagged commit; that mode
uses the stable release filename and refuses to overwrite an existing archive.

## Publish the website

Follow [the website runbook](../../README.md#publish-to-github-pages): enable
GitHub Pages with **GitHub Actions** as its source, then run **Website** on the
default branch. The workflow obtains the repository URL from GitHub, builds and
checks the landing and bilingual guide, then uploads only `site/_pages/`.

The landing is at the deployment URL; the guides are under `docs/` and `docs/ru/`.
Paths work at a domain root or a repository prefix. Pushes affecting the site or
docs on `master` deploy automatically; update that branch filter if necessary.
PRs build and check without deploying. Custom domains are configured in Pages
settings; environment approval rules may require a GitHub approval.

After deployment, open the live English and Russian guides and follow a release
download through to its checksum. The guide links to this repository's Releases
and offers a source-build fallback before the first complete release. It includes
a separate enclosure download and labels the product images as CAD renders.

Local checks cannot prove GitHub permissions, hosted-runner success, live Pages
delivery or installation on another Mac. Verify those during the first draft
release and deployment; the preparation itself publishes nothing.
