---
title: Release and publication
---

<a name="release-and-website-publishing"></a>

# Release and publication

These are maintainer instructions for a source checkout. The `site/` and
`.github/` files referenced below are not included in prebuilt app packages.

App/firmware releases and website deployment use separate GitHub Actions
workflows. STL and STEP files ship only inside the enclosure ZIPs attached to
the release; keep all generated files ignored and out of Git (see
[CONTRIBUTING.md](../../../CONTRIBUTING.md)).

## Distribution

| Download | Contents |
| --- | --- |
| `broglow-vVERSION-aarch64-apple-darwin.tar.gz` | Apple Silicon app, C3 firmware and source, operating manual, licenses and checksums. |
| `broglow-vVERSION-x86_64-apple-darwin.tar.gz` | The same bundle, with a native Intel Mac app. |
| `broglow-vVERSION-x86_64-pc-windows-msvc.tar.gz` | Windows graphical tray app and CLI in `broglow.exe`, with USB lamp support, license and third-party notices; no installer, BLE, firmware or manuals. |
| `broglow-enclosure-magnetic-vVERSION.zip` | Recommended detachable enclosure: five production STL/STEP parts, assembly views, source, guide and reports; four Ø3×2 mm N52 magnets. |
| `broglow-enclosure-vVERSION.zip` | Glued alternative: STL, STEP, CAD source, print guide and validation reports. |
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
source; there is no Homebrew or crates.io publishing workflow.

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

The reviewed STL hashes were produced on **macOS 15 on Apple Silicon with
Python 3.14** (`macos-15` in release CI). Other platforms may produce different
STL bytes and fail the hash check; accept new hashes only after real slicing
checks on that platform.

## Before the first public release

1. Do this before the first `git push`. Check that `package.repository` in
   `Cargo.toml` names the public repository (the docs link to it), add it as a
   remote and verify with `git remote -v`:

    ```bash
    git remote add YOUR_REMOTE {{ repo_url }}
    git remote -v
    ```

2. Use only [our own media](media.md) and include the MIT `LICENSE` and
   dependency notices.
3. Pushing a branch publishes its full history. Review retired assets in older
   commits before making history public; `.gitattributes` archive exclusions do
   not rewrite history. Use a reviewed source snapshot or a separately planned
   history cleanup as needed. Old `v0.*` tags stay local unless pushed by name.
4. Commit the intended app, firmware, docs and workflow changes. Check
   `git status --short`; release packaging rejects a dirty checkout. Include the
   accepted enclosure source, instructions and reviewed slicer validation results
   (`slicer-validation.json`) in the release commit; generate print packages from
   that source, leaving them ignored.
5. Complete the checks below. A new machine still needs the normal macOS
   approval and a real local agent task to validate hook delivery.

## Build a draft release

