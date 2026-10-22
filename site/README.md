# BroGlow DIY website

[English](README.md) | [Русский](README.ru.md)

The Ember homepage in `site/` is the canonical landing. It uses static
HTML/CSS/JavaScript and the same app renderer as the desktop app; no Vue, React,
npm dependencies or separate demo build are needed. The bilingual guide uses
[MkDocs](https://www.mkdocs.org/), [Material](https://squidfunk.github.io/mkdocs-material/)
and [static-i18n](https://ultrabug.github.io/mkdocs-static-i18n/).

The previous prototype from `demos/homepage/` has been promoted here. The
[shared UI contract](SHARED-UI.md) explains the app boundary; the dated
[public-source research](REFERENCE-SOURCE.md) records the Blink Buddy reference.
The [experience comparison](comparison.html) and [viewport harness](viewport-check.html)
are local review tools and are excluded from the published Pages artifact.

## Edit the guide

Edit Markdown sources; never edit or commit generated HTML.

| Topic | English | Russian |
| --- | --- | --- |
| Parts, models and wiring | `guide/en/index.md` | `guide/ru/index.md` |
| Homebrew installation, PATH and Bluetooth | `guide/en/installation.md` | `guide/ru/installation.md` |
| Why BroGlow | `guide/en/why.md` | `guide/ru/why.md` |
| Mac package, source builds, Windows and Linux | `guide/en/alternatives.md` | `guide/ru/alternatives.md` |
| Coding agents (Claude Code, Codex, opencode, Copilot) | `guide/en/agents.md` | `guide/ru/agents.md` |
| Sessions: return, names, dismiss | `guide/en/sessions.md` | `guide/ru/sessions.md` |
| Troubleshooting and agent prompt | `guide/en/troubleshooting.md` | `guide/ru/troubleshooting.md` |
| Hardware reference | `guide/en/hardware.md` | `guide/ru/hardware.md` |
| Printing and assembly (includes `../models/magnetic/GUIDE_ROUND3X2*.md`) | `guide/en/printing.md` | `guide/ru/printing.md` |
| Firmware reference | `guide/en/firmware.md` | `guide/ru/firmware.md` |

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

Run from the **broglow repository root** with [uv](https://docs.astral.sh/uv/getting-started/installation/).
CI uses Python 3.14. MkDocs commands use the locked `docs` dependency group in
`pyproject.toml`; `--only-group docs` installs it without the firmware tools.

```bash
uv run --frozen --only-group docs mkdocs build --strict -f site/mkdocs.yaml
uv run --no-project --python 3.14 python -m http.server 8080 --bind 127.0.0.1
```

Open http://127.0.0.1:8080/site/ for the landing and
http://127.0.0.1:8080/site/docs/ for the guide. Rebuild after Markdown edits;
output goes to ignored `site/docs/` without cleaning the landing.
Website builds need no generated STL, STEP/STP or ZIP files and do not run CAD.
Keep those exports ignored; GitHub print packages are distributed as release assets.

For automatic guide refresh, use
`uv run --frozen --only-group docs mkdocs serve -f site/mkdocs.yaml`.
It serves only the guide; use the repository-root server above to check the
landing and local downloads.

## Interactive 3D and AR

Normal document scrolling moves the page and changes the assembled lamp's camera
and signal as the model section passes through the viewport. The page has no
pinned narrative or scroll snapping. Drag or arrow keys rotate the model;
**Working**, **Paused** and **Needs you** select green, yellow and red, and the
brightness slider changes the lit material's emissive strength. Zoom is disabled
so wheel and vertical touch gestures keep scrolling the page. Reduced-motion
preference disables scripted camera movement.

`lamp.js` uses self-hosted `@google/model-viewer` 4.3.1 with
`assets/broglow.glb`; Three.js resolves through the local import map
(see [vendor/README.md](vendor/README.md)). The LED material indices are checked
against the exported GLB in `demo.test.cjs`. While the model loads or after a
failure, a real lamp photo is shown; failed loading disables controls that require
3D. The photo changes with the selected signal and retains its 7:5 proportions.
Without JavaScript the photo remains visible. The homepage no longer includes the
old exploded assembly explorer; printing and assembly instructions remain in the
guide. `assets/assembly.glb` remains an export asset.

**View in your space** starts native AR directly on supported phones. Other
browsers open a keyboard-accessible handoff dialog with a locally generated QR
code and an **Open on this device** link. The QR uses the current site path with
`#view-in-space` and the selected `?lang=en` / `?lang=ru`, dropping other query
parameters. Localhost/file previews suppress an unreachable QR and offer the
public viewer. Use a phone-accessible HTTPS address to test handoff. QR generation
failure keeps the on-device link usable. The pinned QR encoder and license live
in `vendor/`.

AR uses `webxr`, `scene-viewer` and `quick-look` with `ar-scale="fixed"`; the GLB
uses meters. Safari on iPhone/iPad generates USDZ on demand (`ios-src` is omitted);
other iOS browsers are unverified. Scroll-driven changes pause during AR. Android
Scene Viewer reloads the original GLB, so browser material changes do not reach it.
WebXR requires HTTPS or loopback; a phone cannot reach another device's localhost.
Physical phone placement and size still need acceptance checks. See the
[component's AR reference](https://modelviewer.dev/examples/augmentedreality/).

The model depicts the magnetic enclosure from `models/magnetic/` with reference
magnets and electronics (`appearance.py` for the assembled preview, `exploded.py` for the
explorer). Use the separate enclosure package for printing. Source hashes and
bounds live in `assets/broglow-model.json`; regenerate both outputs rather than
editing them. From a full checkout, run the export with the locked `cad` dependency group:

```bash
uv run --frozen --only-group cad python scripts/export-web-model.py
uv run --no-project --python 3.14 python scripts/export-web-model.py --check
uv run --frozen --only-group cad python scripts/export-web-model.py --exploded
uv run --no-project --python 3.14 python scripts/export-web-model.py --exploded --check
```

Export needs the CAD skill at `~/.agents/skills/cad` or `--cad-skill /path/to/cad`.
`--check` needs only stdlib and verifies source hashes and the `cad` group's
build123d pin when the CAD tree is present;
it reports skipped comparisons in reduced exports. Keep optional codecs self-hosted if
adding Draco/KTX compression.

## Languages and behavior

The landing defaults to English, including its static HTML and metadata. The
EN/RU dictionaries and `data-i18n` bindings live in `language.js`. An explicit
`?lang=en` / `?lang=ru` wins over localStorage key `broglow-language`; otherwise the
saved choice wins, then English. The language icon offers EN/RU and updates the
current URL and translated guide links. The guide uses the URL's language unless
a saved or explicit choice overrides it; `guide/assets/language.js` preserves
page/section and query parameters when storage is blocked. Browser language does
not override these defaults. Without JavaScript, links and installation commands
remain usable in the static page's language.

The landing's Open Graph and Twitter cards use English text and our real
`assets/photos/broglow-all-lights.jpg` lamp photo. Absolute canonical and preview
URLs point to the published site; `pages.py` adjusts them for `--repository`.
Browser metadata follows a manual language change; crawlers receive static English.

The hero demonstrates session monitoring with the real app renderer in a
scripts-only sandbox iframe, using sample state and a simulated native host.
Try Working/Paused/Needs you, terminal focus, dismissals and Settings. **Track
usage** exposes sample per-agent Claude/Codex quotas, opencode tokens and an
all-agent token total. **Show usage in menu bar** controls the status-bar readout
independently. Names, grouping, dashboard visibility, mixed-state priority and the
return-to-chats timeout use the shared renderer. Demo settings and dismissals stay
in memory; it reads no real sessions and cannot install software or control
hardware. See [SHARED-UI.md](SHARED-UI.md) for the source and host contract.

BroGlow works before building or getting a lamp. The header and install section
link to the installation guide; the copyable Homebrew commands omit decorative
shell prompts from selected/copied text. The terminal dashboard and menu-bar app
can be used on their own. The FAQ also links directly to the Printables and
MakerWorld models for visitors who want a lamp.

GitHub links in the hero and source section lead to the repository. `github.js`
loads star and fork counts from GitHub's public API; the links remain usable
without JavaScript or when the request fails. `pages.py` adjusts repository links
for `--repository`. No separate Releases/Issues/license link row is shown.

`theme.js` runs before paint to apply system/light/dark preference without a
flash. It reads `?theme=auto|light|dark`, then localStorage key `broglow-theme`,
then the system color scheme; the brand palette is fixed Ember. The header uses
language and theme icons with accessible labels and native selectors. Other
classic scripts use `defer`; `lamp.js` is a module. The guide uses Material's
three-way palette toggle with its own storage. Material provides navigation,
local search and copy buttons; `copy.js` adds accessible success/failure feedback.
Fonts are system fonts; no analytics or external font service is configured.

## Check changes

```bash
uv run --frozen --only-group docs python site/check-translations.py
node --test site/*.test.cjs
node --test tests/js/lamp-status.test.mjs tests/js/popover-library.test.mjs
uv run --frozen --only-group test pytest site/test_pages.py
uv run --frozen --only-group docs mkdocs build --strict -f site/mkdocs.yaml
uv run --no-project --python 3.14 python site/check-links.py
uv run --no-project --python 3.14 python site/check-links.py --base-url http://127.0.0.1:8080/site/
```

The tests cover shared UI boundaries, settings, tracking, terminal/window
interaction, localization, assets, 3D/AR and social tags. The link check covers
generated pages, anchors, language links and downloads. Keep the preview server
running for the HTTP check. Check narrow and desktop layouts with
`site/viewport-check.html`; these checks do not verify installed services,
installers, physical hardware, phone AR or live social scraping.

## Local downloads and factual limits

The GitHub enclosure download is described in the
[release runbook](../RELEASING.md#distribution); `printing.md` includes
its guide from `models/magnetic/`.

The authored build pages and shared printing guides also link to the published
[Printables](https://www.printables.com/model/1884645) and
[MakerWorld](https://makerworld.com/models/3468773) models. Keep those links in the
Markdown sources, alongside the generated GitHub download choices.

`pages.py` expands `<!-- mac-downloads -->` and `<!-- model-download -->`:
local previews link to the newest complete Mac archive in `dist/` (with its
sibling `.sha256`) and to `dist/enclosure/broglow-models-vVERSION.zip`,
whose checksum is shown inline from the sibling `.sha256`. Without archives, they
offer source instructions. Public builds link to Releases and the source ZIP; the
Pages build fetches the published archive's `.sha256` from the release for the
current `Cargo.toml` version and shows it inline; when the
checksum cannot be fetched the page points to the release page instead. Publishing a release restarts
the Website workflow on the default branch so the checksum lands on the site. GitHub's automatic source ZIP contains editable
source and instructions, without STL, STEP/STP or ready-to-print ZIPs; use the
enclosure release asset, either model listing, or [generate it from source](../RELEASING.md#package-magnetic-enclosure).
Previews neither rebuild nor copy archives; inspect a package's `BUILD.json` for
its architecture, date and source.

Unknown or missing source links in included guides fail the build. Keep the
release runbook's generation procedure aligned with the release workflow.

Use the [hardware manual](guide/en/hardware.md) for supported hardware and
acceptance checks. Website tests and renders verify presentation, not physical
fit or delivery of agent events.

The [media policy](../MEDIA.md) lists the landing photos and 3D outputs;
`photos/broglow-all-lights.jpg`, `photos/signal-green.jpg` and `broglow.jpg` are
7:5 crops at 1400×1000. Other asset sources:

- `assets/photos/`: our photos of the built lamp (`state-*.jpg` squares and `signal-*.jpg` 7:5 crops with one LED lit, `inside-*.jpg` rear views with and without the LED retainer) and the `popover-dark.png` menu-bar screenshot, web-sized; originals stay untracked in `originals/`.
- `guide/assets/favicon.svg`: our three-circle lamp artwork, also used by the landing favicon. Tray icons come from `assets/icons/` in the repository root.

## Publish to GitHub Pages

1. Push the intended sources, including CAD, to your repository's default branch.
2. Select **Settings → Pages → Build and deployment → Source → GitHub Actions**.
3. Run **Actions → Website → Run workflow** on the default branch.
4. Open the `deploy` job's URL. If required, allow the default branch and approve
   deployment in the `github-pages` environment.

The [Website workflow](../.github/workflows/pages.yaml) publishes the landing at
`https://OWNER.github.io/REPOSITORY/` and guides at `docs/` and `docs/ru/`.
Pushes to `master` that touch the site, docs, models or other paths listed in the workflow's `push.paths` deploy automatically; adjust `push.branches` for another
default branch. PRs check without deploying. Configure custom domains in Pages
settings; relative paths support both domain roots and repository prefixes.

Downloads live in GitHub Releases; see the [release distribution](../RELEASING.md#distribution).

### Check the exact Pages artifact locally

Replace `OWNER/REPOSITORY`; build from a full checkout. No GitHub access is needed:

```bash
uv run --frozen --only-group docs python site/pages.py --repository OWNER/REPOSITORY
uv run --no-project --python 3.14 python site/check-links.py --site-dir site/_pages
uv run --no-project --python 3.14 python -m http.server 8081 --bind 127.0.0.1 --directory site/_pages
```

Open http://127.0.0.1:8081/. Only ignored `site/_pages/` is uploaded: landing,
generated docs and assets, including the GLB, provenance and native UI copies in
`app-ui/`. `site/pages.py` is the sole production builder; it remaps source paths
from `../assets/` to `app-ui/` so the artifact runs without the checkout.
`comparison.html`, `viewport-check.html`, tests and developer Markdown are excluded.
Rebuild after changing the landing or shared UI. CAD, print files,
app packages and the Python environment stay outside the artifact. Public builds
keep translated guide links on-site and direct other source links to GitHub.
Verify GitHub Actions, the live site and release downloads after publishing.
