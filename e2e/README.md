# OpenKite E2E Testing

OpenKite is a **Dioxus 0.7 desktop app** (wry/tao WebKitGTK). There is no web
platform — browser automation (Playwright, Selenium) cannot attach, and
dioxus-desktop hardcodes WebKit's automation flag off. Two complementary E2E
layers cover the app:

| Layer | What it drives | Runner | Best for |
|---|---|---|---|
| **Xvfb + xdotool** (`run-desktop-e2e.sh`, `run-flows.sh`) | Real X11 input: keybinds, window, screenshots | bash | Launch smoke, pixel-level visual checks, connected-cluster run |
| **DOM bridge** (`bridge_flows.bats`, `src/test_bridge.rs`) | The live webview DOM via CSS selectors | bats-core + curl | Interaction logic: palette open/filter/close, nav, overlays |

The DOM bridge exists because the webview *is* a real DOM — the app just
doesn't expose it to outside tooling. The bridge runs inside the app
(env-gated, debug builds only) and answers selector queries over
localhost HTTP.

### Why this shape (verified against upstream)

Dioxus-desktop 0.7.10 hardcodes WebKit automation OFF and owns the wry
`WebContext` privately (webview.rs:260) — Playwright cannot attach (its
WebKit is a standalone patched build), and WebDriver would require
forking dioxus-desktop. The Dioxus org tests its own desktop crate the
same way we do here: `document::eval` + synthetic `dispatchEvent`,
CSS selectors, zero `data-testid` (`DioxusLabs/dioxus`
`packages/desktop/headless_tests/`). Our bridge is that mechanism
delivered **cross-process via HTTP** so bats/any harness can drive it,
where the org calls it **in-process from Rust** `[[test]]` binaries.
Both are the same wry eval channel; HTTP trades a little purity for
language-agnostic, screenshot-friendly shell tests.

---

## The DOM bridge (src/test_bridge.rs)

Boot the app with `OPENKITE_TEST_PORT=<port>` and it serves a tiny HTTP
endpoint:

```
POST http://127.0.0.1:<port>/    {"selector": ".palette", "op": "count"}
→ {"ok": true, "result": 1}
```

The bridge is **compiled into every build but inert unless the env var is
set** — production runs are unaffected (one env read at startup).

### Ops reference

| op | selector target | extra fields | result |
|---|---|---|---|
| `count` | all matches | — | number of matches |
| `text` | first match | — | trimmed textContent |
| `visible` | first match | — | `true` if rendered (offsetParent ≠ null) |
| `class` | first match | — | className string |
| `click` | first match | — | dispatches a real click |
| `focus` | first match | — | `.focus()` |
| `type` | first match (input) | `"text": "..."` | sets value + dispatches `input`/`change` (fires Dioxus `oninput`) |
| `key` | (document) | `"key": "Escape"`, optional `"ctrl": true`, `"meta": true` | dispatches a bubbling keydown (fires webview keybinds) |

Every `result` is JSON-serializable; unknown ops return `{"ok": false, ...}`.

### How it works (threading, in case you touch it)

`document::eval` in Dioxus is valid only on the **UI thread** (the runtime is
thread-local). So the bridge installs **one long-lived eval from the
app-shell mount effect** (`spawn_bridge_worker` in `router.rs`), whose JS runs
a request/response pump:

```
HTTP listener (tokio) ──op──▶ mpsc ──▶ UI-thread worker ──eval.send──▶ webview JS
       ▲                                                              │ runs op on DOM
       └──────────────────────── oneshot reply ◀──dioxus.send──────────┘
```

The eval body must `return` the never-resolving pump promise — if it settles,
the Dioxus wrapper closes the query (`window.__msg_queues[id] = null`) and
`eval.send` silently breaks. A unit test guards this.

---

## Writing a DOM test (bats)

Add a `@test` block to `e2e/bridge_flows.bats`. Each test boots its own app
instance with the bridge enabled (`start_app` from `bridge_lib.bash`) and
asserts with the `assert_dom_*` helpers:

