# OKT-197 — mobile parity close-out evidence

Captured with the browser gate's own harness
(`e2e/visual/browser/capture.mjs`, headless Chromium at device scale 1) against
the console served by `openkite-web` with the hydrated wasm client.

| File | What it shows |
| --- | --- |
| `before-home-390.png` | Home at 390×844 before this change (four-cell bar, no log sheet) |
| `after-home-390.png` | Home at 390×844 after this change (five-cell bar with icons and a Logs cell) |
| `bar-before-after.png` | The two bars stacked, cropped to the bottom 104px (before on top) |
| `log-sheet-open-390.png` | The log sheet opened from the Logs cell, with its 44px drag handle |

Measured in the same run (`Runtime.evaluate` against the hydrated page):

- 390×844 — bar `display: block`, height 67px, five cells at 78×66px, 19px icons, `scrollWidth - clientWidth = 0`.
- 320×568 — same bar, cells at 64×66px (still above the 44px touch floor), `scrollWidth - clientWidth = 0`.
- Log sheet — opens to `top: 336px`, `height: 430px` at 390px wide and
  `top: 161px`, `height: 329px` at 320px wide; handle is 44px tall and reports
  `aria-expanded="true"`. Parked, its computed `visibility` is `hidden` and it
  exposes zero focusable descendants.

`before-home-390.png` is the committed baseline from before the change, so the
two frames come from the same harness and viewport; the remaining diff between
them is the bar (1.50% of pixels, `compare -metric AE`), against 0.36–0.76% of
renderer noise on the unchanged 1440×900 and 834×1112 viewports.
