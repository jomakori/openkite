# Browser-host visual baselines (OKT-191)

Visual regression gate for the **browser host** (`openkite-web`, OKT-137):
screenshots of the hydrated client, not the desktop wry/WebKitGTK webview that
`e2e/visual/capture.sh` covers.

Per OKT-191 Decision D1, this lane grades against the OpenDesign mockup
directly (see `docs/reference/opendesign-console/`), not against the desktop
baselines in `e2e/visual/baselines/`. OKT-192 is the parity gate that compares
*both* hosts against the mockup's own markup/tokens; this ticket only commits
the browser host's own pixel baselines and the gate that catches regressions
in them.

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

Four routes (the acceptance list — overlays and desktop-only routes like
`/logs`/`/terminal` are out of scope for this ticket) at three breakpoints:

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
built-in `WebSocket` — same no-dependency approach as `e2e/browser/harness.mjs`
(OKT-188). No Playwright, no browser automation package is installed. It:

1. Boots the real `openkite-web` binary (`OPENKITE_WEB_BIN`) against the
   loopback fixture kubeconfig in `e2e/browser/fixtures/kubeconfig` — the same
   fixture the existing browser E2E harness uses, so every surface renders in
   the disconnected/empty-state the fixture produces (consistent with the
   desktop capture's documented "no kubeconfig" convention).
2. Launches a real headless Chromium (`OPENKITE_BROWSER`) with remote
   debugging, for each of the three viewports navigates to each of the four
   routes, waits for hydration, and screenshots over CDP's
   `Page.captureScreenshot`.
3. Fails loudly (non-zero exit) on any missing binary, missing wasm asset, or
   a JS exception/console error surfaced during the run — it does not degrade
   to a partial or fabricated capture.

```bash
OPENKITE_WEB_BIN=/path/to/openkite-web \
OPENKITE_WEB_ROOT=/path/to/hydration-assets \
OPENKITE_BROWSER=/path/to/chromium \
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

Unlike the desktop lane's fixed `MAX_DIFF_PIXELS` (one capture size, 800x600),
this lane's three viewports are very different sizes, so `run.sh` sets
`compare.sh`'s per-image `MAX_DIFF_PERCENT` (default `1.5`, i.e. 1.5% of each
baseline's own pixel count) instead. A change that alters what a user sees in
the browser host refreshes `baselines/` in the same PR — a moved baseline is a
decision, not an accident, same convention as the desktop lane.

## Provenance of the committed baselines

Captured by actually running `capture.mjs` against a locally built debug
`openkite-web` + release wasm hydration client, driven by the Hermes-managed
headless Chromium-for-Testing binary at
`/opt/hermes/.playwright/chromium_headless_shell-1243/...` (ARM64 dev
sandbox, 2026-10-13). They are real pixels from a real hydrated run, not
hand-written or copied from the desktop/mockup baselines.

They were **not** captured on the GitHub Actions runner this gate will run
on. Font rasterization differs slightly across environments (the desktop
lane's `compare.sh` comment documents the same effect measured on 2026-09-30),
so the first CI run of `visual-regression-browser` may need its baselines
refreshed from a CI-run capture even though no UI changed. That refresh, if
needed, is a known follow-up — see the PR's "Could not verify" note.