The prepared version in `Cargo.toml` and `Cargo.lock` is **1.0.0**. The release
tag is **v1.0.0** and must point to the reviewed release commit.
Check both the source and website using the commands in
[CONTRIBUTING.md](../../../CONTRIBUTING.md#build-and-test) and
[site/README.md](../../README.md#check-changes).

After reviewing the commit and destination, tag and push explicitly. Replace
`YOUR_REMOTE` with the remote added above:

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

Push only this tag, by name. Every pushed `v*` tag starts the **Release**
workflow, and the inherited `v0.*` tags fail its version check, so never use
`git push --tags`. For the next release, bump `version` in `Cargo.toml` and
`Cargo.lock`, commit, then create and push only that version's tag:
`git push YOUR_REMOTE vX.Y.Z`.

The **Release** workflow runs in this order:

1. The `source` job on Apple Silicon `macos-15` runs the packaging tests and
   checks the tag, version and clean source. It installs the pinned Python CAD
   toolchain and generates both enclosure variants from the tag:
    - **Magnetic**: the five production parts with their STL and STEP files and
      the assembly views.
    - **Glued**: every printable STL and the four STEP views, full CAD/mesh
      checks and dimension images.

    Both variants verify exact STL hashes against the reviewed slicer validation
    results and produce a ZIP with its `.sha256`. The job then checks that the
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
  hardware test. Keep unverified platforms and first-release limitations visible.
- Review the generated notes and choose the appropriate release/prerelease label.
  Publishing the draft is the action that makes binary downloads public.

The operating manual's [acceptance checklist](hardware.md#physical-and-background-service-acceptance)
separates builds, service delivery, physical LEDs and enclosure validation.
Never reflash a working board just to test the release workflow.

## Package locally without publishing

Release CI packages both enclosure variants and attaches both ZIPs and checksums
to the same draft release. Locally, each package is built separately.
[`scripts/package-enclosure.py`](../../../scripts/package-enclosure.py) builds the
magnetic variant by default. `--output-dir PATH` selects another output
directory; keep it ignored. For a release rehearsal, use a clean checkout of the
matching tag and add `--release-tag vVERSION`: this strict mode validates tag,
version and clean source before generation.

<a name="package-magnetic-enclosure"></a>

### Magnetic enclosure

The [magnetic print guide](printing.md) covers the print inventory, magnet
bonding and source generation. Run:

```bash
uv run --no-project --python 3.14 --with-requirements models/broglow/requirements.txt \
  python scripts/package-enclosure.py --variant magnetic
```

This produces `dist/enclosure/broglow-enclosure-magnetic-vVERSION.zip` and its
`.sha256`, using `dist/enclosure/magnetic-build/source/` for isolated generation.
Only production parts 01–05 are exported; no coupons or machine-specific
3MF/G-code are included. The stored slicer validation results must match all
five STL hashes.

<a name="package-glued-enclosure"></a>

### Glued enclosure

Pass `--variant glued` explicitly. For a glued-only local build (no `--release-tag`,
not a release rehearsal), use the
[manual generation procedure](../../../models/broglow/PRINT_GUIDE.en.md#generate-from-source)
or the equivalent [agent prompt](../../../models/broglow/PRINT_GUIDE.en.md#agent-generation).
Both run the packaging script on macOS 15 Apple Silicon with Python 3.14 and the
pinned [`models/broglow/requirements.txt`](../../../models/broglow/requirements.txt);
no board, PlatformIO or agent skill is needed. For a release rehearsal:

```bash
uv run --no-project --python 3.14 --with-requirements models/broglow/requirements.txt \
  python scripts/package-enclosure.py --variant glued --release-tag vVERSION
```

The command generates geometry, dimensions and full mesh validation in the
isolated `dist/enclosure/source/` tree, then reuses
[`package_prints.py`](../../../models/broglow/package_prints.py) to verify the
slicer validation results and package contents. It leaves reviewed source
reports intact.

`dist/enclosure/broglow-enclosure-vVERSION.zip` (a copy of
`broglow-print-pack.zip`) and its `.sha256` are the glued release assets.
`broglow-compact-STL.zip` and `broglow-USB-fit-test.zip` in the same directory
are for local use; the **Release** workflow does not upload them.

Keep the entire `models/broglow/preview/` directory ignored and untracked, including
renders, dimension diagrams and render recipes. Keep selected app/site assets and
reviewed validation data in Git. Generate release diagrams under `dist/enclosure/`.

Ad hoc CAD inspection reports and revision-specific snapshot job files are not
build inputs; keep them under ignored `dist/enclosure/`, outside Git. The build
regenerates `parts.json` and `validation.json` in its isolated output tree.
Keep the reviewed [`slicer-validation.json`](../../../models/broglow/slicer-validation.json):
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

## Publish the website

Follow [the website runbook](../../README.md#publish-to-github-pages), then do
the post-deployment check below.

After deployment, open the live English and Russian guides and follow a release
download through to its checksum. Download links point to this repository's
Releases; until the release is published, the guide offers a source build
instead; check that the links work after publication. The guide includes a separate enclosure
download, and the landing labels the product image as a render.

Local checks cannot prove GitHub permissions, hosted-runner success, live Pages
delivery or installation on another Mac. Verify those during the first draft
release and deployment; the preparation itself publishes nothing.
