[English](REFERENCE-SOURCE.md) | [Русский](REFERENCE-SOURCE.ru.md)

<a name="reference-source"></a>
# Blink Buddy: public-source reverse engineering

Inspected on **2026-10-21**. Primary reference: <https://blink-buddy.com/>.

This report describes the public homepage and its shipped JavaScript/CSS. It distinguishes implemented website behavior from product claims; the hardware, firmware and desktop app were not tested.

<a name="sources-method"></a>
## Sources and method

The public HTML, CSS and three JavaScript modules were downloaded for **text inspection only** into `/private/tmp/broglow-blink-reference/`. No downloaded JavaScript was executed by this source-inspection task. The first sandboxed request failed DNS resolution; an authorized read-only request outside the network sandbox succeeded. Source was therefore available.

| Source | What it establishes |
|---|---|
| [Homepage HTML](https://blink-buddy.com/) | Page sections, copy, image descriptions, seven reel captions, setup controls, FAQ and links |
| [app.js](https://blink-buddy.com/app.js) | Scroll progress, stop selection, caption synchronization, provider switch, setup animation and input handling |
| [viewer.js](https://blink-buddy.com/viewer.js) | Three.js WebGL, procedural case geometry, live screen texture, orbit/zoom, drag and keyboard handling |
| [device.js](https://blink-buddy.com/device.js) | Canvas screen renderer, usage dials, statuses, boot face and settings/update/connecting views |
| [styles.css](https://blink-buddy.com/styles.css) | Sticky scenes, native scroll snap, typography, transitions, responsive and reduced-motion rules |

Line numbers below refer to the downloaded snapshots. Remote files can change.

<a name="homepage-sections"></a>
## What the homepage shows

| Section | Visitor sees / can do |
|---|---|
| Fixed navigation | BLINK wordmark, install anchor, pre-order link; cart appears when populated |
| Hero | “Your AI usage. At a glance.”, concise Claude Code/Codex value proposition, two overlapping product photographs, pre-order CTA |
| Interactive product reel | A live 3D product, seven scroll-controlled explanations, camera close-ups, Claude Code/Codex switch, drag-to-turn and keyboard turning |
| Setup | Three steps beside a live canvas preview: plug in → run installer → dials appear. OS tabs, Windows shell tabs, copyable install/status commands, manual-install details |
| Box / requirements | To-scale desk/monitor SVG, interactive 24″/27″/34″ monitor selector, dimensions, box contents, compatibility and limitations |
| Stand instructions | Four product photographs showing folded stand → slide down → swing out → stand upright |
| FAQ | Ordinary native disclosure controls; answers cover plan usage, supported apps, status lights, sleep, privacy, USB and uninstall |
| Company editions | Factory-set corporate boot logo/animation, asset requirements, 320×240 preview and batch inquiry CTA |
| Closing CTA / footer | Both case photographs, pre-order link, November 2026 dispatch/Israel-first wording, legal/support links and independent maker details |

The visual system is a dark product studio: charcoal `#0E0F13`, warm off-white text, orange hardware, blue alternate case, restrained hairlines and radial stage light. IBM Plex Sans/Mono are self-hosted. Hero photographs bob independently, respond slightly to a fine pointer, and move/scale subtly with scroll. These are separate DOM transform layers, not the 3D reel.

<a name="scroll-beats"></a>
## The seven scroll beats

These are actual `<article class="reel__cap">` nodes in homepage HTML, lines 188–284. Some historical source comments still say “six”; the DOM contains **seven** and the running code counts it.

| Beat | Visible explanation / screen | Camera focus `(x,y,zoom)` |
|---|---|---|
| 1 | **It wakes up and starts counting.** Full object; two eyes blink, then usage dials appear | `(0,0,1)` |
| 2 | **Your five hours.** Left usage dial, percentage and rollover countdown | `(-16.5,3.3,0.44)` |
| 3 | **Your week.** Right weekly dial and time remaining | `(16.5,3.3,0.44)` |
| 4 | **Every session, side by side.** Zoom into session marks; sessions arrive/change state and a project hint names the session requiring attention | `(-18.6,16.5,0.5)` |
| 5 | **And one light for the numbers.** Right-hand freshness light shown amber; “Reading is old – showing last known” | `(18.6,16.5,0.5)` |
| 6 | **One device, both tools.** Bottom provider pill / page indicator | `(0,-19.4,0.42)` |
| 7 | **Swipe for the rest.** Settings screen: brightness, version and available update | `(0,0,0.72)` |

Important distinction: the website's reachable provider control sits **below the 3D model**. The model canvas handles turning. Its drawn brightness buttons, chevrons and provider pill are presentation pixels; the inspected viewer does not implement corresponding hit targets. The text describes the physical device's controls.

<a name="scroll-mechanism"></a>
## How the scroll mechanism actually works

<a name="sticky-viewport"></a>
### 1. Native document scrolling with a sticky viewport

The model and caption live together inside `.reel__pin`, which is `position: sticky; top: 0; height: 100svh`. A separate `.reel__track` supplies the long section height (`styles.css` 530–544). `100svh` keeps the pin stable when mobile browser chrome expands/collapses.

`app.js` reads the number of captions from the DOM and writes `--stops`; the track height is `count × --stop`. Desktop `--stop` is `115vh`; the narrow-screen rule changes it to `45svh`. There is no GSAP/ScrollTrigger dependency in the inspected source: its name appears only in explanatory comments.

<a name="native-snapping"></a>
### 2. Native snapping supplies the stops

The page uses `html { scroll-snap-type: y proximity; }`. Reel/setup top edges and hidden `.snap` marker elements have `scroll-snap-align: start` and `scroll-snap-stop: always` (`styles.css` 3035–3142).

The first stop is the section's top. JavaScript creates the remaining markers at fractions `i / count`. Their positions use the same section span as the progress calculation. Native scroll snapping encourages each gesture to settle at a meaningful state. It is proximity snapping, so behavior still depends on the browser and input; “one gesture, one stop” is an intended result, not a custom wheel-event lock.

<a name="scroll-progress"></a>
### 3. JavaScript reads progress, chooses the nearest stop

A passive `scroll` listener batches updates in `requestAnimationFrame` (`app.js` 638–689). Progress is:

```text
t = clamp(-sectionRect.top / (sectionRect.height - stickyPin.offsetHeight), 0, 1)
```

The sticky element's measured height is used instead of dynamic `innerHeight`, avoiding mobile URL-bar movement changing the logical step. Caption boundaries are **midpoints between snap stops**, so the caption and browser snapping agree (`stopAt`, `app.js` 523–562).

The body still turns continuously across the section:

```text
azimuth = -0.95 + t × 1.25
elevation = 0.30 - t × 0.28
```

<a name="caption-camera-screen"></a>
### 4. A caption change also changes aim, zoom and screen state

`showCap()` reads `data-focus`, `data-screen`, `data-session`, status and hint attributes from the active article. The camera aims at that physical part of the model while the screen texture gets repainted (`app.js` 381–497).

Outgoing words fade/slide left in 100 ms; new words begin at 100 ms and finish their slide by 360 ms. The screen changes at that same 100 ms crossover. Caption changes have a 380 ms minimum dwell; a large jump is bounded to the penultimate and target captions instead of playing a long backlog. Invisible captions are `inert` and `aria-hidden`, rather than merely transparent.

<a name="live-rendering"></a>
### 5. The product is rendered live, not a video or frame sequence

`viewer.js` imports local **Three.js** (`./vendor/three.module.min.js`) and uses an alpha/antialiased WebGL renderer with a perspective camera, lights, materials and procedural extrusion geometry. The orange case and blue six-lobed case are constructed in code from measurements. No `.glb`, `.gltf`, pre-rendered video or image-sequence loader appears in this viewer.

`device.js` draws the screen in logical **320×240** coordinates. `viewer.js` renders that at 4× resolution (**1280×960**) to a Canvas 2D texture and maps it onto the 3D screen. The setup section uses the **same screen renderer** on a flat canvas; model and setup do not need independent screen artwork.

Camera angles ease at factor `0.12` per rendered frame; aim/zoom use `0.10`. Drag input adds a temporary angular offset, which decays by `0.928` each frame after release so the model returns to the scroll pose. Touch turning requires at least 6 px of predominantly horizontal movement; vertical touch scrolling stays available. Left/right arrow keys also turn it (`viewer.js` 411–539).

<a name="autonomous-animation"></a>
### 6. Small autonomous animations make the model feel alive

- First-stop wake face lasts **2.1 seconds** and replays when the viewer returns to the first stop.
- The demo session dial increases by **0.004 every 900 ms**, wrapping from above 0.97 to 0.06. This is scripted sample data, not the visitor's real account usage.
- Session marks run a **1-second-per-frame**, nine-frame script: `r|rr|rrr|rrf|rwf|rw|rr|xr|r` (running/finished/waiting/failed).
- Changing provider cross-fades/scales between the genuinely different case geometries over **340 ms**.
- Setup “Connecting” presents two boot stages at **2.4 seconds each**, with a 600 ms out/600 ms back pulse, then holds “Ready”.

Weekly demo usage is also synthetic: `setSession(p)` computes it as `clamp(0.50 + p × 0.44)`, rather than fetching a live account. Renderer support for an update-download view exists, but the seven homepage reel beats display usage and settings; the setup uses connecting.

<a name="fallbacks-performance"></a>
### 7. Fallbacks and performance are part of the effect

The viewer renders only while it intersects the viewport and the tab is visible. A dirty flag avoids re-rendering an unchanged 3D scene; session-mark changes repaint only once a second. Pixel ratio is capped at 2. WebGL failure/context loss replaces the viewer with a static product photograph.

`prefers-reduced-motion` is observed live. CSS unpins the animated sections and displays their text together, stops snapping those sections and removes decorative movement. Setup also unpins whenever its measured content cannot fit in the viewport; `ResizeObserver` and font-ready callbacks recheck the fit. These fallback details avoid trapping content below a pinned viewport.

<a name="product-claims"></a>
## Product features claimed on the page

These are **website claims**, not independently tested hardware behavior.

| Area | Claimed behavior |
|---|---|
| Usage | Five-hour/session and seven-day/weekly percentages with reset countdowns. Gauge colours are green below 60%, amber from 60% to below 90%, red from 90%. |
| Work state | One mark per session: running green, finished/unread amber, waiting/failed red. Project hint identifies attention needed. Explicit tool errors are reported; silence alone is not classified as failure. |
| Reading health | Separate live/stale/error indicator; stale data stays visible with an explicit warning. |
| Providers | Claude Code and Codex tracked together, each with its own view; two physical enclosures run the same software. |
| Controls | Provider pill, brightness, version/update control, boot animation; integrated folding stand. |
| Installation | USB cable supplies power and readings; macOS Apple silicon/Intel, x86-64 Linux and Windows installers; login startup, automatic board discovery, uninstall/status commands. |
| Compatibility | Claude Code 2.1.100+, terminal/VS Code/JetBrains; current Codex. Claude Desktop on macOS/Windows has slower refresh and no per-session marks. Browser-only claude.ai/ChatGPT is unsupported. |
| Sleep | Device sleeps about 30 seconds after its computer sleeps; tapping shows last figures for ten seconds, then returns to sleep. |
| Privacy | USB connection, no Wi-Fi setup, no login; page says figures are read locally and credentials/data are not sent elsewhere. |
| Commercial | Device/cable only; no AI subscription or usage included. Pre-orders advertise November 2026, Israel first. Batch corporate branding is factory-set. |

<a name="broglow-takeaways"></a>
## What is useful for BroGlow

Adapt the **explanation structure**: one real product, a short progression of meaningful states, camera/light changes that demonstrate each state, and nearby copy explaining exactly what changed. BroGlow's working/paused/needs-input colours should drive the narrative, rather than borrowing usage dials it does not offer.

A local BroGlow implementation can reuse its existing 3D asset/renderer, with native sticky layout, small scroll-state code and a separate interactive state selector. Keep the source reference out of the demo's dependencies. It should have its own copy, composition and graphics, plus reduced-motion and WebGL fallback paths.

The recommendations above were recorded on 2026-10-21. The current demo uses normal
document flow rather than a pinned scroll section and reuses the shared app UI.
See [the demo README](README.md) and [shared UI contract](SHARED-UI.md) for its current
implementation and tracking scope.

<a name="limits"></a>
## Limits

Only the homepage and its directly relevant modules/styles were source-inspected. Checkout/payment, analytics/consent/gate scripts, installer binaries, firmware and product delivery claims were not audited. Visual/browser verification is recorded separately. Historical comments sometimes describe removed behavior, so the findings above prioritize actual markup and executable statements over comments.
