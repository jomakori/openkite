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

# boot_and_capture <route-path> <output-name> <label>
#   Boot a fresh app instance on OPENKITE_ROUTE, screenshot once the
#   window paints, kill the instance. One boot per surface = the route is
#   the *initial* render, so the capture is deterministic.
boot_and_capture() { # <route> <name> <label>
  local route="$1" name="$2" label="$3"
  local app_log="$OUT/$name.app.log"

  log "[$name] booting on OPENKITE_ROUTE=$route"

  Xvfb "$DISPLAY_NUM" -screen 0 "$SCREEN" -nolisten tcp >"$OUT/$name.xvfb.log" 2>&1 &
  local xvfb_pid=$!
  sleep 1
  export DISPLAY="$DISPLAY_NUM"

  openbox >"$OUT/$name.wm.log" 2>&1 &
  local wm_pid=$!
  sleep 1

  if command -v stdbuf >/dev/null 2>&1; then
    OPENKITE_ROUTE="$route" stdbuf -oL -eL "$BIN" >"$app_log" 2>&1 &
  else
    OPENKITE_ROUTE="$route" "$BIN" >"$app_log" 2>&1 &
  fi
  local app_pid=$!

  # Wait for the window to appear.
  local wid=""
  for _ in $(seq 1 60); do
    wid=$(xdotool search --onlyvisible --name "OpenKite" 2>/dev/null | head -1 || true)
    [ -n "$wid" ] && break
    wid=$(xdotool search --onlyvisible --class "openkite" 2>/dev/null | head -1 || true)
    [ -n "$wid" ] && break
    sleep 1
  done

  if [ -z "$wid" ]; then
    kill $app_pid $wm_pid $xvfb_pid 2>/dev/null || true
    log "$name app.log tail:"
    tail -8 "$app_log" 2>/dev/null || true
    fail "[$name] window never appeared"
  fi
  log "[$name] window found: $wid"

  # Give WebKit time to paint the target route.
  sleep 4
  import -window "$wid" "$OUT/$name.png" 2>/dev/null || {
    kill $app_pid $wm_pid $xvfb_pid 2>/dev/null || true
    fail "[$name] screenshot failed"
  }
  assert_nonblank "$OUT/$name.png" "$label"

  # Confirm the app actually booted the requested route (log line).
  if ! grep -q "OPENKITE_ROUTE: booting onto route route=$route" "$app_log"; then
    log "[$name] WARNING: OPENKITE_ROUTE log line not found; app may have ignored the hook"
  fi

  kill $app_pid $wm_pid $xvfb_pid 2>/dev/null || true
  wait $app_pid 2>/dev/null || true
  sleep 1
  log "[$name] captured"
}

# ============================================================
# 10 golden baseline screenshots — one fresh boot per route.
# ============================================================

boot_and_capture "/"            "01-home"      "01-home"
boot_and_capture "/cluster"     "02-cluster"   "02-cluster"
boot_and_capture "/workloads"   "03-workloads" "03-workloads"
boot_and_capture "/logs"        "04-logs"      "04-logs"
boot_and_capture "/terminal"    "05-terminal"  "05-terminal"
boot_and_capture "/config"      "06-config"    "06-config"

# 07. Home again (return route after navigating away in prior boots).
boot_and_capture "/"            "07-home-return" "07-home-return"

# 08. Workloads re-entry (deterministic second visit).
boot_and_capture "/workloads"   "08-workloads-reentry" "08-workloads-reentry"

# 09. Command Palette overlay open (Ctrl+P) over Home. Requires input
#     automation because the palette is an overlay, not a route.
log "capturing 09-palette (overlay via Ctrl+P)"
Xvfb "$DISPLAY_NUM" -screen 0 "$SCREEN" -nolisten tcp >"$OUT/09.xvfb.log" 2>&1 &
XVFB_PID=$!
sleep 1
export DISPLAY="$DISPLAY_NUM"
openbox >"$OUT/09.wm.log" 2>&1 &
WM_PID=$!
sleep 1
stdbuf -oL -eL "$BIN" >"$OUT/09.app.log" 2>&1 &
APP_PID=$!
WID=""
for _ in $(seq 1 60); do
  WID=$(xdotool search --onlyvisible --name "OpenKite" 2>/dev/null | head -1 || true)
  [ -n "$WID" ] && break
  sleep 1
done
[ -n "$WID" ] || fail "09-palette: window never appeared"
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
kill $APP_PID $WM_PID $XVFB_PID 2>/dev/null || true

# 10. Cluster Switcher overlay open (Ctrl+Tab) over Home. Overlay, not a
#     route — driven with the same input automation as 09.
log "capturing 10-switcher (overlay via Ctrl+Tab)"
Xvfb "$DISPLAY_NUM" -screen 0 "$SCREEN" -nolisten tcp >"$OUT/10.xvfb.log" 2>&1 &
XVFB_PID=$!
sleep 1
export DISPLAY="$DISPLAY_NUM"
openbox >"$OUT/10.wm.log" 2>&1 &
WM_PID=$!
sleep 1
stdbuf -oL -eL "$BIN" >"$OUT/10.app.log" 2>&1 &
APP_PID=$!
WID=""
for _ in $(seq 1 60); do
  WID=$(xdotool search --onlyvisible --name "OpenKite" 2>/dev/null | head -1 || true)
  [ -n "$WID" ] && break
  sleep 1
done
[ -n "$WID" ] || fail "10-switcher: window never appeared"
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
kill $APP_PID $WM_PID $XVFB_PID 2>/dev/null || true

# --- summary ------------------------------------------------------------
log "captured 10 baselines:"
ls -la "$OUT"/*.png 2>/dev/null || true
log "DONE"
