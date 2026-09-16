# ClawLight DIY landing

Static Russian/English landing page. No dependencies, build step, analytics or
backend. All work is scoped to `site/`; the application and firmware are separate.

Run from the **clawlight repository root** so the guide and model links resolve:

```bash
python3 -m http.server 8080 --bind 127.0.0.1
```

Open http://127.0.0.1:8080/site/ . The page also opens directly from `index.html`.
Serve over HTTP when reviewing normal link/navigation behavior. No deployment
or remote repository changes are needed.

The four-state demo uses native radio controls and CSS, with keyboard support
and reduced-motion handling. It illustrates signals; it never connects to a lamp.

The header's RU/EN selector switches the entire landing, including image descriptions
and page metadata. The first visit uses the first supported language in the browser's
language preferences (`ru` or `en`, including regional variants), falling back to
English. A manual choice takes priority on later visits and is saved in localStorage
under `clawlight-language`. Clearing that key restores browser-language detection.
If storage is blocked, switching still works for the current page. Without JavaScript,
the Russian page and signal demo remain usable. Linked project documents keep their
original language. Switching languages does not reset the selected demo signal.

Run the localization check from the repository root (Node.js, no packages required):

```bash
node site/i18n.test.cjs
```

Images are copies of the project's own CAD previews:
- `assets/clawlight.png`: `models/clawlight/preview/finished_20260916T150307Z.png`
- `assets/assembly.png`: `models/clawlight/preview/exploded_20260916T150307Z.png`

Copy is based on the local hardware guide. The design references
https://clawlight.dev/ and credits the original project in the footer. Product
photographs, store pricing and claims of verified hardware are not reused.
The build guide is authoritative; landing copy does not replace it.

If publishing later, include the linked docs/models under the same directory
structure or update their two link targets to actual hosted artifacts. Do not
point local BLE setup to upstream releases: they do not contain this extension.
