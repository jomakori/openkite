#!/usr/bin/env bash
# Capture TEN golden visual-regression baseline screenshots of OpenKite's
# real surfaces under Xvfb.
#
# OpenKite is a Dioxus *desktop* app (wry/tao WebKitGTK) — no web
# platform, so browser Playwright cannot drive it. We boot the real
# binary on Xvfb. Route reach is deterministic: each surface is captured
# by booting a fresh app instance with OPENKITE_ROUTE=<path> (src/router.rs
# AppShell hook, OKT-61) — no coordinate clicks, no palette typing, no
# input races. This is what makes the baselines pixel-stable across runs.
#
# All surfaces render in the disconnected state (no kubeconfig): the app
# logs "no kubeconfig; starting disconnected" and shows empty-state
# panels instead of live cluster data.
#
# One Xvfb + one openbox for the whole run; each surface boots a fresh
# app instance on the SAME display (killing the previous instance). A
# fresh Xvfb per surface caused :99 restart races (previous server had
# not released the socket → new server died → window never appeared).
#
# Usage: capture-baselines.sh <path-to-openkite-binary> <output-dir>
# Output: 10 PNGs named 01-home.png … 10-switcher.png in <output-dir>.

set -euo pipefail

BIN="${1:?path to openkite binary}"
OUT="${2:?output dir}"
mkdir -p "$OUT"

export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export LIBGL_ALWAYS_SOFTWARE=1
export RUST_BACKTRACE=full

SCREEN="1280x800x24"
DISPLAY_NUM=":99"

log() { echo "[capture-baselines] $*"; }
fail() { log "FAIL: $*"; exit 1; }

pixel_stddev() {
  identify -format "%[fx:standard_deviation]" "$1" 2>/dev/null
}

assert_nonblank() { # <png> <label>
  local png="$1" label="$2"
  local std
  std=$(pixel_stddev "$png")
  log "$label stddev=$std"
  awk -v s="$std" 'BEGIN { exit !(s > 0.01) }' || fail "$label screenshot is blank/flat (stddev=$std)"
}

# boot_app [env-assignments...] — boot ONE app instance on the shared
# display. Caller must kill it (capture_one does). Env assignments are
# passed inline so OPENKITE_ROUTE reaches the app process.
boot_app() {
  local app_log="$1"; shift
  if command -v stdbuf >/dev/null 2>&1; then
    env "$@" stdbuf -oL -eL "$BIN" >"$app_log" 2>&1 &
  else
    env "$@" "$BIN" >"$app_log" 2>&1 &
  fi
  APP_PID=$!
}

# wait_for_window <app_log> — poll xdotool until the OpenKite window is
# visible or 60s elapse. Prints the window id on success.
wait_for_window() {
  local app_log="$1" wid=""
  for _ in $(seq 1 60); do
    wid=$(xdotool search --onlyvisible --name "OpenKite" 2>/dev/null | head -1 || true)
    [ -n "$wid" ] && break
    wid=$(xdotool search --onlyvisible --class "openkite" 2>/dev/null | head -1 || true)
    [ -n "$wid" ] && break
    sleep 1
  done
  if [ -z "$wid" ]; then
    log "app.log tail:"
    tail -8 "$app_log" 2>/dev/null || true
    fail "window never appeared"
  fi
  echo "$wid"
}

# capture_one <output-name> <label> [env-assignments...]
#   Boot a fresh app instance (with optional env, e.g. OPENKITE_ROUTE=),
#   screenshot once the window paints, kill the instance.
capture_one() { # <name> <label> [env...]
  local name="$1" label="$2"; shift 2
  local app_log="$OUT/$name.app.log"
  local png="$OUT/$name.png"

  log "[$name] booting app (env: $*)"
  boot_app "$app_log" "$@"

  local wid
  wid=$(wait_for_window "$app_log")
  log "[$name] window found: $wid"

  # Give WebKit time to paint the target route.
  sleep 4
  import -window "$wid" "$png" 2>/dev/null || {
    kill $APP_PID 2>/dev/null || true
    fail "[$name] screenshot failed"
  }
  assert_nonblank "$png" "$label"

  # Confirm the app actually booted the requested route (log line).
  # Grep the message only — tracing's field rendering (route="/x" vs
  # route=/x) is not stable across fmt versions.
  if [ "$#" -gt 0 ]; then
    if ! grep -q "OPENKITE_ROUTE: booting onto route" "$app_log"; then
      log "[$name] WARNING: OPENKITE_ROUTE log line not found; app may have ignored the hook"
    fi
  fi

  kill $APP_PID 2>/dev/null || true
  wait $APP_PID 2>/dev/null || true
  sleep 2
  log "[$name] captured"
}

