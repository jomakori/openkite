# Browser-host visual baselines

Visual regression gate for the **browser host** (`openkite-web`): screenshots of
the hydrated client, not the desktop wry/WebKitGTK webview that
`e2e/visual/capture.sh` covers.

This lane grades against its own committed baselines, not the desktop set in
`e2e/visual/baselines/`.

## Why hydrated, not SSR

`crates/openkite-web/src/ssr.rs` serves a server-rendered document that the
wasm-bindgen client then hydrates. A screenshot taken before hydration mounts
would grade markup the mockup's rendered surfaces never show a user — anything
the client paints only after mount (interactive controls, client-side state)
would be silently absent. `capture.mjs` waits for both the `[data-surface="app"]`
root to exist **and** the `openkite-web-client_bg.wasm` resource to have a
non-zero `responseEnd` before it screenshots, so every baseline here is the
real post-hydration DOM.

## What's captured

Four routes at three breakpoints; overlays and desktop-only routes (`/logs`,
`/terminal`) are out of scope:

| Route | Baseline slug |
|---|---|
| `/` | `home` |
| `/cluster` | `cluster` |
| `/workloads` | `workloads` |
| `/config` | `config` |

| Viewport | Slug |
|---|---|
| 1440x900 | `1440x900` |
| 834x1112 | `834x1112` |
| 390x844 | `390x844` |

12 baselines total, named `<route>-<viewport>.png` in `baselines/`.

## How it works

`capture.mjs` drives Chrome DevTools Protocol directly over Node 22+'s
built-in `WebSocket` — the same no-dependency approach as
`e2e/browser/harness.mjs`. No Playwright, no browser automation package is
installed. It:

1. Boots the real `openkite-web` binary (`OPENKITE_WEB_BIN`) with the
   credential-less environment, so the host serves the fixture gateway and
   every surface renders its disconnected/empty state.
2. Launches a headless Chrome (`OPENKITE_BROWSER`) with remote debugging, then
   for each of the three viewports navigates to each of the four routes, waits
   for hydration, and screenshots over CDP's `Page.captureScreenshot`.
3. Fails loudly (non-zero exit) on any missing binary, missing wasm asset, or
   a JS exception/console error surfaced during the run — it does not degrade
   to a partial or fabricated capture.

```bash
OPENKITE_WEB_BIN=/path/to/openkite-web \
OPENKITE_WEB_ROOT=/path/to/hydration-assets \
OPENKITE_BROWSER=/path/to/chrome \
node e2e/visual/browser/capture.mjs <output-dir>
```

`OPENKITE_WEB_ROOT` must contain `openkite-web-client.js` and
`openkite-web-client_bg.wasm` — the wasm-bindgen output, built the same way
the production Dockerfile builds it (`--target web --no-typescript`).

## Running the gate

`run.sh` captures a fresh set and compares it against the committed baselines
with `../compare.sh` (ImageMagick):

```bash
OPENKITE_WEB_BIN=/path/to/openkite-web \
OPENKITE_WEB_ROOT=/path/to/hydration-assets \
./run.sh baselines artifacts/visual-browser
```

The three viewports differ in size, so `run.sh` sets `compare.sh`'s per-image
`MAX_DIFF_PERCENT` (default `1.5`, i.e. 1.5% of each baseline's own pixel count)
instead of the fixed pixel budget the desktop lane uses.

Baselines are the captures this gate produces. A change that alters what a user
sees in the browser host refreshes `baselines/` in the same PR — a moved
baseline is a decision, not an accident, same convention as the desktop lane.
