[English](RELEASING.md) | [Русский](RELEASING.ru.md)

<a name="release-and-website-publishing"></a>

# Release and publication

These are maintainer instructions for a source checkout. The `site/` and
`.github/` files referenced below are not included in prebuilt app packages.

App/firmware releases and website deployment use separate GitHub Actions
workflows. GitHub releases distribute STL and STEP files inside the separate
enclosure ZIP. The enclosure is also available on
[Printables](https://www.printables.com/model/1884645) and
[MakerWorld](https://makerworld.com/models/3468773); those listings use CC BY 4.0,
while the repository retains its MIT license. Keep all generated files ignored and out of Git (see
[CONTRIBUTING.md](CONTRIBUTING.md)).

## Distribution

| Download | Contents |
| --- | --- |
| `broglow-vVERSION-aarch64-apple-darwin.tar.gz` | Apple Silicon app, C3 firmware and source, operating manual, licenses and checksums. |
| `broglow-vVERSION-x86_64-apple-darwin.tar.gz` | The same bundle, with a native Intel Mac app. |
| `broglow-vVERSION-x86_64-pc-windows-msvc.tar.gz` | Windows graphical tray app and CLI in `broglow.exe`, with USB lamp support, the C3 firmware images with source and licenses, license and third-party notices; no installer, BLE or manuals. |
| `broglow-models-vVERSION.zip` | Enclosure print files: the five production parts 01–05 in `stl/` (print orientation) and `step/`, plus `BUILD.json`. |
| Matching `.sha256` | Checksum of each archive, with its original filename. |

Mac bundles also contain `BUILD.json`, an internal `SHA256SUMS`, flash offsets
and the minimum macOS version extracted from the executable. Install the app by
following the bundled [operating manual](site/guide/en/hardware.md#install-a-prebuilt-package).
Every app archive carries `THIRD-PARTY-NOTICES.md` for the Rust crates in that
target's binary and the Rust standard library notices; Mac packages add the
firmware's linked libraries.
[`scripts/third-party-notices.py`](scripts/third-party-notices.py)
generates them from `Cargo.lock`, the crate sources and the firmware linker map;
packaging runs it. Builds are not signed with a Developer ID certificate or notarized by Apple. Linux users build from
source. The Homebrew formula is published to the separate `broglow/tap`
repository by the [Homebrew tap workflow](#homebrew-tap); there is no crates.io
publishing workflow.

For the Linux tray's GTK 3 advisories, see
[CONTRIBUTING](CONTRIBUTING.md#development-setup).

## Python version

Release, firmware CI and Pages use **Python 3.14**. Root Python and firmware tools
come from `pyproject.toml` + `uv.lock`; firmware builds use **PlatformIO Core 6.2.0**
and esptool **5.4.0**.

The reviewed STL hashes were produced on **macOS 15 on Apple Silicon with
Python 3.14** (`macos-15` in release CI). Other platforms may produce different
STL bytes and fail the hash check; accept new hashes only after real slicing
checks on that platform.

## Firmware version

The C3 firmware carries its own version in `firmware/esp32-c3-supermini/VERSION`,
independent of the app's, so an app release never asks lamps to reflash unchanged
firmware. The version is the date of the firmware's last change, `YYYY.MM.DD`, with
`.1`, `.2`, … for further changes on the same day. `version-lock.json` next to it
records the sources that version was built from, and the firmware host test fails
when `src/main.cpp` or `platformio.ini` change without a new version. After changing
the firmware, run:

```bash
uv run --no-project --python 3.14 python scripts/firmware-version.py bump
```

## Build a draft release

Review and commit all release changes first, then check both the source and
website using the commands in
[CONTRIBUTING.md](CONTRIBUTING.md#build-and-test) and
[site/README.md](site/README.md#check-changes).

From that clean checkout, use the release helper to create **vVERSION**. It
updates `Cargo.toml` and the root `broglow` entry in `Cargo.lock`, validates
locked Cargo metadata offline, commits the version files if they changed and
creates the matching annotated tag. It does not push. If the version already
matches, it creates the tag without an empty commit.

After reviewing the resulting commit and destination, check the tagged source
and push explicitly. Replace `YOUR_REMOTE` with the GitHub remote's name
(`git remote -v`):

```bash
uv run --no-project --python 3.14 python scripts/release-tag.py vVERSION
uv run --frozen python scripts/package-macos.py --release-tag vVERSION --check-only
git push YOUR_REMOTE HEAD:master
git push YOUR_REMOTE vVERSION
```

Plain `git tag` does not run the helper or update Cargo versions; release CI
rejects a tag/version mismatch. If the tagged-source check fails before pushing,
delete only the unpublished local tag with `git tag -d vVERSION`, fix and commit
the problem, then repeat the helper and check. Pushing `master` runs CI and, when
it changes the site, docs, models or `Cargo.toml`, deploys the website automatically.

Push only this tag, by name; never use `git push --tags`. Pushing a `v*` tag
starts the **Release** workflow. For the next release, repeat this procedure with
the next version.

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
   Mac app and C3 firmware, and a Windows runner builds the tray app and CLI and
   adds the firmware directory the source job packaged.
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
- Check on real hardware: reconnect, all LED colors and brightness, stale-status
  blanking within ten seconds and enclosure fit; never reflash a working board
  just to test the release workflow. State which machines and lamp behavior were
  actually tested. Local acceptance currently covers Apple Silicon; a successful
  Intel build is not an Intel hardware test. Keep unverified platforms visible.
- Review the generated notes and choose the appropriate release/prerelease label.
  Publishing the draft makes the downloads public and reruns the **Website**
  workflow so the guide shows the enclosure archive's SHA-256.

Never delete a published release or move its tag: the tap's formulae point at its
downloads and checksums. Ship the next patch version instead.

## Package locally without publishing

[`scripts/package-enclosure.py`](scripts/package-enclosure.py) builds
the release enclosure package. `--output-dir PATH` selects another output
directory; keep it ignored. For a release rehearsal, use a clean checkout of the
matching tag and add `--release-tag vVERSION`: this strict mode validates tag,
version and clean source before generation.

<a name="package-magnetic-enclosure"></a>

### Magnetic enclosure

The [magnetic print guide](site/guide/en/printing.md) covers the print inventory, magnet
bonding and source generation. Run:

```bash
uv run --frozen --only-group cad python scripts/package-enclosure.py
```

This produces `dist/enclosure/broglow-models-vVERSION.zip` and its
`.sha256`, using `dist/enclosure/magnetic-build/source/` for isolated generation.
Only production parts 01–05 are exported; no coupons or machine-specific
3MF/G-code are included. The archive has no CAD source or guide; the guide lives
on the website.

After a geometry change in `models/magnetic/refined.py`, the same command stops
with **Stale magnetic slicer evidence** by design: the ZIP is only built from STLs
that match the reviewed slicing results. The regenerated parts stay in
`dist/enclosure/magnetic-build/source/models/magnetic/` (`stl/`, `step/`,
`validation.json`); slice and test-print them before updating the evidence.

Ad hoc CAD inspection reports and revision-specific snapshot job files are not
build inputs; keep them under ignored `dist/enclosure/`, outside Git. The build
regenerates `validation.json` in its isolated output tree.
Keep the reviewed [`slicer-validation.json`](models/magnetic/slicer-validation.json):
packaging checks its STL hashes because generating geometry does not rerun the
external slicer.

On a hash mismatch, investigate the pinned toolchain first. Geometry checks do
not replace slicing, and neither proves print fit.

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
release from the `broglow/homebrew-tap` GitHub repository. `Formula/broglow.rb`
follows the latest release; `Formula/broglow@VERSION.rb` keeps each release
installable as a keg-only alternate (`brew install broglow/tap/broglow@1.0.0`).

[`scripts/homebrew-formula.py`](scripts/homebrew-formula.py) renders both
formulae from the release's `broglow-vVERSION-*-apple-darwin.tar.gz.sha256` files
(the Apple Silicon and Intel archives under `on_macos`/`on_arm`/`on_intel`,
`bin/broglow`, firmware and manuals under `share/broglow`, and a caveat that
`broglow install` must run once and after upgrades). `tests/test_homebrew_formula.py`
checks the rendering in CI.

The **Homebrew tap** workflow (`.github/workflows/homebrew-tap.yaml`) runs when
a release is published, or by hand: **Actions → Homebrew tap → Run workflow**
with a released tag. It downloads the checksums from that release, renders both
formulae, stages them in a local tap on a macOS runner for `brew style` and
`brew audit --strict`, installs the latest formula before the pinned one to keep
the default command linked, tests both, and uploads them as the
`homebrew-formula` artifact. For a published release, or by hand with **publish**
ticked, it then pushes them to `broglow/homebrew-tap`'s `Formula/` using the
`HOMEBREW_TAP_TOKEN` repository secret (a fine-grained token with
**Contents: read and write** on `homebrew-tap` only). Renew the token before it
expires.

<a name="homebrew-installation-order"></a>

### Required installation order

Rehearse formula/workflow changes on a clean Mac with no BroGlow formulae installed.
Use [the workflow](.github/workflows/homebrew-tap.yaml) for staging and style/audit
checks. Install latest first, then pinned, in separate commands:

```bash
LATEST=broglow/tap/broglow
PINNED=broglow/tap/broglow@VERSION
brew install "$LATEST"
brew install "$PINNED"
brew test "$LATEST" "$PINNED"
"$(brew --prefix)/bin/broglow" --version
"$(brew --prefix "$PINNED")/bin/broglow" --version
```

Both binaries must report VERSION.

Combining the installs caused v1.0.2's fatal **latest formula** `is not linked`
error. The pinned formula's **installed but not linked** warning is expected:
latest owns the command. Keep both tests; do not follow the warning's `brew link`
suggestion, relink to hide failures, or suppress diagnostics with `--quiet`.

For local archives, render with `--url-base file:///path/to/dist/` and use an
unused temporary tap; different taps still share Cellar names. Remove only test
packages/taps. Never run rehearsal `broglow install` over a working app.

<a name="homebrew-publication-acceptance"></a>

### Publication acceptance and retry

1. Require successful **Release**, **Homebrew tap** (including `publish`) and
   **Website** runs; a successful build alone does not prove publication.
2. In a fresh environment without the rehearsal tap, run `brew tap broglow/tap`.
   Verify public formula versions, archive URLs/checksums and both installs above.
   Outside the checkout, check `command -v broglow` resolves to Homebrew;
   Cargo can shadow it on PATH.
3. Save evidence under ignored `.recovery/` and report untested platforms/hardware.

After a workflow fix, start a **new** Homebrew tap run from **master** with the
existing released **tag** and **publish** ticked. **Re-run jobs** keeps the old
workflow. The generator still comes from the tag; generator/app fixes need a new
patch release, not replaced assets.

## Publish the website

Follow [the website runbook](site/README.md#publish-to-github-pages), then do
the post-deployment check below.

After deployment, open the live English and Russian guides and follow a release
download from this repository's Releases through to its checksum. The guide
includes a separate enclosure download. Check the Ember homepage in both
languages and themes: the shared app demo and tracking settings, copyable install
commands, guide/print links and scroll-driven 3D lamp. Confirm the real-photo
fallback when 3D is unavailable. Physical phone AR needs a separate check.

Local checks cannot prove GitHub permissions, hosted-runner success, live Pages
delivery or installation on another Mac. Verify those after each release
and deployment.
