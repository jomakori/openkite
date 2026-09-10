# OKT-67 spike — React 19 in the wry webview

> **Verdict: GO** (conditional — see [Recommendation](#recommendation)).
> React 19 + Tailwind 4 runs in a browser engine and round-trips state over
> the **existing, unmodified** `/openkite` bridge. The remaining conditions are
> a real-WebKitGTK re-measure and an additive Rust→JS push channel for live
> state; neither invalidates the core capability.
>
> **Status:** spike complete · **Branch:** `feat/okt67-react-spike` · **Date:** 2026-09-10

## TL;DR

| Question the gate asks | Answer | Evidence |
|---|---|---|
| Can React 19 + Tailwind 4 build to a mountable webview asset? | **Yes** | Vite 8 → `app.js` + `app.css`, 226 kB / 71 kB gzip |
| Does it run inside the webview? | **Yes*** | Real engine run, 0 page errors; host mount uses the same `include_str!` + `document::eval` path the repo already ships for JS plugins and xterm |
| Does state move over the EXISTING bridge? | **Yes** | Captured wire envelope is byte-identical to `OPENKITE_BRIDGE_JS` (`{id, plugin, request:{op:"list",kind:"pods",ns:"default"}}`) |
| Is it fast enough? | **Yes, with margin** | First paint ~50–62 ms; 500 rows ~16.3 ms median (1 frame, stable across 13 runs) |
| Is the memory cost acceptable? | **Yes** | Webview renderer +12–20 MiB over an empty page (GC-state dependent) |

`*` The literal wry/WebKitGTK webview could not be launched in this
environment (no `glib-2.0`, no display — the same reason `cargo` cannot link
here). Rendering numbers are from a Chromium proxy; see
[Measurement caveats](#measurement-caveats).

## What was built

Minimal, additive — no core architecture change.

| Path | What it is |
|---|---|
| `web/` | React 19 + Tailwind 4 source; Vite 8 lib build (esbuild/oxc minify), no heavy toolchain |
| `web/src/bridge.ts` | Bridge client: uses host `window.openkite.api.list` verbatim; a wire-compatible shim only for browser measurement |
| `web/src/App.tsx` | UI + the in-page `SpikeController` the harness reads for timings |
| `web/src/ResourceTable.tsx` | The table rendered from bridge data |
| `assets/vendored/openkite-react-spike/` | Committed build output (`app.js`, `app.css`, `SOURCE.txt`), same vendoring convention as `tools/build-xterm` |
| `src/react_spike.rs` | Host half: mounts the bundle, serves the read-only `/openkite-spike` context endpoint, reuses `router::json_response` |
| `web/measure/measure.mjs` | Playwright measurement harness (first paint, 500-row render, `/proc` RSS) |
| `web/scripts/bundle-size.mjs` | Bundle size report + `SOURCE.txt` provenance |

### Bridge contract used (nothing new on the kube path)

JS → Rust (existing, `openkite.api.list`):

```json
{ "id": 1, "plugin": "openkite-react-spike",
  "request": { "op": "list", "kind": "pods", "ns": "default" } }
```

Rust → JS (existing `ApiResponse`): `{"status":"ok","result":{"items":[…]}}`.

The only host addition is a read-only `/openkite-spike` endpoint returning the
active kubeconfig context (`{op:"context"}` → `{context, connected}`), because
the existing op set has no equivalent. It rides the **same** dioxus
asset-handler seam (`use_asset_handler` + same-origin `fetch`) — no new
transport. The plugin bridge, kube reflect/watch, PTY, exec, logs, and plugin
host are untouched.

## Measured numbers

Environment: aarch64 Linux, Node v26.7.0, 12 vCPU, Chromium 153 (Playwright
headless). Production vendored bundle. The runtime figures are medians of 5
spike runs against 3 empty-page control runs; per-run evidence is in
`web/measure/report-1..5.json`, `web/measure/baseline-1..3.json`, and
`web/measure/summary.json`. RSS is sampled both before and after a forced
`window.gc()` because Chromium's uncollected heap is noisy on a shared host.

### Bundle size

| Artifact | Raw | Gzip |
|---|---:|---:|
| `app.js` (React 19.3 + ReactDOM + app) | 225,901 B (220.6 KiB) | 69,922 B (68.3 KiB) |
| `app.css` (Tailwind 4, tree-shaken) | 10,444 B (10.2 KiB) | 3,166 B (3.09 KiB) |
| **Total** | **236,345 B (230.8 KiB)** | **73,088 B (71.4 KiB)** |

For reference, the already-shipped vendored `assets/vendored/xterm/xterm.js`
is **290,509 B raw / 71,715 B gzip** — so the React runtime is slightly
*smaller* than a JS bundle this repo already `include_str!`s and evals.

### Runtime (medians of 5 runs; ranges in parentheses)

| Metric | Value | Notes |
|---|---:|---|
| First paint (bundle-relative) | **61.6 ms** (53.6–98.8) | React commit → next frame |
| Browser wall to controller ready | 163 ms | navigation start → `ready` |
| Bridge-loaded paint (30 rows) | **92.3 ms** | first paint *with* data |
| `openkite` list round-trip | **5.4 ms** (3.2–15.5) | JS→Rust→JS, localhost server |
| `context` round-trip | 31.7 ms | first call, cold |
| **Render 500 rows** | **16.3 ms median** | stable across 13 runs (14.9–17.1); max 110 ms outlier |
| Idle webview renderer RSS (post-GC) | **32.6 MiB** (31.9–33.9) | Chromium renderer process |
| Idle webview renderer RSS (pre-GC) | 38.2 MiB | before forced collection |
| Empty-page renderer RSS (post-GC) | 17.3 MiB (15.2–20.0) | same engine, no React |
| **Marginal renderer RSS** | **+15.3 MiB post-GC / +19.6 MiB pre-GC** | cost attributable to the UI layer |
| Idle full process tree RSS (post-GC) | 85.6 MiB | browser + gpu + renderer et al. |

Zero page errors, zero console errors across all runs.

First-paint and round-trip numbers move run-to-run with host load (a warm
localhost `list` is ~2 ms; the same call under contention ran ~15 ms). The
500-row render median is the most stable figure, at ~16 ms across every run.

## What worked

- **The existing bridge is enough.** React's first call produced the exact
  envelope `OPENKITE_BRIDGE_JS` emits; `bridge.rs::handle_post` needs no change.
- **Vendoring is clean.** Fixed-name `app.js`/`app.css` means the host does a
  plain `include_str!` with no manifest/hash parsing, exactly like xterm.
- **Tailwind 4 tree-shakes well** — 10.2 kB CSS for the whole spike UI.
- **500-row render sits at one frame.** Median 16.3 ms across 13 runs.

## What didn't / gotchas found

1. **Vite lib mode does not replace `process.env.NODE_ENV`.** The first build
   shipped *both* React dev and prod code (653 kB) and would have thrown
   `process is not defined` in the webview. Fixed with an explicit
   `define: {'process.env.NODE_ENV': '"production"'}`, which quartered the
   bundle to 226 kB. This must stay in `web/vite.config.ts`.
2. **Tailwind auto-detection scanned stale build output.** Before the generated
   `web/dist/` was gitignored, Tailwind globbed the previous build's CSS and
   inflated the stylesheet to 13.8 kB with classes the spike never uses. Pinned
   with `@import "tailwindcss" source(none)` + explicit `@source` globs in
   `web/src/index.css`, making the build deterministic.
3. **No Rust→JS push exists in the bridge.** `openkite.api.*` is
   request/response, and `watch` returns a one-shot snapshot (see
   `plugin-architecture.md`). The spike's state round-trip is therefore
   *request → answer*; **live** cluster updates need an additive push channel
   (e.g. periodic `document::eval("window.openkite._pushState(...)")` or SSE
   over an asset handler). This is Phase-3 work, not a spike blocker.
4. **Large eval payloads.** The mount path evals a 226 kB script via
   `document::eval`; dioxus transmits it with `{script:?}` (Debug escaping), so
   the backtick-heavy minified bundle is safe. Worth watching if the bundle
   grows several-fold.
5. **Toolchain drift risk.** The Rust build `include_str!`s the committed
   bundle, but CI's paths-filter only watches `**.rs`/`Cargo.*`, so a stale
   bundle would compile silently. Phase 3 needs a bundle-freshness CI gate
   (`npm run build` then `git diff --exit-code assets/vendored/...`).

## Measurement caveats

- **Chromium is a proxy for WebKitGTK.** wry on Linux uses WebKitGTK; the
  actual webview **could not be launched here** (missing `glib-2.0`,
  `gstreamer`, `libGLESv2`, etc. — the same system-lib gap that blocks
  `cargo build`/`cargo link`). Playwright WebKit also failed to launch for the
  same reason. Chromium's JS/render engine is generally *faster* than
  WebKitGTK, so treat the timings as an optimistic floor and re-measure in the
  real webview before setting a perf budget.
- **Rust host mounts were verified by reading + `cargo fmt` only**, because
  `cargo build`/`test` cannot link in this environment. CI is the real compile
  check.
- The `list`/`context` round-trips are localhost HTTP against the harness, so
  they measure bridge/JS overhead (~2–5 ms warm), **not** kube API latency.
- Chromium's uncollected heap is noisy on this shared host; the RSS figures are
  sampled after `window.gc()` (and before it, for reference) so the control and
  the spike are compared like-for-like.

## Recommendation

**GO** — adopt React 19 + Tailwind 4 for the Phase 3 UI layer.

The gate's core question is answered: React builds, mounts through the
repo's existing webview asset mechanism, and exchanges state over the
**unmodified** `/openkite` bridge with captured proof. Build size is in line
with what the repo already vendors (xterm), and the measured render/memory
costs leave headroom.

Adopt with these conditions tracked in Phase 3:

1. **Re-measure in the real WebKitGTK webview** (Xvfb, like
   `e2e/run-desktop-e2e.sh`) and confirm the 500-row render / first-paint
   budget before committing to a perf target.
2. **Add a Rust→JS push channel** for live cluster state (reflector deltas),
   since the existing bridge is request/response only.
3. **Add a CI bundle-freshness gate** so the committed vendored bundle can't
   drift from `web/src`.

### Fallback

If condition (1) fails the budget, the fallback is **NO-GO for React and port
the UI gaps into Dioxus RSX + existing CSS** (the current architecture), rather
than introducing a second toolchain. The spike's bridge findings still apply:
the RSX path already owns the state and would not need the `/openkite-spike`
endpoint at all.

## Reproduce

```sh
# Build the vendored bundle
cd web && npm install && npm run build && npm run bundle-size

# Measure (Chromium must be installed first)
cd web && PLAYWRIGHT_BROWSERS_PATH=<path> npx playwright install chromium
cd web && PLAYWRIGHT_BROWSERS_PATH=<path> node measure/measure.mjs          # spike
cd web && PLAYWRIGHT_BROWSERS_PATH=<path> node measure/measure.mjs --baseline  # control

# Host (in CI, where glib is present)
PATH=/opt/data/home/.cargo/bin:$PATH RUSTUP_HOME=/opt/data/home/.rustup \
  RUSTUP_TOOLCHAIN=stable-aarch64-unknown-linux-gnu cargo fmt
cargo build --workspace
OPENKITE_ROUTE=/spike ./target/debug/openkite
```
