# OpenKite E2E Testing

OpenKite is a **Dioxus 0.7 desktop app** (wry/tao WebKitGTK). There is no web
platform — browser automation (Playwright, Selenium) cannot attach, and
dioxus-desktop hardcodes WebKit's automation flag off. Two complementary E2E
layers cover the app:

| Layer | What it drives | Runner | Best for |
|---|---|---|---|
| **Xvfb + xdotool** (`run-desktop-e2e.sh`) | Real X11 input: keybinds, window, screenshots | bash | Launch smoke, pixel-level visual checks, connected-cluster run |
| **Xvfb user flows** (`user_flows.bats`, `flows_lib.bash`) | Real X11 input per user scenario | bats-core | Boot/visual/user flows, results in the GHA summary via dorny |

The DOM-selector bridge (OKT-64, PR #82) was **built then rejected**: an
env-gated localhost listener running `document::eval(JS)` gave real DOM
selectors from bats, but the bats+bridge CI job wedged on a 27-minute
persistent loop. DOM interaction coverage lives in **in-process headless
tests** instead (`tests/palette.rs` + friends, PR #81), which drive the
VirtualDom directly — milliseconds, no X server. These bats flows cover what
only a real boot can: window creation, X11 keybinds, rendering, screenshots.

### Why this shape (verified against upstream)

Dioxus-desktop 0.7.10 hardcodes WebKit automation OFF and owns the wry
`WebContext` privately (webview.rs:260) — Playwright cannot attach (its
WebKit is a standalone patched build), and WebDriver would require forking
dioxus-desktop. The Dioxus org tests its own desktop crate in-process via
`document::eval` + synthetic `dispatchEvent`, CSS selectors, zero
`data-testid` (`DioxusLabs/dioxus` `packages/desktop/headless_tests/`). Our
headless `tests/*.rs` mirror that mechanism from Rust; bats drives the real
binary's X11 surface.

---

## Writing a flow test (bats)

Add a `@test` block to `e2e/user_flows.bats`. `setup()` boots a fresh
Xvfb + openbox + app instance for every test (unique `:NN` display per
`BATS_TEST_NUMBER`); `teardown()` kills it. Shared helpers live in
`e2e/flows_lib.bash`:

| helper | purpose |
|---|---|
| `flow_setup` / `flow_teardown` | per-test app boot / teardown (called by bats `setup`/`teardown`) |
| `wait_for_window` | wait for the app window via `xdotool search` |
| `focus_window` | focus the app window (X11 keybinds need focus) |
| `shot <name>.png` | screenshot to `$ART/<name>.png` (needs `import`) |
| `assert_rendered <png> <label>` | non-blank screenshot (stddev > 0.01) |
| `assert_pixels_changed <pngA> <pngB> <min-diff> <label>` | pixel diff above threshold |
| `assert_pixels_unchanged <pngA> <pngB> <max-diff> <label>` | pixel diff below threshold |

```bash
@test "flow 03: palette filters the command list" {
  shot "01-home.png"
  assert_rendered "$ART/01-home.png" "home"

  focus_window
  xdotool key --clearmodifiers ctrl+p
  sleep 1
  xdotool type --delay 60 "go to workloads"
  sleep 1

  shot "02-filtered.png"
  assert_rendered "$ART/02-filtered.png" "filtered palette"

  # Escape to restore baseline.
  xdotool key --clearmodifiers Escape
  sleep 1
  shot "03-after-escape.png"
  assert_pixels_unchanged "$ART/01-home.png" "$ART/03-after-escape.png" 1000 "palette closed cleanly"
}
```

### Timing conventions (WebKitGTK)

- `sleep 1` after every input that triggers a Dioxus re-render; WebKit paint
  settle needs `sleep 3` after window creation.
- Palette typing uses `--delay 60` so the filter sees each keystroke.
- **One palette session per flow.** WebKitGTK autofocus does not re-grab on
  remount, so a second session's typing may never reach the filter.
  Multi-session round-trips live in the headless `tests/palette.rs` layer.

### Artifacts

Per-test screenshots + logs land in `$ART_ROOT/flow-NN/` (default
`e2e/artifacts-flows/`). The CI job uploads them as `user-flow-artifacts`
and publishes a dorny report to the job summary from
`--report-formatter junit` output. On failure, screenshots are the first
place to look — each `shot` names the state it captured.

### Running locally

```bash
# e2e/ directory, openkite binary built:
cargo build --workspace
cd e2e
BIN=../target/debug/openkite ART_ROOT=artifacts-flows bats user_flows.bats
```

---

## Bridge dispatch guard

`e2e/bridge-guard.sh` is a focused regression guard for the `/openkite`
dispatch path. It boots the app under Xvfb with the test-only plugin in
`e2e/fixtures/okt95-guard/` installed into an isolated `$HOME`, then asserts a
`register` POST returned a well-formed JSON envelope, the Dioxus-side
registration mirror wrote, and no panic reached `app.log`.

The no-panic check is the point: a runtime-mirror regression panics on the
tokio worker, which is contained (the surface looks healthy) and only visible
in `app.log`. The corresponding CI job is `Bridge dispatch guard` in `e2e.yml`.

```bash
cargo build --workspace
cd e2e
./bridge-guard.sh ../target/debug/openkite artifacts-bridge-guard
```

---

## Console parity gate (OKT-129)

The browser target and the desktop host render the same React console from the
same sources, but the only check that used to exist was
`web/test/parity.test.tsx`: it server-renders the tree and asserts DOM text and
class markers, so it cannot see a missing stylesheet, a wrong CSS scope or a
broken layout — anything that only shows up in pixels. (It really happened: the
browser console mounted into `#root` while every rule in `web/src/theme.css` is
scoped under `#openkite-react-spike-root`, so the preview rendered unstyled and
nothing noticed.)

The parity gate adds the two **real renders** and pixel-compares them. The SSR
marker check still runs first in the same job as a fast pre-check; it is not
replaced.

### What is compared

| Input | Produced by | Data it renders |
|---|---|---|
| browser | `e2e/parity/capture-browser.mjs` — headless Chromium over the served `web/dist`, 800×600 viewport | the console's bundled fixtures, because the static host answers `POST /openkite` without a bridge ([`web/src/bridge.ts`](../web/src/bridge.ts) transport failure ⇒ `fixtureCall`) |
| desktop | `e2e/parity/capture-desktop.sh` — the real binary under Xvfb with `OPENKITE_ROUTE`, an isolated `$HOME` (`menuBar = "hide"`) and `OPENKITE_CONSOLE_FIXTURES` | the export of those same fixtures, served by [`src/console_fixtures.rs`](../src/console_fixtures.rs) |

| surface | browser nav id | desktop `OPENKITE_ROUTE` | fixture content |
|---|---|---|---|
| `01-pods` | `pods` (default) | `/workloads` | 12 pods, namespace + health + restarts + controller columns, log dock |
| `02-overview` | `overview` | `/cluster` | count chips per counted kind, connection/health summary |
| `03-configmaps` | `configmaps` | `/config` | configmap list |

Those are the console-owned routes the desktop can be booted onto
deterministically (`src/router.rs` `console_route`). **Logs, terminal, plugin
views and the menu bar/window chrome are excluded**, each with the capability
that explains it, in `e2e/parity/parity-config.sh`. The browser capture reaches
its surface through `window.__openkite_react_console.setRoute` rather than by
clicking: at 800×600 the sidebar is an overlay, so a click would capture a
different shell state than the desktop renders.

`capture-browser.mjs` refuses to write anything unless the static host really is
static — it probes `POST /openkite` (a 2xx means a bridge host answered), then
asserts the fixture cluster context and fixture resource names are actually
painted. Without that, a run could quietly compare fixtures to fixtures and call
it parity.

### Tolerance, masks and the allow-list

`e2e/parity/compare.sh` runs three ImageMagick `compare -metric AE` passes per
surface against the committed baselines (the browser render is the reference):

| pass | cap | why |
|---|---|---|
| browser vs baseline | `BROWSER_SELF_MAX` (0.1 %) | the same renderer must reproduce its own baseline; above this the gate is measuring its own noise |
| desktop vs baseline | `MAX_DIFF_PIXELS` (1 %) | the gate: a real divergence between the two consoles lands here |
| desktop vs browser | same as above | the direct renderer-parity number, reported for diagnosis |

A cross-renderer comparison can never be an exact-pixel diff: Skia (Chromium) and
Cairo/WebKitGTK antialias text differently even with identical fonts, so
`-fuzz` (8 %) absorbs that and the pixel cap bounds the rest. The values in
`parity-config.sh` carry a `PARITY-CALIBRATION` note recording the measurement
that produced them — change them only with a new measurement, never to make a
run green.

Rules for the allow-list:

- every **exclusion** names the capability that explains it (no browser
  equivalent), and `compare.sh` fails if an entry has no reason;
- every **mask** (a rectangle painted out on all three images) carries
  `id:rect:reason:ticket`, and the total masked area is capped at 2 % of the
  frame. Masks are added only with a recorded measurement in the reason — never
  to silence a real difference;
- an unexplained difference **fails the job** and writes
  `<surface>.<a>-vs-<b>.diff.png` next to the captures, which are uploaded as
  the `console-parity-artifacts` artifact together with `compare.log` and both
  app/capture logs.

### Fixtures: one source of truth

`web/src/fixtures.ts` is the only source. `npm run export:parity-fixtures`
(which reuses the same `vite --ssr` mechanism as `test:parity`) writes
`e2e/parity/fixtures.json`, and that file is **committed**: the payloads the gate
compares are reviewable, and the job re-exports them with
`npm run export:parity-fixtures -- --check` so a stale copy fails instead of
silently comparing the wrong data. Fixture ages are built from the clock at
payload-build time, so the export records `exportedAt` and the host rebases every
timestamp onto its own clock (`src/console_fixtures.rs`) — otherwise the
committed copy would show `241m` where the browser shows `12m`.

### Refreshing the baselines

Baselines are the **browser** render (the browser is the target; the desktop
must match it). They live in `e2e/parity/baselines/` and refresh **in the same
PR as the visual change they follow**, with the diff image attached. A
standalone "make CI green" baseline commit is a silenced gate and must be
rejected in review.

```bash
# dispatch the browser-only capture job, then commit what it produced
gh workflow run e2e.yml -f capture_parity_baselines=true
gh run download <run-id> -n parity-baselines -D e2e/parity/baselines
git add e2e/parity/baselines && git commit -m "test(e2e): refresh console parity baselines (<why>)"
```

### Running it locally

Needs a Rust build of the desktop binary (CI does it), ImageMagick, Xvfb,
`xdotool`, `openbox`, and `npx playwright install --with-deps chromium` in `web/`
— all of which live in the workflow, never required on a dev host.

```bash
cd web && npm ci && npm run build:web
python3 -m http.server -d dist 8000 &                       # POST /openkite stays unanswerable
node ../e2e/parity/capture-browser.mjs http://127.0.0.1:8000 ../e2e/parity/out 800x600

cargo build --workspace
cd e2e && EXPECT_DIMS=800x600 ./parity/capture-desktop.sh ../target/debug/openkite parity/out
./parity/compare.sh parity/out parity/baselines
```

### Limits (deliberate)

- The gate compares **two renderers, not two data sources**: with the fixture
  hook both sides show the same payloads, so it can never catch a bridge/kube
  regression — that is `desktop-e2e-connected` and `bridge-guard` territory.
- Three surfaces only; the other nav ids are browser-reachable but not
  desktop-reachable without input automation (see the follow-ups in the OKT-129
  plan: a console deep-link per nav id).
- `OPENKITE_CONSOLE_FIXTURES` is an env-gated, no-op-when-unset test hook in
  product code, mirroring the accepted `OPENKITE_ROUTE` precedent. It logs
  `console fixtures: enabled (N kinds from <path>)` when active, and the desktop
  capture fails if that line is missing.

