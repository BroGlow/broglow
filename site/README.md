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

Navigation and theme settings live in `mkdocs.yml`. To add a page, add the same
filename in both language folders and add its entry to `nav` (and its Russian
label to `nav_translations`). Agent prompts are fenced `text` blocks; Material
adds the copy button. Tables and links use normal Markdown.

The detailed operating reference remains `docs/hardware-setup.md` at the repository
root. Keep the short human guide in sync with it; firmware/service changes belong
to that separate workstream.

## Build and preview

Run these from the **clawlight repository root**. Python 3.10+ is required.

```bash
python3 -m venv site/.venv
site/.venv/bin/python -m pip install -r site/requirements-docs.txt
site/.venv/bin/mkdocs build --strict -f site/mkdocs.yml
python3 -m http.server 8080 --bind 127.0.0.1
```

Open http://127.0.0.1:8080/site/ for the landing and
http://127.0.0.1:8080/site/docs/ for the guide. After editing Markdown, repeat the
build command and refresh. The guide output is ignored under `site/docs/`; its
build never cleans the landing. Dependencies are isolated in `site/.venv/` and
pinned to compatible MkDocs 1.x / Material versions.

For live documentation editing, `site/.venv/bin/mkdocs serve -f site/mkdocs.yml`
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
site/.venv/bin/mkdocs build --strict -f site/mkdocs.yml
python3 site/check-links.py
python3 site/check-links.py --base-url http://127.0.0.1:8080/site/
```

The link check covers the landing and every generated guide page, including
anchors, language links and downloads. Human navigation never opens raw Markdown.

## Downloads and factual limits

Downloads reference the authoritative files in `models/` and `dist/`; they are not
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

The hardware guide records BLE/GPIO checks on one Mac/C3. Physical LEDs, printed
fit, brick-only power, sleep/wake recovery and two physical lamps remain unverified.
Website builds and tests do not count as hardware or installed-service checks.

Images are the project's own CAD previews, copied into `assets/`:

- `clawlight.png`: `models/clawlight/preview/finished_20260916T173845Z.png`
- `assembly.png`: `models/clawlight/preview/exploded_20260916T173845Z.png`

## Future publishing

Build the guide before publishing, then include the landing, generated `docs/`,
models ZIP and Mac package/checksum in the same relative structure. `dist/` is
ignored build output; a Git clone alone does not supply the downloads. Run the
link check against the assembled files. This task does not publish or push anything.
