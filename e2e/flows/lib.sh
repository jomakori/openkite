#!/usr/bin/env bash
# Shared helpers for per-flow E2E scripts.
#
# Each flow script sources this file, calls `flow_setup` to bring up Xvfb +
# openbox + the app, performs its xdotool/screenshot assertions, and calls
# `flow_teardown` at the end (or relies on the EXIT trap).
#
# Variables consumed (env or parameters):
#   BIN         — path to the openkite binary (required)
#   ART         — artifact directory for this flow (required)
#   FLOW_NAME   — short label used in log prefixes (required)
#   EXPECT_CONNECTED — 1 = require cluster connection (optional, default 0)

set -euo pipefail

# WebKitGTK headless rendering nudges (same as run-desktop-e2e.sh).
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export LIBGL_ALWAYS_SOFTWARE=1
export RUST_BACKTRACE=full

SCREEN="1280x800x24"
DISPLAY_NUM=":99"
WINDOW_ID=""
XVFB_PID=""
WM_PID=""
APP_PID=""

# log <msg>
flow_log() { echo "[$FLOW_NAME] $*"; }

# fail <msg>  — print, then exit non-zero
flow_fail() { flow_log "FAIL: $*"; exit 1; }

# pixel_stddev <png> → float 0..1 (0 = flat/blank image)
pixel_stddev() {
  identify -format "%[fx:standard_deviation]" "$1" 2>/dev/null
}

# assert_rendered <png> <label> — image must be non-blank (stddev > 0.01)
assert_rendered() {
  local png="$1" label="$2"
  local std
  std=$(pixel_stddev "$png")
  flow_log "$label stddev=$std"
  awk -v s="$std" 'BEGIN { exit !(s > 0.01) }' || flow_fail "$label is blank/flat (stddev=$std)"
}

# assert_pixels_changed <before> <after> <label> [threshold]
#   Compares two screenshots; the second must differ by > threshold pixels
#   (default 500). Uses ImageMagick compare -metric AE.
assert_pixels_changed() {
  local before="$1" after="$2" label="$3" threshold="${4:-500}"
  local diff
  diff=$(compare -metric AE "$before" "$after" null: 2>&1 || true)
  flow_log "$label differing pixels: $diff (threshold: $threshold)"
  [ -n "$diff" ] && [ "$diff" -gt "$threshold" ] || flow_fail "$label did not change the screen (diff=$diff, threshold=$threshold)"
}

# assert_pixels_unchanged <before> <after> <label> [threshold]
#   Asserts two screenshots are nearly identical (<= threshold differing
#   pixels). Used for "closing overlay returns to baseline" flows.
assert_pixels_unchanged() {
  local before="$1" after="$2" label="$3" threshold="${4:-500}"
  local diff
  diff=$(compare -metric AE "$before" "$after" null: 2>&1 || true)
  flow_log "$label differing pixels: $diff (threshold: $threshold)"
  [ -n "$diff" ] && [ "$diff" -le "$threshold" ] || flow_fail "$label unexpectedly changed the screen (diff=$diff, threshold=$threshold)"
}

# shot <filename> — screenshot the app window into ART/
shot() {
  local name="$1"
  import -window "$WINDOW_ID" "$ART/$name" 2>/dev/null || flow_fail "screenshot $name failed"
}

# wait_for_window — poll xdotool until the OpenKite window appears
wait_for_window() {
  flow_log "waiting for window"
  local i
  for i in $(seq 1 60); do
    WINDOW_ID=$(xdotool search --onlyvisible --name "OpenKite" 2>/dev/null | head -1 || true)
    [ -n "$WINDOW_ID" ] && break
    WINDOW_ID=$(xdotool search --onlyvisible --class "openkite" 2>/dev/null | head -1 || true)
    [ -n "$WINDOW_ID" ] && break
    sleep 1
  done
  [ -n "$WINDOW_ID" ] || flow_fail "app window never appeared (see app.log)"
  flow_log "window found: $WINDOW_ID"
}

# focus_window — activate + click into webview for keyboard input
focus_window() {
  xdotool windowactivate --sync "$WINDOW_ID" 2>/dev/null || true
  sleep 1
  xdotool mousemove 640 400 click 1 2>/dev/null || true
  sleep 1
}

# flow_setup — start Xvfb + openbox + app, wait for window
flow_setup() {
  : "${BIN:?BIN must be set to the openkite binary}"
  : "${ART:?ART must be set to the artifact dir}"
  : "${FLOW_NAME:?FLOW_NAME must be set}"
  mkdir -p "$ART"

  flow_log "starting Xvfb on $DISPLAY_NUM ($SCREEN)"
  Xvfb "$DISPLAY_NUM" -screen 0 "$SCREEN" -nolisten tcp >"$ART/xvfb.log" 2>&1 &
  XVFB_PID=$!
  trap 'flow_teardown' EXIT
  sleep 1
  export DISPLAY="$DISPLAY_NUM"

  flow_log "starting openbox"
  openbox >"$ART/wm.log" 2>&1 &
  WM_PID=$!
  sleep 1

  flow_log "launching $BIN"
  if command -v stdbuf >/dev/null 2>&1; then
    stdbuf -oL -eL "$BIN" >"$ART/app.log" 2>&1 &
  else
    "$BIN" >"$ART/app.log" 2>&1 &
  fi
  APP_PID=$!

  wait_for_window

  # Connection-state check (optional)
  if [ "${EXPECT_CONNECTED:-0}" = "1" ]; then
    if grep -q "cluster connected" "$ART/app.log"; then
      flow_log "connection OK: app joined the cluster"
    else
      flow_log "app.log tail:"
      tail -5 "$ART/app.log" || true
      flow_fail "expected connected app but no 'cluster connected' in app.log"
    fi
  fi

  # Give WebKit a moment to paint
  sleep 3
}

# flow_teardown — kill app, WM, Xvfb (called via EXIT trap)
flow_teardown() {
  kill $APP_PID $WM_PID $XVFB_PID 2>/dev/null || true
}

# assert_disconnected — verify app.log shows disconnected state
assert_disconnected() {
  grep -q "no kubeconfig; starting disconnected" "$ART/app.log" || flow_fail "expected disconnected state but app.log does not contain 'no kubeconfig; starting disconnected'"
  flow_log "disconnected state confirmed"
}
