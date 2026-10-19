---
title: Release and publication
---

<a name="release-and-website-publishing"></a>

# Release and publication

These are maintainer instructions for a source checkout. The `site/` and
`.github/` files referenced below are not included in prebuilt app packages.

App/firmware releases and website deployment use separate GitHub Actions
workflows. STL and STEP files ship only inside the enclosure ZIP attached to
the release; keep all generated files ignored and out of Git (see
[CONTRIBUTING.md](../../../CONTRIBUTING.md)).

## Distribution

| Download | Contents |
| --- | --- |
| `broglow-vVERSION-aarch64-apple-darwin.tar.gz` | Apple Silicon app, C3 firmware and source, operating manual, licenses and checksums. |
| `broglow-vVERSION-x86_64-apple-darwin.tar.gz` | The same bundle, with a native Intel Mac app. |
| `broglow-vVERSION-x86_64-pc-windows-msvc.tar.gz` | Windows graphical tray app and CLI in `broglow.exe`, with USB lamp support, license and third-party notices; no installer, BLE, firmware or manuals. |
| `broglow-models-vVERSION.zip` | Enclosure print files: the five production parts 01–05 in `stl/` (print orientation) and `step/`, plus `BUILD.json`. The mascot attaches with four Ø3×2 mm N52 magnets (two in the mascot, two in the traffic light). |
| Matching `.sha256` | Checksum of each archive, with its original filename. |

