#!/usr/bin/env bash
# Capture TEN golden visual-regression baseline screenshots of OpenKite's
# real surfaces under Xvfb.
#
# OpenKite is a Dioxus *desktop* app (wry/tao WebKitGTK) — no web
# platform, so browser Playwright cannot drive it. We boot the real
# binary on Xvfb and drive the X11 layer. Route changes are done through
# the app's own command palette (Ctrl+P → type "go to X" → Enter), which
# is deterministic and layout-independent — coordinate-clicking sidebar
# items is fragile across WebKit font/theme rendering.
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

# --- Xvfb + WM + app bootstrap (same as run-desktop-e2e.sh) ------------
log "starting Xvfb on $DISPLAY_NUM (${SCREEN})"
Xvfb "$DISPLAY_NUM" -screen 0 "$SCREEN" -nolisten tcp >"$OUT/xvfb.log" 2>&1 &
XVFB_PID=$!
trap 'kill $XVFB_PID $WM_PID $APP_PID 2>/dev/null || true' EXIT
sleep 1
export DISPLAY="$DISPLAY_NUM"

log "starting openbox"
openbox >"$OUT/wm.log" 2>&1 &
WM_PID=$!
sleep 1

log "launching $BIN"
if command -v stdbuf >/dev/null 2>&1; then
  stdbuf -oL -eL "$BIN" >"$OUT/app.log" 2>&1 &
else
  "$BIN" >"$OUT/app.log" 2>&1 &
fi
APP_PID=$!

log "waiting for window"
WINDOW_ID=""
for _ in $(seq 1 60); do
  WINDOW_ID=$(xdotool search --onlyvisible --name "OpenKite" 2>/dev/null | head -1 || true)
  [ -n "$WINDOW_ID" ] && break
  WINDOW_ID=$(xdotool search --onlyvisible --class "openkite" 2>/dev/null | head -1 || true)
  [ -n "$WINDOW_ID" ] && break
  sleep 1
done
[ -n "$WINDOW_ID" ] || fail "app window never appeared (see app.log)"
log "window found: $WINDOW_ID"

# Activate + click into the webview so keydown listeners receive input.
xdotool windowactivate --sync "$WINDOW_ID" 2>/dev/null || true
sleep 1
xdotool mousemove 640 400 click 1 2>/dev/null || true
sleep 2

capture() { # <output-name> <label>
  local name="$1" label="$2"
  sleep 2  # let WebKit paint
  import -window "$WINDOW_ID" "$OUT/$name" 2>/dev/null || fail "screenshot $name failed"
  assert_nonblank "$OUT/$name" "$label"
}

# goto_route <query> — open palette, type the fuzzy query for the
# "Go to <route>" command, Enter to run it, wait for navigation.
goto_route() { # <query>
  local query="$1"
  xdotool key --clearmodifiers ctrl+p
  sleep 2
  xdotool type --delay 60 "$query"
  sleep 2
  xdotool key --clearmodifiers Return
  sleep 3
}

# ============================================================
# 10 golden baseline screenshots
# ============================================================

# 01. Home (/) — the app starts on the Home route.
log "capturing 01-home"
capture "01-home.png" "01-home"

# 02. Cluster (/cluster) — palette "go to cluster".
log "capturing 02-cluster"
goto_route "go to cluster"
capture "02-cluster.png" "02-cluster"

# 03. Workloads (/workloads) — palette "go to workloads".
log "capturing 03-workloads"
goto_route "go to workloads"
capture "03-workloads.png" "03-workloads"

# 04. Logs (/logs) — palette "go to logs".
log "capturing 04-logs"
goto_route "go to logs"
capture "04-logs.png" "04-logs"

# 05. Terminal (/terminal) — palette "go to terminal".
log "capturing 05-terminal"
goto_route "go to terminal"
capture "05-terminal.png" "05-terminal"

# 06. Config (/config) — palette "go to config".
log "capturing 06-config"
goto_route "go to config"
capture "06-config.png" "06-config"

# 07. Home again after palette round-trip (clean baseline for overlays).
log "capturing 07-home-return"
goto_route "go to home"
capture "07-home-return.png" "07-home-return"

# 08. Workloads via palette once more (deterministic re-entry).
log "capturing 08-workloads-reentry"
goto_route "go to workloads"
capture "08-workloads-reentry.png" "08-workloads-reentry"

# 09. Command Palette overlay open (Ctrl+P) over the current route.
log "capturing 09-palette"
xdotool key --clearmodifiers ctrl+p
sleep 2
capture "09-palette.png" "09-palette"
xdotool key --clearmodifiers Escape
sleep 1

# 10. Cluster Switcher overlay open (Ctrl+Tab).
log "capturing 10-switcher"
xdotool key --clearmodifiers ctrl+Tab
sleep 2
capture "10-switcher.png" "10-switcher"
xdotool key --clearmodifiers Escape
sleep 1

# --- summary ------------------------------------------------------------
log "captured 10 baselines:"
ls -la "$OUT"/*.png 2>/dev/null || true
log "DONE"