# ============================================================
# One Xvfb + one openbox for the whole run (no :99 restart races).
# ============================================================

log "starting Xvfb on $DISPLAY_NUM ($SCREEN)"
Xvfb "$DISPLAY_NUM" -screen 0 "$SCREEN" -nolisten tcp >"$OUT/xvfb.log" 2>&1 &
XVFB_PID=$!
sleep 1
export DISPLAY="$DISPLAY_NUM"

openbox >"$OUT/wm.log" 2>&1 &
WM_PID=$!
sleep 1

trap 'kill $APP_PID $WM_PID $XVFB_PID 2>/dev/null || true' EXIT

# 10 golden baseline screenshots — one fresh app boot per surface.
# Route surfaces boot via OPENKITE_ROUTE; the two overlays (palette,
# switcher) are NOT routes and are driven with xdotool instead.
# ============================================================

capture_one "01-home" "01-home" OPENKITE_ROUTE="/"
capture_one "02-cluster" "02-cluster" OPENKITE_ROUTE="/cluster"
capture_one "03-workloads" "03-workloads" OPENKITE_ROUTE="/workloads"
capture_one "04-logs" "04-logs" OPENKITE_ROUTE="/logs"
capture_one "05-terminal" "05-terminal" OPENKITE_ROUTE="/terminal"
capture_one "06-config" "06-config" OPENKITE_ROUTE="/config"

# 07. Home again (return route after navigating away in prior boots).
capture_one "07-home-return" "07-home-return" OPENKITE_ROUTE="/"

# 08. Workloads re-entry (deterministic second visit).
capture_one "08-workloads-reentry" "08-workloads-reentry" OPENKITE_ROUTE="/workloads"

# 09. Command Palette overlay open (Ctrl+P) over Home. Requires input
#     automation because the palette is an overlay, not a route.
log "capturing 09-palette (overlay via Ctrl+P)"
boot_app "$OUT/09-palette.app.log"
WID=$(wait_for_window "$OUT/09-palette.app.log")
log "09-palette window found: $WID"
xdotool windowactivate --sync "$WID" 2>/dev/null || true
sleep 1
xdotool mousemove 640 400 click 1 2>/dev/null || true
sleep 2
xdotool key --clearmodifiers ctrl+p
sleep 2
import -window "$WID" "$OUT/09-palette.png" 2>/dev/null || fail "09-palette screenshot failed"
assert_nonblank "$OUT/09-palette.png" "09-palette"
xdotool key --clearmodifiers Escape
sleep 1
kill $APP_PID 2>/dev/null || true
wait $APP_PID 2>/dev/null || true
sleep 2
log "09-palette captured"

# 10. Cluster Switcher overlay open (Ctrl+Tab) over Home. Overlay, not a
#     route — driven with the same input automation as 09.
log "capturing 10-switcher (overlay via Ctrl+Tab)"
boot_app "$OUT/10-switcher.app.log"
WID=$(wait_for_window "$OUT/10-switcher.app.log")
log "10-switcher window found: $WID"
xdotool windowactivate --sync "$WID" 2>/dev/null || true
sleep 1
xdotool mousemove 640 400 click 1 2>/dev/null || true
sleep 2
xdotool key --clearmodifiers ctrl+Tab
sleep 2
import -window "$WID" "$OUT/10-switcher.png" 2>/dev/null || fail "10-switcher screenshot failed"
assert_nonblank "$OUT/10-switcher.png" "10-switcher"
xdotool key --clearmodifiers Escape
sleep 1
kill $APP_PID 2>/dev/null || true
wait $APP_PID 2>/dev/null || true
sleep 2
log "10-switcher captured"

# --- summary ------------------------------------------------------------
trap - EXIT
kill $WM_PID $XVFB_PID 2>/dev/null || true
log "captured 10 baselines:"
ls -la "$OUT"/*.png 2>/dev/null || true
log "DONE"