Mac bundles also contain `BUILD.json`, an internal `SHA256SUMS`, flash offsets
and the minimum macOS version extracted from the executable. Install the app by
following the bundled [operating manual](hardware.md#install-a-prebuilt-package).
Every app archive carries `THIRD-PARTY-NOTICES.md` for the Rust crates in that
target's binary and the Rust standard library notices; Mac packages add the
firmware's linked libraries.
[`scripts/third-party-notices.py`](../../../scripts/third-party-notices.py)
generates them from `Cargo.lock`, the crate sources and the firmware linker map;
packaging runs it. Builds are not signed with a Developer ID certificate or notarized by Apple. Linux users build from
source. The Homebrew formula is published to the separate `broglow/tap`
repository by the [Homebrew tap workflow](#homebrew-tap); there is no crates.io
publishing workflow.

No Linux binaries are published; for the Linux tray's GTK 3 advisories see
[CONTRIBUTING](../../../CONTRIBUTING.md#development-setup).

## Python version

Release, firmware CI and Pages use **Python 3.14**. Root Python and firmware tools
come from `pyproject.toml` + `uv.lock`; firmware builds use **PlatformIO Core 6.2.0**
and esptool **5.4.0**.

The reviewed STL hashes were produced on **macOS 15 on Apple Silicon with
Python 3.14** (`macos-15` in release CI). Other platforms may produce different
STL bytes and fail the hash check; accept new hashes only after real slicing
checks on that platform.

## Build a draft release

The prepared version in `Cargo.toml` and `Cargo.lock` is **1.0.0**. The release
tag is **v1.0.0** and must point to the reviewed release commit.
Check both the source and website using the commands in
[CONTRIBUTING.md](../../../CONTRIBUTING.md#build-and-test) and
[site/README.md](../../README.md#check-changes).

After reviewing the commit and destination, tag and push explicitly. Replace
`YOUR_REMOTE` with the GitHub remote's name (`git remote -v`):

```bash
git tag v1.0.0
uv run --frozen python scripts/package-macos.py --release-tag v1.0.0 --check-only
git push YOUR_REMOTE HEAD:master
git push YOUR_REMOTE v1.0.0
```

If the check fails (dirty tree or version mismatch), delete the local tag with
`git tag -d v1.0.0`, fix the problem and repeat. Pushing `master` runs CI and,
when it changes the site, docs, models or `Cargo.toml`, deploys the website
automatically.

Push only this tag, by name. Every pushed `v*` tag other than the ignored `v0.*`
ones starts the **Release** workflow, and the inherited `v0.*` tags belong to the
pre-BroGlow history, so never use `git push --tags`. For the next release, bump `version` in `Cargo.toml` and
`Cargo.lock`, commit, then create and push only that version's tag:
`git push YOUR_REMOTE vX.Y.Z`.

The **Release** workflow runs in this order:

1. The `source` job on Apple Silicon `macos-15` runs the packaging tests and
   checks the tag, version and clean source. It installs the pinned Python CAD
   toolchain and generates the enclosure from the tag: the five production
   parts with their STL and STEP files. Exact STL hashes
   are verified against the reviewed slicer validation results, and the ZIP is
   produced with its `.sha256`. The job then checks that the
    web model export is current (`scripts/export-web-model.py --check`), and a
    final clean-source check prevents generated reports from changing tracked files.
2. The reusable CI suite and the **Website** checks run next, without deploying
   the site. CI covers Rust format/lint/tests, Python and firmware formatting,
   JavaScript integration, packaging and generated-artifact checks, firmware
   host checks and both C3 firmware builds.
3. After both pass, native Apple Silicon and Intel runners build and package the
   Mac app and C3 firmware, and a Windows runner builds the tray app and CLI.
4. The final job verifies every expected archive, its checksum and the
   third-party notices, then creates a **draft GitHub Release**. Only the
   archives and their `.sha256` files are attached.

Before selecting **Publish release** in GitHub:

- Download the draft's correct Mac archive and matching checksum. Follow the
  included manual from extraction through installation; confirm `--version`,
  login service, Bluetooth approval, pairing and a real agent task.
- Check `BUILD.json`, archive contents and checksums. App bundles and the enclosure
  ZIP are separate downloads; exclude reference media, recovery files and local
  user settings from every artifact.
- State which machines and lamp behavior were actually tested. Local acceptance
  currently covers Apple Silicon; a successful Intel build is not an Intel
  hardware test. Keep unverified platforms visible.
- Review the generated notes and choose the appropriate release/prerelease label.
  Publishing the draft is the action that makes binary downloads public; it also
  reruns the **Website** workflow so the guide shows the enclosure archive's SHA-256.

The operating manual's [acceptance checklist](hardware.md#physical-and-background-service-acceptance)
separates builds, service delivery, physical LEDs and enclosure validation.
Never reflash a working board just to test the release workflow.

## Package locally without publishing

Release CI packages the enclosure and attaches the ZIP and checksum to the draft
release. [`scripts/package-enclosure.py`](../../../scripts/package-enclosure.py)
builds the same package locally. `--output-dir PATH` selects another output
directory; keep it ignored. For a release rehearsal, use a clean checkout of the
matching tag and add `--release-tag vVERSION`: this strict mode validates tag,
version and clean source before generation.

<a name="package-magnetic-enclosure"></a>

### Magnetic enclosure

The [magnetic print guide](printing.md) covers the print inventory, magnet
bonding and source generation. Run:

```bash
uv run --no-project --python 3.14 --with-requirements models/broglow/requirements.txt \
  python scripts/package-enclosure.py
```

This produces `dist/enclosure/broglow-models-vVERSION.zip` and its
`.sha256`, using `dist/enclosure/magnetic-build/source/` for isolated generation.
Only production parts 01–05 are exported; no coupons or machine-specific
3MF/G-code are included. The stored slicer validation results must match all
five STL hashes. The archive holds `stl/`, `step/` and `BUILD.json`, with no CAD source or guide inside; the guide lives on
the website.

After a geometry change in `models/magnetic/refined.py`, the same command stops
with **Stale magnetic slicer evidence** by design: the ZIP is only built from STLs
that match the reviewed slicing results. The regenerated parts stay in
`dist/enclosure/magnetic-build/source/models/magnetic/` (`stl/`, `step/`,
`validation.json`); slice and test-print them before updating the evidence.

Keep the entire `models/broglow/preview/` directory ignored and untracked, including
renders, dimension diagrams and render recipes. Keep selected app/site assets and
reviewed validation data in Git.

Ad hoc CAD inspection reports and revision-specific snapshot job files are not
build inputs; keep them under ignored `dist/enclosure/`, outside Git. The build
regenerates `validation.json` in its isolated output tree.
Keep the reviewed [`slicer-validation.json`](../../../models/magnetic/slicer-validation.json):
packaging checks its STL hashes because generating geometry does not rerun the
external slicer.

A slicer hash mismatch is a failed build: investigate the pinned toolchain and
obtain real slicer validation for changed meshes before updating the reviewed
results. Geometry checks do not replace slicing, and neither proves physical
print fit.

<a name="package-mac-app"></a>

### Mac app package

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

<a name="homebrew-tap"></a>

## Homebrew tap

`brew install broglow/tap/broglow` installs the prebuilt Mac binaries of a
release from the `broglow/homebrew-tap` GitHub repository, which holds only
`Formula/broglow.rb`.

[`scripts/homebrew-formula.py`](../../../scripts/homebrew-formula.py) renders the
formula from the release's `broglow-vVERSION-*-apple-darwin.tar.gz.sha256` files
(the Apple Silicon and Intel archives under `on_macos`/`on_arm`/`on_intel`,
`bin/broglow`, firmware and manuals under `share/broglow`, and a caveat that
`broglow install` must run once and after upgrades). `tests/homebrew_formula.py`
checks the rendering in CI.

The **Homebrew tap** workflow (`.github/workflows/homebrew-tap.yaml`) runs only
by hand: **Actions → Homebrew tap → Run workflow** with the released tag. It
downloads the checksums from that release (a draft is enough), renders the
formula, stages it in a local tap on a macOS runner for `brew style` and
`brew audit --strict`, installs and tests it, and uploads `broglow.rb` as the
`homebrew-formula` artifact. With **publish** ticked it also pushes
`Formula/broglow.rb` to `broglow/homebrew-tap` using the `HOMEBREW_TAP_TOKEN`
repository secret (a fine-grained token with **Contents: read and write** on
`homebrew-tap` only). Publish the GitHub release first, then run the workflow
with **publish** ticked.

To rehearse locally, render the formula against archives in `dist/` with
`--url-base file:///path/to/dist/`, copy it into `brew tap-new broglow/tap`,
run `brew style`, `brew audit --strict` and `brew install broglow/tap/broglow`,
then `brew uninstall broglow` and `brew untap broglow/tap`. Do not run
`broglow install` from a rehearsal binary on a machine with a working install.

## Publish the website

Follow [the website runbook](../../README.md#publish-to-github-pages), then do
the post-deployment check below.

After deployment, open the live English and Russian guides and follow a release
download through to its checksum. Download links point to this repository's
Releases; check that the links work after publication. The guide includes a separate enclosure
download, and the landing opens with the product photo before the 3D view loads.

Local checks cannot prove GitHub permissions, hosted-runner success, live Pages
delivery or installation on another Mac. Verify those after each release
and deployment.
