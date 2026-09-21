# BroGlow DIY website

[English](README.md) | [Русский](README.ru.md)

The landing is plain HTML/CSS. The bilingual guide uses [MkDocs](https://www.mkdocs.org/),
[Material](https://squidfunk.github.io/mkdocs-material/) and
[static-i18n](https://ultrabug.github.io/mkdocs-static-i18n/).

## Edit the guide

Edit Markdown sources; never edit or commit generated HTML.

| Topic | English | Russian |
| --- | --- | --- |
| Parts, models and wiring | `guide/en/index.md` | `guide/ru/index.md` |
| Installation, agent prompt and Bluetooth | `guide/en/installation.md` | `guide/ru/installation.md` |
| Codex integration | `guide/en/codex.md` | `guide/ru/codex.md` |
| Troubleshooting and agent prompt | `guide/en/troubleshooting.md` | `guide/ru/troubleshooting.md` |
| Hardware reference | `guide/en/hardware.md` | `guide/ru/hardware.md` |
| Firmware, releases, images, enclosure design | `guide/en/NAME.md` | `guide/ru/NAME.md` |
| Printing and assembly | `../models/broglow/PRINT_GUIDE.en.md` | `../models/broglow/PRINT_GUIDE.md` |

To add a page, create matching filenames in both language folders and update
`nav` and `nav_translations` in `mkdocs.yaml`. Use fenced `text` blocks for
copyable agent prompts.

## Keep the instructions in sync

The complete public manuals live directly under `guide/en/` and `guide/ru/`.
Quick guides include named sections from the local hardware pages, for example:

```text
--8<-- "site/guide/ru/hardware.md:lamp-wiring"
```

Keep the `start:` / `end:` markers for `lamp-wiring`, `path-check` and `path-setup`
in both hardware pages. Keep commands, GPIOs, resistor values, protocol fields
and explicit heading IDs equivalent across translations.

Every authored project document needs an EN/RU counterpart; dependency licenses
remain verbatim. For behavior changes, update affected summaries and agent prompts
in both `guide/en/` and `guide/ru/`. Run the checks below and the Pages build before
committing. Translation checks reject missing counterparts, anchors and localized
includes; missing snippet sections fail the build.

## Build and preview

Run from the **broglow repository root**. CI uses Python 3.14.

```bash
python3 -m venv site/.venv
site/.venv/bin/python -m pip install -r site/requirements-docs.txt
site/.venv/bin/mkdocs build --strict -f site/mkdocs.yaml
python3 -m http.server 8080 --bind 127.0.0.1
```

Open http://127.0.0.1:8080/site/ for the landing and
http://127.0.0.1:8080/site/docs/ for the guide. Rebuild after Markdown edits;
output goes to ignored `site/docs/` without cleaning the landing.
Website builds need no generated STL, STEP/STP or ZIP files and do not run CAD.
Keep those exports ignored; printable downloads are release assets.

For automatic guide refresh, use
`site/.venv/bin/mkdocs serve -f site/mkdocs.yaml`. It serves only the guide;
use the repository-root server above to check the landing and local downloads.

## Interactive 3D and AR

The build section has a rotatable assembly explorer in place of the exploded
still image. It reuses the self-hosted model-viewer with `assets/assembly.glb`,
loaded lazily, and supports drag, keyboard rotation, zoom, Reset view and retry
after loading errors. Its separated parts expose the LED retainer and mounting
points. Signal effects and AR remain in the hero. Without JavaScript, the page
directs visitors to enable it.

The 3D preview starts automatically with Working selected. Self-hosted
`@google/model-viewer` 4.3.1 and the effects module share Three.js via a local import
map (see `vendor/README.md`). Working, Idle and Needs you light only the green,
yellow or red lens; Off dims all three and removes the glow. A restrained bloom
keeps the lens color visible. One Three.js point light follows the active LED and
casts its color onto nearby surfaces with distance falloff; Off disables it.
The light attaches through the effects composer's public mesh selection, so it
follows the model during rotation. The darker tower, warm stage and closer camera
make the light easier to read.

The selected signal applies before the first rendered frame and after retrying;
Reset view preserves it. Automatic loading does not take keyboard focus. The
matching Working-state poster remains visible while loading and without JavaScript.
After a loading failure, **Retry 3D** is available; the still image cannot preview
other signals. Camera controls and Reset view use the component's native behavior.
The material mapping is checked against the exported GLB in `model-preview.test.cjs`.
External Android Scene Viewer reloads the original GLB, so these live signal changes
apply to the browser preview, not that external AR view. Bloom postprocessing is
not available in AR modes, including WebXR.

**View in your space** opens a keyboard-accessible dialog with a locally generated
QR code and an **Open on this device** link. The QR uses the current site path with
`#view-in-space`, without query parameters; scanning opens the 3D preview, then the
visitor taps **View on your desk** to start AR. The phone uses its own language
preference. Localhost/file previews show guidance instead of an unreachable QR;
use a phone-accessible HTTPS address to test scanning and AR. QR generation failure
keeps the on-device link usable. The pinned QR encoder and license live in `vendor/`.

AR uses `webxr`, `scene-viewer` and `quick-look` with `ar-scale="fixed"`; the GLB
uses meters. Safari on iPhone/iPad generates USDZ on demand (`ios-src` is omitted);
other iOS browsers are unverified. WebXR requires HTTPS or loopback, and a phone
cannot reach another device's localhost. Physical phone placement and size still
need acceptance checks. See the [component's AR reference](https://modelviewer.dev/examples/augmentedreality/).

The model depicts our 1.0.0 enclosure with illustrative
electronics. Use the separate enclosure package for printing. Source hashes and
bounds live in `assets/broglow-model.json`; regenerate both outputs rather than
editing them. From a full checkout, activate the CAD Python environment and run:

```bash
python -m pip install -r models/broglow/requirements.txt
python scripts/export-web-model.py
python scripts/export-web-model.py --check
python scripts/export-web-model.py --exploded
python scripts/export-web-model.py --exploded --check
```

Export needs the CAD skill at `~/.agents/skills/cad` or `--cad-skill /path/to/cad`.
`--check` needs only stdlib and verifies source hashes when the CAD tree is present;
it reports skipped comparisons in reduced exports. Ordinary website builds use
the committed GLB without CAD dependencies. Keep optional codecs self-hosted if
adding Draco/KTX compression. Vendor sources and licenses are in `vendor/README.md`.

## Languages and behavior

The first supported browser language selects RU/EN, otherwise English. Manual
choice wins and is shared through localStorage key `broglow-language`; clear it
to restore detection. `guide/assets/language.js` preserves page/section and uses
`?lang=ru` / `?lang=en` when storage is blocked. Without JavaScript, guide links
work in their URL's language and prompts can be copied manually.

Material provides navigation, local search and copy buttons; `copy.js` adds
accessible success/failure feedback. Fonts are system fonts; no analytics or
external font service is configured.

## Check changes

```bash
site/.venv/bin/python site/check-translations.py
node site/i18n.test.cjs
node site/model-preview.test.cjs
node site/docs-language.test.cjs
node site/docs-copy.test.cjs
site/.venv/bin/python site/pages.test.py
site/.venv/bin/mkdocs build --strict -f site/mkdocs.yaml
python3 site/check-links.py
python3 site/check-links.py --base-url http://127.0.0.1:8080/site/
```

The link check covers generated pages, anchors, language links and downloads.
Keep the preview server running for the HTTP check.

## Local downloads and factual limits

`pages.py` expands `<!-- mac-downloads -->` and `<!-- model-download -->`:
local previews link to the newest complete Mac archive in `dist/` and enclosure
archive `dist/enclosure/broglow-enclosure-vVERSION.zip`, each with a sibling
`.sha256`. Without archives, they offer source instructions. Public builds link
to Releases and the source ZIP. GitHub's automatic source ZIP contains editable
source and instructions, without STL, STEP/STP or ready-to-print ZIPs; use the
enclosure release asset or [generate it from source](guide/en/releasing.md).
Previews neither rebuild nor copy archives; inspect a package's `BUILD.json` for
its architecture, date and source.

Print-guide STL links from `models/broglow/parts.json` and the four named STEP
views route to the versioned enclosure release asset in public builds. Local
builds use the enclosure archive above, or the generation instructions when it
is absent. The original member path stays visible for use after extraction;
Markdown links inside the extracted package remain relative. Unknown or missing
source links still fail the build. Keep the manual and agent generation procedures
in the [release runbook](guide/en/releasing.md) aligned with the release workflow.

Use the [hardware manual](guide/en/hardware.md) for supported hardware and
acceptance checks. Website tests and renders verify presentation, not physical
fit or delivery of agent events.

Our asset sources:

- `assets/broglow.png`: `site/assets/broglow.glb`, rendered in the default Working state with the pinned model-viewer/effects and hero camera.
- `assets/assembly.glb`: separated assembly from `models/broglow/exploded.py`; source hashes and bounds are recorded in `assets/assembly-model.json`. Regenerate with `--exploded` when its CAD source changes.
- `guide/assets/favicon.svg`: our three-circle lamp artwork, also used by the landing favicon. Tray icons come from `assets/icons/` in the repository root.

Follow the [media policy and photo checklist](guide/en/media.md). Keep render/photo
labels accurate and theme/library license notices intact.

## Publish to GitHub Pages

1. Follow the [release runbook](guide/en/releasing.md) and push the intended sources,
   including CAD, to your repository's default branch.
2. Select **Settings → Pages → Build and deployment → Source → GitHub Actions**.
3. Run **Actions → Website → Run workflow** on the default branch.
4. Open the `deploy` job's URL. If required, allow the default branch and approve
   deployment in the `github-pages` environment.

The [Website workflow](../.github/workflows/pages.yaml) publishes the landing at
`https://OWNER.github.io/REPOSITORY/` and guides at `docs/` and `docs/ru/`.
Site guide changes on `master` deploy automatically; adjust `push.branches` for another
default branch. PRs check without deploying. Configure custom domains in Pages
settings; relative paths support both domain roots and repository prefixes.

### Downloads live in GitHub Releases

Public downloads use this repository's Releases with a source-build fallback.
Publish full Mac/C3 bundles and the separate enclosure ZIP with matching checksums;
Windows bundles include the graphical tray app and CLI but omit the installer,
firmware and manuals. See the [release runbook](guide/en/releasing.md)
for filenames, checks and publication steps. Preserve archive names and state
pending physical checks in release notes. Building the website does not publish
an app release.

### Check the exact Pages artifact locally

Replace `OWNER/REPOSITORY`; build from a full checkout. No GitHub access is needed:

```bash
site/.venv/bin/python site/pages.py --repository OWNER/REPOSITORY
python3 site/check-links.py --site-dir site/_pages
python3 -m http.server 8081 --bind 127.0.0.1 --directory site/_pages
```

Open http://127.0.0.1:8081/. Only ignored `site/_pages/` is uploaded: landing,
generated docs and assets, including the GLB and provenance. CAD, print files,
app packages and the Python environment stay outside the artifact. Public builds
keep translated guide links on-site and direct other source links to GitHub.
Verify GitHub Actions, the live site and release downloads after publishing.
