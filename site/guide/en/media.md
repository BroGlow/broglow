---
title: Images and photos
---

# Project images and photos

Use our own photos, screenshots, diagrams and CAD renders for project media.
Keep software copyright and license notices in distributed packages.

The website's renders and interactive 3D/AR preview come from our release
CAD and component measurements. Label renders as renders; they do not substitute
for physical-fit results. The app uses our geometric status-light icons.

Keep the entire `models/broglow/preview/` directory ignored and untracked, including
renders, dimension diagrams and render recipes. Keep selected app/site assets and
reviewed validation data in Git. Generate release diagrams under `dist/enclosure/`.

## 3D and AR preview

Keep the editable enclosure under `models/broglow/`. Export a presentation
snapshot to `site/assets/broglow.glb`; keep its source hashes and dimensions in
`site/assets/broglow-model.json`. The export must preserve the assembled colors,
use meters and Y-up coordinates, and place the feet on the ground for AR.
Update the snapshot when the CAD changes; website builds should
serve the checked export without rebuilding or modifying the enclosure.

The same model supplies the browser view and supported phone AR modes. Keep a
still-image fallback and identify it as a CAD render. Use the separate enclosure
package for printing. Export and website verification steps live in `site/README.md`.

## Prepare now

| Image | What to show |
| --- | --- |
| Working electronics | Our C3 board and all three LEDs on a desk, with the wiring visible. |
| Wiring close-up | Unpowered board, readable GPIO labels, resistors, LED polarity and shared GND. Include the USB connector for orientation. |
| App screenshot | Our current menu-bar popover with harmless example sessions and visible status. |
| Lamp controls screenshot | Our Lamps view showing a connected lamp and brightness control; hide private names and identifiers. |

## Finished assembly photos

| Image | What to show |
| --- | --- |
| Main product photo | Completed lamp from the front at a slight angle, on a clean desk, with the USB cable visible. Leave space around it for cropping. |
| Assembly photo | Open enclosure showing our board, LED mounting and wire routing before the cover is fitted. Disconnect power first. |
| Rear/USB detail | Finished connector opening, fitted cover and cable clearance. |
| Status sequence · optional | Same camera position for red, yellow, green and off; a short video is also useful. |

## Capture and handoff

- Phone originals are enough. Use soft daylight and a plain background; keep LEDs
  visible without blowing out their colors. Avoid filters and portrait-mode blur.
- Send original full-resolution photos, ideally at least 2000 px on the long edge;
  JPG, PNG or HEIC is fine. Do not send screenshots of photos or messenger-compressed copies.
- Capture app screenshots as PNG at native resolution. Use example task names and
  remove private prompts, file paths, account details and device identifiers.
- No need to crop or add text. Keep an uncropped original; prepare a landscape
  crop for the website and a square crop when integrating it.
- Note who took each photo and whether the hardware shown is a prototype or the
  final build. Remove location metadata from the web copy before publication.

Keep source originals outside the downloadable app bundle. Add only selected,
web-sized copies to `site/assets/` and list their source in `site/README.md`.
Use `site/assets/screenshots/` for newly captured project screenshots when available.

## Publication copies

Publish reviewed current assets. Review retired media in older Git commits before
making repository history public. Rebuild app packages so embedded icons and
manuals match the current source.
