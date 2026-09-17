# ClawLight DIY website

The short landing is plain HTML/CSS. The separate guide uses [MkDocs](https://www.mkdocs.org/)
with [Material](https://squidfunk.github.io/mkdocs-material/) and
[static-i18n](https://ultrabug.github.io/mkdocs-static-i18n/). This follows the original
project's [landing](https://clawlight.dev/) / [docs](https://clawlight.dev/docs) split.

## Edit the guide

Edit ordinary Markdown files; generated HTML is never edited or committed.

| Topic | English | Russian |
| --- | --- | --- |
| Parts, models and wiring | `guide/en/index.md` | `guide/ru/index.md` |
| Installation, agent prompt and Bluetooth | `guide/en/installation.md` | `guide/ru/installation.md` |
| Codex integration | `guide/en/codex.md` | `guide/ru/codex.md` |
| Troubleshooting and agent prompt | `guide/en/troubleshooting.md` | `guide/ru/troubleshooting.md` |
| Full hardware reference | `../docs/hardware-setup.md` | Same source, marked as English |

Navigation and theme settings live in `mkdocs.yaml`. To add a page, add the same
filename in both language folders and add its entry to `nav` (and its Russian
label to `nav_translations`). Agent prompts are fenced `text` blocks; Material
adds the copy button. Tables and links use normal Markdown.

## Keep the instructions in sync

Edit **`docs/hardware-setup.md` once** for the detailed operating reference. The
website's Hardware reference page includes that file directly using the existing
PyMdown Snippets extension. It is rebuilt in both language routes; the Russian
route clearly labels the full reference as English. The source remains ordinary
Markdown, readable in GitHub and included in app packages.

The wiring diagram and PATH commands in both quick guides also come from named
sections of that file. For example:

```text
--8<-- "docs/hardware-setup.md:lamp-wiring"
```

Invisible `start:lamp-wiring` / `end:lamp-wiring` comments delimit the shared block.
The same pattern supplies `path-check` and `path-setup`. Change the original block;
do not paste a second copy into the website. Missing sections fail the build.
Both quick guides refer to the diagram for resistor values, and the landing links
to current test results instead of keeping a separate acceptance summary.

For a behavior change, review the affected short explanations and agent prompts
in **both** `guide/en/` and `guide/ru/` in the same change. These summaries are edited
by a person or coding agent; they are not automatically translated. `AGENTS.md`
routes future implementation work here, so the user need not relay changes between
sessions. Run the checks below and the Pages build before committing.

Changes to the hardware guide trigger the Website workflow, including PR checks.
After Pages is enabled, a push to `main` rebuilds the published reference
automatically. Locally, `mkdocs serve` watches the source guide; for the complete
static preview, rerun the documented build command after edits.

## Build and preview

Run these from the **clawlight repository root**. Python 3.10+ is required.

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
node site/i18n.test.cjs
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

Local preview downloads reference the authoritative files in `models/` and `dist/`; they are not
copied into the website. Update both package and checksum links in **both**
`guide/en/installation.md` and `guide/ru/installation.md` when replacing a package.
Relative download paths follow the generated URL depth, including the extra `ru/`.
The Mac package is an Apple Silicon build with C3 firmware, not an upstream release.
The package includes the full manual and Codex setup guide; the website keeps
the essential steps and agent prompts self-contained.

Copy was checked against the implementation, hardware guide, print guide and
upstream README on 2026-09-16. Mac USB power is supported; a power adapter and
printed enclosure are optional. C3 is recommended, while other BLE SuperMini
boards require suitable firmware/pins and an enclosure fit check. `connected`
means the service wrote to the indicated lamp; it does not prove LED operation
or that Codex is sending hook events. Desktop delivery needs a real desktop task
check; cloud/remote tasks and ordinary ChatGPT chats do not drive the local service.

Use the hardware guide's latest results for current physical acceptance; its
historical test entries remain dated. Website builds and tests do not count as
hardware or installed-service checks.

Images are the project's own CAD previews, copied into `assets/`:

- `clawlight.png`: `models/clawlight/preview/finished_20260916T173845Z.png`
- `assembly.png`: `models/clawlight/preview/exploded_20260916T173845Z.png`

## Publish to GitHub Pages

The prepared [Website workflow](../.github/workflows/pages.yaml) publishes the
landing at `https://OWNER.github.io/REPOSITORY/` and the guide at `docs/` beneath it.
RU lives at `docs/ru/`. Relative navigation also works with a custom domain.
No `gh-pages` branch, committed generated HTML, or custom token is needed.

When you are ready:

1. Push the project to **your fork**, including the firmware, models and
   `docs/hardware-setup.md` from their separate workstream. The source-download
   fallback needs those files in the fork's default branch.
2. In **Settings → Pages → Build and deployment → Source**, choose **GitHub Actions**.
   Enable Actions in the fork if GitHub prompts you to do so.
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

The public guide links to **your fork's Releases**, taken from the workflow's
repository context. It never advertises the upstream CLI-only archive as the BLE
package. Until a BLE package is available, it offers the fork's source ZIP and an
agent prompt that supports building from source.

When you have verified a BLE package, publish a release in your fork and attach:

- The full `dist/clawlight-…-aarch64-apple-darwin-….tar.gz` package and its matching
  `.tar.gz.sha256` from the packaging workflow in `docs/hardware-setup.md`.
- `models/clawlight-print-pack.zip` for the enclosure download.

Keep the original package filename so its checksum file still names the correct
archive. Explain the supported Mac/board and remaining hardware checks in the
release notes. The website links to the release list, so publishing a newer
package does not require a website edit. See GitHub's
[release-link documentation](https://docs.github.com/en/repositories/releasing-projects-on-github/linking-to-releases).

The existing upstream `release.yaml` produces CLI-only archives and also updates
the upstream Homebrew tap; it is **not** the full BLE packaging workflow. For a
manual BLE release, create a release tag outside its `v*` trigger (for example,
`ble-2026-09-16`) unless you have separately adapted that workflow for your fork.
This website preparation does not change or run the app release workflow.

### Check the exact Pages artifact locally

Replace `OWNER/REPOSITORY` with your future fork. No GitHub access is needed to build:

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
their direct local downloads. In the included reference, repository-file links
open the fork on GitHub; local previews show their checkout paths. The small tray
icons are embedded from `assets/icons/`. Models and packages are not copied into
the Pages artifact. You can edit the same Markdown pages for both previews.

The Pages build now also reads `docs/hardware-setup.md` and its referenced repository
files, so build from a full checkout. Local builds, HTTP links at both root and
repository-prefixed URLs, and language/copy tests are verified. GitHub Actions,
the public Pages URL and release downloads need verification after you publish.
No repository, release or public site was created by this preparation.
