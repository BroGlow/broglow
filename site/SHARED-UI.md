[English](SHARED-UI.md) | [Русский](SHARED-UI.ru.md)

<a name="shared-ui"></a>
# One app interface, two hosts

The desktop app and landing demo use the same UI sources in `assets/popover/`.
There is no Vue or React dependency and no second recreation of the popover.
The landing is static HTML/CSS/JavaScript. Preview, build and check commands live
in [the website runbook](README.md#build-and-preview).

<a name="sources"></a>
## Shared sources

| Source | Responsibility |
| --- | --- |
| `assets/popover.html` | Shared document shell and explicit initialization slots |
| `assets/popover/popover.css` | App styles |
| `assets/popover/popover.js` | Sessions, settings, lamp details and per-agent usage views |
| `assets/popover/presentation.js` | Agent glyphs, token formatting and browser menu-bar readout |
| `assets/popover/document.js` | Browser/Node document assembly with required-slot validation |

<a name="hosts"></a>
## Host boundaries

The native host embeds the shell, styles and scripts at compile time in
`src/popover.rs`. It supplies `window.ipc.postMessage` and sends native state through
`window.__setState`. Error/confirmation callbacks keep their existing names.
The installed app needs no external files or HTTP server.

The landing host in `site/app-preview.js` loads those same sources and
provides sample state plus simulated IPC through the explicit host slot. Its
scripts-only iframe has no native services or access to the parent document.
Source, origin and per-load channel tokens authenticate both message directions.
Usage, dismissals and settings stay in memory in the preview.

<a name="tracking"></a>
## Tracking behavior

Tracking starts disabled. Enable **Track usage** in Settings to see separate Claude
and Codex quotas, opencode's token-only row, and the all-agent token total.
These are sample values. **Show usage in menu bar** hides only the status-bar
readout, preserving tracking and popover totals. Turning tracking off also hides
the readout and retains its display preference for the next enable.

<a name="packaging"></a>
## Static packaging and checks

`site/pages.py` packages the canonical homepage, guide, shared UI and pinned
browser dependencies into ignored `site/_pages/`. In source previews the landing
loads `../assets/`; the builder copies the native shell, renderer and tray icons
into `app-ui/` and remaps those paths, preserving the shared UI source bytes.
The artifact runs without the checkout. There is no separate prototype builder.

Rebuild after shared-source changes. Native view tests exercise the same assembled
views as the demo; Pages CI runs those tests when shared sources change. Run
`node --test site/*.test.cjs` and
`node --test tests/js/lamp-status.test.mjs tests/js/popover-library.test.mjs`.
See [the website runbook](README.md#check-the-exact-pages-artifact-locally) for the
production build and validation limits. Building does not publish the site.
