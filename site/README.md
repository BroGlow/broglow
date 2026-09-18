# ClawLight DIY website

[English](README.md) | [Русский](README.ru.md)

The short landing is plain HTML/CSS. The separate guide uses [MkDocs](https://www.mkdocs.org/)
with [Material](https://squidfunk.github.io/mkdocs-material/) and
[static-i18n](https://ultrabug.github.io/mkdocs-static-i18n/). These pages describe
ClawLight's app, ESP32-C3 firmware and printable enclosure.

## Edit the guide

Edit ordinary Markdown files; generated HTML is never edited or committed.

| Topic | English | Russian |
| --- | --- | --- |
| Parts, models and wiring | `guide/en/index.md` | `guide/ru/index.md` |
| Installation, agent prompt and Bluetooth | `guide/en/installation.md` | `guide/ru/installation.md` |
| Codex integration | `guide/en/codex.md` | `guide/ru/codex.md` |
| Troubleshooting and agent prompt | `guide/en/troubleshooting.md` | `guide/ru/troubleshooting.md` |
| Full hardware reference | `../docs/hardware-setup.md` | `../docs/hardware-setup.ru.md` |
| Firmware, releases, images, enclosure design | `../docs/NAME.md` | `../docs/NAME.ru.md` |
| Printing and assembly | `../models/clawlight/PRINT_GUIDE.en.md` | `../models/clawlight/PRINT_GUIDE.md` |

Navigation and theme settings live in `mkdocs.yaml`. To add a page, add the same
filename in both language folders and add its entry to `nav` (and its Russian
label to `nav_translations`). Agent prompts are fenced `text` blocks; Material
adds the copy button. Tables and links use normal Markdown.

## Keep the instructions in sync

Maintain the full manual in **`docs/hardware-setup.md`** (English) and
**`docs/hardware-setup.ru.md`** (Russian). Each website route includes its matching
source through PyMdown Snippets. Firmware, release, media, enclosure and print
pages follow the same pattern; no language falls back to the other.
The same Markdown sources are readable in the repository and included in packages.

The wiring diagram and PATH commands in each quick guide come from named sections
of that language's manual, for example:

```text
--8<-- "docs/hardware-setup.ru.md:lamp-wiring"
```

Keep the `start:lamp-wiring` / `end:lamp-wiring`, `path-check` and `path-setup`
markers in both manuals. Translate prose, comments and diagram labels; keep
commands, GPIOs, resistor values and protocol fields equivalent. Missing sections
fail the build. Translated headings retain the same explicit section IDs as their
counterparts, so bookmarks and language switching stay on the same section.

Every authored project document has an English/Russian counterpart, including
repository instructions and package manuals. `check-translations.py` checks file
coverage, heading anchors and included page sources; it rejects English fallback
pages. Preserve dependency license notices verbatim.

For a behavior change, review the affected short explanations and agent prompts
in **both** `guide/en/` and `guide/ru/` in the same change. These summaries are edited
by a person or coding agent; they are not automatically translated. `AGENTS.md`
routes future implementation work here, so the user need not relay changes between
sessions. Run the checks below and the Pages build before committing.

Changes to the website or documentation trigger the Website workflow, including PR checks.
After Pages is enabled, a push to `main` rebuilds the published reference
automatically. Locally, `mkdocs serve` watches the source guide; for the complete
static preview, rerun the documented build command after edits.

## Build and preview

Run these from the **clawlight repository root**. Python 3.14 is used in CI and verified locally.

```bash
python3 -m venv site/.venv
site/.venv/bin/python -m pip install -r site/requirements-docs.txt
site/.venv/bin/mkdocs build --strict -f site/mkdocs.yaml
python3 -m http.server 8080 --bind 127.0.0.1
```

Open http://127.0.0.1:8080/site/ for the landing and
http://127.0.0.1:8080/site/docs/ for the guide. After editing Markdown, repeat the
build command and refresh. The guide output is ignored under `site/docs/`; its
build never cleans the landing. Dependencies are isolated in `site/.venv/` and
pinned to compatible MkDocs 1.x / Material versions.

For live documentation editing, `site/.venv/bin/mkdocs serve -f site/mkdocs.yaml`
provides automatic refresh. That docs-only server does not serve the landing or
the external downloads; use the repository-root preview above for the complete site.

## Interactive 3D and AR

The hero starts with a static render of our 1.0.0 enclosure release candidate.
**Explore in 3D** loads the self-hosted
`@google/model-viewer` 4.3.1 module from `vendor/` and our `assets/clawlight.glb`.
No model or renderer is fetched until that click. Without JavaScript, or after a
module/model failure, the image remains usable. Drag/pinch, mouse wheel, arrows
and Page Up/Page Down use the component's camera controls; Reset view restores
the initial camera. No automatic rotation or custom renderer is used.

AR uses the component's `webxr`, `scene-viewer` and `quick-look` modes with
`ar-scale="fixed"`. The GLB is exported in meters and needs no HTML scale transform.
`ios-src` is deliberately omitted: Safari on iPhone/iPad generates USDZ on request.
Other iOS browsers are not promised. WebXR needs HTTPS (loopback is sufficient for
development); a phone cannot fetch a model from another device's localhost.
The component exposes its AR button only on supported devices. These are the
[official AR behaviors](https://modelviewer.dev/examples/augmentedreality/);
actual placement and size on physical phones still need acceptance checks.

`assets/clawlight-model.json` records the model's source and bounds. The model is
our own enclosure release candidate, including illustrative electronics. The
interactive export is a viewing aid; the separate enclosure archive provides
printable CAD and STL. Keep release-candidate wording in both languages, and
record final physical testing separately from release readiness. The exporter
owns the GLB and metadata; do not edit generated geometry in the website. To regenerate it from a full checkout, activate your CAD Python
environment and run:

```bash
python -m pip install -r models/clawlight/requirements.txt
python scripts/export-web-model.py
python scripts/export-web-model.py --check
```

Export requires the installed CAD skill at `~/.agents/skills/cad`; pass
`--cad-skill /path/to/cad` if it is elsewhere. The source model remains untouched.
`--check` uses only the Python standard library: it checks the committed GLB and
provenance, plus every recorded source hash when the CAD tree is present. In a
reduced source export without the CAD tree it explicitly skips unavailable
source comparisons. The normal 1.0.0 source archive includes our CAD.
Website CI runs that check. Ordinary site builds use the committed GLB and need no
CAD libraries. Avoid Draco/KTX compression unless those optional codecs are also
self-hosted. `vendor/README.md` records the pinned source, integrity and license.
The Pages build copies `assets/` and `vendor/` along with `model-preview.js`; link
checks include the model, module and poster URLs.

## Languages and behavior

Both pages detect the first supported browser language (RU/EN; otherwise English).
A manual choice takes priority and is stored as `clawlight-language` in localStorage,
shared between landing and guide. Clear that key to restore browser detection.
The guide's native language menu keeps the current page and section. Its small
`guide/assets/language.js` bridge preserves the landing's language preference;
`?lang=ru` / `?lang=en` lets a manual choice work even when storage is blocked.
Without JavaScript, guide text and links remain usable in their URL's language;
copy prompts manually. The landing's four-color demo remains native HTML/CSS.

Material provides responsive navigation, local search and code/prompt copying.
The small `copy.js` adapter keeps Material's buttons and uses the Clipboard API,
with visible, accessible feedback when copying succeeds or needs to be done manually.
Fonts are local system fonts; no external font service or analytics is configured.

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

The link check covers the landing and every generated guide page, including
anchors, language links and downloads. Human navigation never opens raw Markdown.

## Local downloads and factual limits

The `<!-- mac-downloads -->` and `<!-- model-download -->` markers in both language
sources are expanded by `pages.py`. Local previews use the newest complete Mac
archive in `dist/` that has a sibling `.sha256`. They also link directly to
`models/clawlight-print-pack.zip` when it exists, with instructions to open its
`START_HERE.md` / `START_HERE.ru.md` and the English or Russian print guide. If that archive is absent,
the guide explains where to find the print instructions in a full source checkout.
No archives are copied into the website. With no local Mac package, the guide
links to the source-build instructions, so a clean checkout needs no ignored
`dist/` files. Public builds link to this repository's Releases and source archive.
Relative paths account for both language routes. A local package's `BUILD.json`
identifies its architecture, date and source; a local preview does not rebuild it.
The package includes the full manual and Codex setup guide; the website keeps
the essential steps and agent prompts self-contained.

Copy was checked against the implementation, hardware guide and print guide;
project-scope wording was corrected on 2026-09-18. Mac USB power is supported; a power adapter and
printed enclosure are optional. The supplied firmware targets ESP32-C3 SuperMini;
other chips are not supported by this package. `connected`
means the service wrote to the indicated lamp; it does not prove LED operation
or that Codex is sending hook events. Desktop delivery needs a real desktop task
check; cloud/remote tasks and ordinary ChatGPT chats do not drive the local service.

Use the hardware guide's results for current physical acceptance. Website builds
and tests do not count as
hardware or installed-service checks.

Images are the project's own CAD renders, copied into `assets/`. The owner has
approved the enclosure and print package as the 1.0.0 release candidate, ready
for release. Final physical assembly testing remains in progress; renders and
website checks do not establish physical fit. Preview source files:

- `clawlight.png`: rendered from `site/assets/clawlight.glb` with the pinned model-viewer and the hero camera; it depicts the same geometry as the interactive view.
- `assembly.png`: `models/clawlight/preview/flat_floor_exploded_20260918T101601Z.png`

Use only our own photos, screenshots, CAD renders and geometric artwork.
Keep each asset's source recorded here and follow the media checklist below.

The guide's logo and favicon are our simple three-circle lamp SVG at
`guide/assets/favicon.svg`; the landing uses the same geometric design as an
inline favicon. The two CAD previews above and the GLB generated from our CAD are the site product visuals.
Material's navigation, search and copy icons are UI controls supplied by the
licensed theme, not product artwork. Keep theme/library license notices.

For the photos and screenshots still to prepare, follow the
[media checklist](../docs/media.md). Replace renders only with our own checked
images, and keep render/photograph labels accurate. The public guide also includes
any status icons referenced by the operating manual; those must use this project's
own geometric lamp artwork.

## Publish to GitHub Pages

The prepared [Website workflow](../.github/workflows/pages.yaml) publishes the
landing at `https://OWNER.github.io/REPOSITORY/` and the guide at `docs/` beneath it.
RU lives at `docs/ru/`. All local navigation and asset paths are relative, so the
same artifact works at a domain root, a repository prefix, or a Pages custom domain.
Configure a custom domain in the repository's Pages settings; no source URL rewrite
is needed.
No `gh-pages` branch, committed generated HTML, or custom token is needed.

When you are ready:

1. Push the app, firmware, documentation and enclosure sources to **your repository**.
   The source download needs those files, including our CAD, in the default branch.
   Follow the [release runbook](../docs/releasing.md) before publishing.
2. In **Settings → Pages → Build and deployment → Source**, choose **GitHub Actions**.
   Enable Actions in the repository if GitHub prompts you to do so.
3. Open **Actions → Website → Run workflow** and select the default branch.
   Later website or hardware-guide changes on `main` publish automatically. If your default
   branch has another name, update the workflow's `push.branches` entry.
4. Open the URL reported by the `deploy` job. If your `github-pages` environment
   requires approval, approve that deployment in GitHub. Allow the default branch
   in that environment's deployment rules.

Pull requests build and check the website without deployment permissions or a
public preview. Only the default branch can deploy. The workflow uses GitHub's
[standard Pages actions](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).

### Downloads live in GitHub Releases

The public guide links to **this repository's Releases**, taken from the workflow's
repository context, plus its source ZIP and an agent prompt that supports building
from source. Publish the complete Mac/C3 package for the installation guide's
prebuilt route; CLI-only archives do not contain the firmware and manual.

When you have verified full Mac packages, publish them with matching `.sha256`
files using the release runbook. Publish the enclosure separately as
`clawlight-enclosure-vVERSION.zip` with a matching `.zip.sha256`. The 1.0.0
release candidate contains the print package and guide; its release notes must
state the final physical checks still pending.

Keep the original package filename so its checksum file still names the correct
archive. Explain the supported Mac/board and remaining hardware checks in the
release notes. The website links to the release list, so publishing a newer
package does not require a website edit. See GitHub's
[release-link documentation](https://docs.github.com/en/repositories/releasing-projects-on-github/linking-to-releases).

The release workflow produces complete Mac/C3 bundles for Apple Silicon and Intel:
`clawlight-vVERSION-aarch64-apple-darwin.tar.gz` and
`clawlight-vVERSION-x86_64-apple-darwin.tar.gz`, each with its `.sha256`.
Windows CLI archives use a distinct `clawlight-cli-…` name and omit firmware and
the operating guide. The website identifies the full Mac filenames explicitly;
before the first complete release it offers the source-build route. The build
guide identifies the separate enclosure ZIP and checksum, links to the print
guide in the repository, and offers the source ZIP with CAD and STL as a fallback.
See [the release runbook](../docs/releasing.md) for release checks and assets.
Building this website does not publish an app release.

### Check the exact Pages artifact locally

Replace `OWNER/REPOSITORY` with this project's repository. No GitHub access is needed to build:

```bash
site/.venv/bin/python site/pages.py --repository OWNER/REPOSITORY
python3 site/check-links.py --site-dir site/_pages
python3 -m http.server 8081 --bind 127.0.0.1 --directory site/_pages
```

Open http://127.0.0.1:8081/. The ignored `site/_pages/` folder contains only the
landing, generated docs and their assets. The Pages workflow uploads this folder;
it does not publish repository files, the Python environment, or local binaries.
`pages.py` also supplies a small MkDocs hook: public builds rewrite local download
links to Releases and add the source fallback, while ordinary local previews keep
their direct local downloads. In included references, links to translated guides stay on the website in the
selected language. Other repository-file links open GitHub in public builds;
local previews show their checkout paths. The small tray
icons are embedded from this project's original artwork in `assets/icons/`. The enclosure GLB and its provenance metadata are included in the Pages artifact;
editable CAD, print files and app packages are not. You can edit the same Markdown pages for both previews.

The Pages build reads both language versions under `docs/` and the print guides,
so build from a full checkout. Local builds, HTTP links at both root and
repository-prefixed URLs, and language/copy tests are verified. GitHub Actions,
the public Pages URL and release downloads need verification after you publish.
No repository, release or public site was created by this preparation.
