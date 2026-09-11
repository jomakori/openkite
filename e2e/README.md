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
