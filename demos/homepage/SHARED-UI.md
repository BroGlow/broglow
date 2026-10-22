[English](SHARED-UI.md) | [Русский](SHARED-UI.ru.md)

<a name="shared-ui"></a>
# One app interface, two hosts

The desktop app and landing demo use the same UI sources in `assets/popover/`.
There is no Vue or React dependency and no second recreation of the popover.
The landing is static HTML/CSS/JavaScript. Preview, build and check commands live
in [the demo README](README.md#build-and-check).

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

The landing host in `demos/homepage/app-preview.js` loads those same sources and
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

The builder copies the shared UI and its pinned dependencies into the ignored
standalone output; runtime paths no longer depend on the source checkout.
Rebuild after shared-source changes. The native view tests exercise the same
assembled views as the demo; Pages CI runs those tests when shared sources change.
See [the README](README.md) for commands and validation limits. Building the demo
does not replace `site/index.html` or publish a site.
