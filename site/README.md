# ClawLight DIY landing

Static Russian/English landing page. No dependencies, build step, analytics or
backend. All work is scoped to `site/`; the application and firmware are separate.

Run from the **clawlight repository root** so model and Mac package downloads resolve:

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
the Russian page and signal demo remain usable. Switching languages does not reset
the selected demo signal or open setup sections.

Both setup links lead to the bilingual `#setup` section on this page. It covers
parts, wiring, installation and USB-assisted pairing in four steps. Optional agent
setup, Codex integration and troubleshooting use native expandable sections. Full technical procedures
remain in `docs/hardware-setup.md` and in the downloadable package; the site does not
navigate visitors to raw Markdown.

The parts table recommends ESP32-C3 without imposing its memory size on other
Bluetooth LE SuperMini boards. The included firmware and GPIO example are specific
to C3; other chips require suitable firmware/pins and an enclosure fit check.
Setup and troubleshooting prompts have copy buttons that copy the currently selected
language. Copy feedback is announced to screen readers. If browser clipboard access
is unavailable, the page explains how to copy the selectable text manually. No prompts
are sent to an agent automatically. Without JavaScript, the prompts remain readable.

Run the localization, copy-button and link checks from the repository root (Node.js and Python 3,
no packages required):

```bash
node site/i18n.test.cjs
python3 site/check-links.py
```

To also check that every local file is served correctly by the preview:

```bash
python3 site/check-links.py --base-url http://127.0.0.1:8080/site/
```

Downloads use the authoritative files in `models/` and `dist/`, without duplicating
them in `site/`. The Mac package is an existing Apple Silicon build with C3 firmware;
it is not a universal Mac binary or an upstream release. Keep its `.tar.gz` and
`.sha256` together. When replacing the package, update both links in `index.html`
and rerun the link check. The instructions delegate checksum verification and safe
installation to the included agent runbook, with manual steps in the same package.
The currently linked package predates the hardware guide's “Connect local Codex
sessions” section. Keep the site's Codex steps and agent prompts self-contained;
they cover manual hook approval and a real local task check, including desktop tasks.
Do not describe desktop hook delivery as verified until that check succeeds.

Images are copies of the project's own CAD previews:
- `assets/clawlight.png`: `models/clawlight/preview/finished_20260916T173845Z.png`
- `assets/assembly.png`: `models/clawlight/preview/exploded_20260916T173845Z.png`

Copy is based on the local hardware guide. The design references
https://clawlight.dev/ and credits the original project in the footer. Product
photographs, store pricing and claims of verified hardware are not reused.
The build guide is authoritative; landing copy does not replace it.

Fact-check completed on 2026-09-16 against the current implementation, hardware
guide, print guide, packaged artifacts, and upstream README:

| Claim | Basis and limit |
| --- | --- |
| USB power | The Mac can power the board while BLE controls it. A separate adapter is optional. |
| Parts and enclosure | C3 is recommended; another BLE SuperMini needs adapted firmware/pins and a fit check. The supplied case assumes 5 mm LEDs and needs five screws, two ties and glue; see `models/clawlight/PRINT_GUIDE.md`. |
| Signals | `src/state.rs` defines aggregate priority; firmware also turns LEDs off when disabled or stale. Red reflects a reported attention event, not every conversational question. |
| Connection | `connected` confirms a recent write to the indicated lamp, not visible LED operation. Reconnect requires enabled control, a running Mac service, Bluetooth and a powered nearby lamp. |
| Supported agents | `src/harness.rs` and `src/hook.rs` implement local integrations. The hardware guide's Codex steps cover CLI and local desktop tasks with trusted hooks; desktop event delivery still needs a real task check. Cloud/remote tasks and ordinary ChatGPT conversations do not drive this local service. |
| Verification | The hardware guide records BLE/GPIO checks on one Mac/C3. Physical LEDs, printed fit, brick-only power, sleep/wake recovery and two physical lamps remain unverified. |
| Downloads and images | Current package checksum and model ZIP integrity checked. Preview images now match the print guide's current revision; download sizes are omitted to avoid stale labels. |

This was a content/artifact audit, not a new hardware or installed-service test.

If publishing later, include the linked model archive and Mac package/checksum
under the same directory structure or update their links to actual hosted artifacts.
`dist/` is ignored build output: a Git clone alone does not supply the Mac download.
The link check must pass against the deployment files before publishing. Do not
point local BLE setup to upstream releases: they do not contain this extension.