```bash
@test "my flow: doing the thing" {
  start_app                          # boots Xvfb + openbox + app + bridge
  assert_dom_visible '.sidebar'      # app is up

  dom '' key '{"key":"p","ctrl":true}'   # open palette (document keydown)
  sleep 1                                # let Dioxus re-render
  assert_dom_visible '.palette'

  dom '.palette-input' type '{"text":"logs"}'   # type into the filter
  sleep 1
  assert_dom_text '.palette-list' 'Logs'        # filtered result visible

  dom '' key '{"key":"Escape"}'
  sleep 1
  assert_dom_hidden '.palette'                 # closed cleanly
}
```

### Helper reference (bridge_lib.bash)

| helper | purpose |
|---|---|
| `start_app [env…]` | boot app under Xvfb with bridge; waits for it to answer |
| `dom <selector> <op> [json-extra]` | raw bridge call, prints `.result` |
| `assert_dom_count <selector> <n>` | exact match count |
| `assert_dom_text <selector> <substr>` | textContent contains substring |
| `assert_dom_visible / assert_dom_hidden <selector>` | rendered / not rendered |

### Finding selectors

The app uses stable class names — grep the source:

```bash
grep -n 'class: "' src/router.rs src/palette.rs src/switcher.rs
```

Key ones: `.app-shell`, `.sidebar`, `.nav-item`, `.content`, `.palette`,
`.palette-input`, `.palette-list`, `.palette-section`, `.switcher`,
`.topbar`, `.status`.

### Timing

Dioxus renders on the UI thread; after a `key`/`click`/`type`, sleep ~1s
before asserting. The bridge has no wait-for-selector primitive yet — if you
need polling, add a `wait_dom <selector> <op> <expected>` loop helper.

### Running locally

```bash
# build once (needs GTK/WebKit deps — or use CI)
BIN=../target/debug/openkite bats e2e/bridge_flows.bats

# run a subset
BIN=../target/debug/openkite bats --filter 'palette' e2e/bridge_flows.bats
```

CI runs the suite in `.github/workflows/e2e.yml` (`bats` job) on ubuntu-latest
with `xvfb`, `openbox`, `curl`, `python3`, and the GTK/WebKit build deps.

### Future: in-process Rust tests

For deep interaction coverage the Dioxus-org-idiomatic layer is Rust
`[[test]] harness = false` binaries (`packages/desktop/headless_tests/`
pattern): launch the app in-process, drive it with `document::eval`,
self-terminate via `window().close()`. The bats suite above is the
shell-level smoke layer — fast to add, language-agnostic, keeps
screenshots. Both can coexist; convert flows to Rust binaries if a test
outgrows shell ergonomics.

### Debugging a failing test

- `bats --trace` shows every command; add `echo` liberally
- On failure the tmp dir (`$BATS_TEST_TMPDIR`) keeps `app.log`, `xvfb.log`,
  `wm.log` — check `app.log` for the "test bridge listening" line and any
  panic
- Hit the bridge manually while the app runs:
  ```bash
  curl -s -X POST http://127.0.0.1:39871/ -d '{"selector":".palette","op":"visible"}'
  ```

---

## Adding a route-based visual baseline (no input automation)

Routes boot deterministically with `OPENKITE_ROUTE=/cluster` (see
`route_from_path` in `src/router.rs` and `e2e/capture-baselines.sh`):
baseline screenshots boot a fresh instance per route instead of clicking
through the UI. Overlays (palette, switcher) still need the DOM bridge or
xdotool.

---

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `bridge never came up` | app crashed at boot — read `app.log`; or `OPENKITE_TEST_PORT` collides (set `PORT` for a per-test port) |
| `eval failed: ...` in bridge reply | the selector op threw in JS — check the selector and op JSON |
| palette/overlay assertions flaky | add `sleep` after the triggering key/click (Dioxus render is async) |
| tests pass locally, fail in CI | CI is headless-disconnected; tests must not assume a cluster (no kubeconfig → disconnected state) |
