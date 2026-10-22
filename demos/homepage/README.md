[English](README.md) | [Русский](README.ru.md)

<a name="homepage-demo"></a>
# BroGlow homepage demo

The Ember homepage prototype lives here; the production homepage in `site/` is
unchanged. Run commands below from the repository root. Node.js and Python 3
are sufficient; no npm dependencies are required.

<a name="preview"></a>
## Preview

```sh
python3 -m http.server 8080 --bind 127.0.0.1
```

Open <http://127.0.0.1:8080/demos/homepage/>. Serve the repository root because the
source page reads shared app assets from `assets/` and product assets from `site/`.
Opening `index.html` with `file://` does not work.

<a name="build-and-check"></a>
## Build and check

```sh
node --test demos/homepage/*.test.cjs
node --test tests/js/lamp-status.test.mjs tests/js/popover-library.test.mjs
node demos/homepage/build.cjs
```

The default output is ignored `dist/homepage-demo/`. With the same server running,
open <http://127.0.0.1:8080/dist/homepage-demo/>. This standalone directory includes
the shared UI, icons, photos, GLB and pinned browser libraries; it can be served
without the checkout. Rebuild after changing source files. The builder preserves
unknown files and refuses an unrelated nonempty output directory.

<a name="review-scope"></a>
## Review scope

Try EN/RU, System/Light/Dark, Working/Paused/Needs you, and **Settings → Track usage**.
The demo uses the production popover with synthetic sessions and a simulated native
host. Settings and dismissals stay in memory; it cannot read real sessions, install
software or control hardware. See [the shared UI contract](SHARED-UI.md).

Normal page scrolling drives the 3D lamp, with manual state/brightness controls,
reduced-motion support and photo fallback. Photos retain their 7:5 proportions.
Native phone AR needs a compatible browser and usually HTTPS; loopback previews
show handoff guidance. Social crawlers receive static English metadata; the browser
supports EN/RU. Physical phone AR and live social scraping need separate validation.

The [viewport harness](viewport-check.html) checks narrow and desktop layouts.
The [experience comparison](comparison.html) records the old/new feature comparison
and three independent Lazarus reviews. Tests cover shared UI boundaries, settings,
tracking, terminal/window interaction, localization, assets, 3D/AR and social tags.
Installed services, installers and physical hardware are outside these checks.

<a name="references"></a>
## References

[Public-source research](REFERENCE-SOURCE.md) preserves the dated Blink Buddy
findings. No reference-site code, media, fonts or branding are demo dependencies.
BroGlow's product sources are the [README](../../README.md),
[installation guide](../../site/guide/en/installation.md),
[session guide](../../site/guide/en/sessions.md) and [media policy](../../MEDIA.md).
This demo neither replaces `site/index.html` nor publishes it. The former
`outputs/broglow-homepage-demo/dist/` preview is local generated evidence, not source.
