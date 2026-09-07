#!/usr/bin/env bash
# Capture TEN golden visual-regression baseline screenshots of OpenKite's
# real surfaces under Xvfb, following the run-desktop-e2e.sh pattern.
#
# OpenKite is a Dioxus *desktop* app (wry/tao WebKitGTK) — no web platform,
# so browser Playwright cannot drive it. Instead we boot the real binary on
# Xvfb, navigate to each surface via xdotool (sidebar clicks + keybinds),
# and capture full-window PNGs with ImageMagick `import`.
#
# The app tolerates no-kubeconfig: it logs "no kubeconfig; starting
# disconnected" and renders a disconnected banner. All 10 surfaces render
# deterministically in that state.
#
# Usage: capture-baselines.sh <path-to-openkite-binary> <output-dir>
#
# Output: 10 PNGs named 01-home.png … 10-switcher.png in <output-dir>.

set -euo pipefail

BIN="${1:?path to openkite binary}"
OUT="${2:?output dir}"
mkdir -p "$OUT"

# WebKitGTK env nudges for headless X (same as run-desktop-e2e.sh).
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export LIBGL_ALWAYS_SOFTWARE=1
export RUST_BACKTRACE=full

SCREEN="1280x800x24"
DISPLAY_NUM=":99"

log() { echo "[capture-baselines] $*"; }
fail() { log "FAIL: $*"; exit 1; }

pixel_stddev() { # <png> -> stdout float in 0..1 (0 = flat image)
  identify -format "%[fx:standard_deviation]" "$1" 2>/dev/null
}

# Assert screenshot is non-blank: stddev > 0.01 (same threshold as harness).
assert_nonblank() { # <png> <label>
  local png="$1" label="$2"
  local std
  std=$(pixel_stddev "$png")
  log "$label stddev=$std"
  awk -v s="$std" 'BEGIN { exit !(s > 0.01) }' || fail "$label screenshot is blank/flat (stddev=$std)"
}

# --- start Xvfb ---------------------------------------------------------
log "starting Xvfb on $DISPLAY_NUM (${SCREEN})"
Xvfb "$DISPLAY_NUM" -screen 0 "$SCREEN" -nolisten tcp >"$OUT/xvfb.log" 2>&1 &
XVFB_PID=$!
trap 'kill $XVFB_PID $WM_PID $APP_PID 2>/dev/null || true' EXIT
sleep 1
export DISPLAY="$DISPLAY_NUM"

# --- start a window manager ---------------------------------------------
log "starting openbox"
openbox >"$OUT/wm.log" 2>&1 &
WM_PID=$!
sleep 1

# --- launch app ---------------------------------------------------------
log "launching $BIN"
if command -v stdbuf >/dev/null 2>&1; then
  stdbuf -oL -eL "$BIN" >"$OUT/app.log" 2>&1 &
else
  "$BIN" >"$OUT/app.log" 2>&1 &
fi
APP_PID=$!

# --- wait for the window ------------------------------------------------
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

# Activate window + click into webview for keyboard focus.
xdotool windowactivate --sync "$WINDOW_ID" 2>/dev/null || true
sleep 1
xdotool mousemove 640 400 click 1 2>/dev/null || true
sleep 2

# --- helper: screenshot + assert non-blank ------------------------------
capture() { # <output-name> <label>
  local name="$1" label="$2"
  sleep 2  # let WebKit paint
  import -window "$WINDOW_ID" "$OUT/$name" 2>/dev/null || fail "screenshot $name failed"
  assert_nonblank "$OUT/$name" "$label"
}

# --- helper: click sidebar nav item by text -----------------------------
# The sidebar nav items are <a> elements: Cluster, Workloads, Logs, Terminal, Config.
# We use xdotool to click approximate coordinates in the sidebar.
# Sidebar is ~200px wide on the left; nav items are stacked vertically.
# Home is the brand area at top; nav items start ~80px down.
click_nav() { # <y-coordinate>
  local y="$1"
  xdotool mousemove 100 "$y" click 1 2>/dev/null || true
  sleep 1
}

# ============================================================
# 10 golden baseline screenshots
# ============================================================

# 01. Home (/) — the app starts on Home route by default.
log "capturing 01-home"
capture "01-home.png" "01-home"

# 02. Cluster (/cluster) — click "Cluster" nav item.
log "capturing 02-cluster"
click_nav 120
capture "02-cluster.png" "02-cluster"

# 03. Workloads (/workloads) — Pods tab is the default kind.
log "capturing 03-workloads-pods"
click_nav 160
sleep 1
# Click into the content area to ensure focus.
xdotool mousemove 640 400 click 1 2>/dev/null || true
capture "03-workloads-pods.png" "03-workloads-pods"

# 04. Workloads — Deployments tab.
# Kind tabs are below the nav, in the content area top row.
log "capturing 04-workloads-deployments"
# Tab buttons are in a horizontal row at top of content. Approximate x positions:
# Pods ~250, Nodes ~310, Deployments ~400, StatefulSets ~500, DaemonSets ~620,
# ReplicaSets ~720, Jobs ~810, CronJobs ~870, Secrets ~960, + New
# Click "Deployments" tab.
xdotool mousemove 400 90 click 1 2>/dev/null || true
sleep 1
capture "04-workloads-deployments.png" "04-workloads-deployments"

# 05. Workloads — Secrets tab.
log "capturing 05-workloads-secrets"
# Click "Secrets" tab — approximate position further right.
xdotool mousemove 960 90 click 1 2>/dev/null || true
sleep 1
capture "05-workloads-secrets.png" "05-workloads-secrets"

# 06. Logs (/logs) — click "Logs" nav item.
log "capturing 06-logs"
click_nav 200
capture "06-logs.png" "06-logs"

# 07. Terminal (/terminal) — click "Terminal" nav item.
log "capturing 07-terminal"
click_nav 240
capture "07-terminal.png" "07-terminal"

# 08. Config (/config) — click "Config" nav item.
log "capturing 08-config"
click_nav 280
capture "08-config.png" "08-config"

# 09. Command Palette (Ctrl+P overlay).
# Navigate back to Home first for a clean backdrop.
log "capturing 09-palette"
click_nav 80   # Home/brand area
sleep 1
xdotool mousemove 640 400 click 1 2>/dev/null || true
sleep 1
xdotool key --clearmodifiers ctrl+p
sleep 2
capture "09-palette.png" "09-palette"
# Close palette.
xdotool key --clearmodifiers Escape
sleep 1

# 10. Cluster Switcher (Ctrl+Tab overlay).
log "capturing 10-switcher"
xdotool mousemove 640 400 click 1 2>/dev/null || true
sleep 1
# The switcher keybind is Ctrl+Tab (OKT-51).
xdotool key --clearmodifiers ctrl+Tab
sleep 2
capture "10-switcher.png" "10-switcher"
# Close switcher.
xdotool key --clearmodifiers Escape
sleep 1

# --- summary ------------------------------------------------------------
log "captured 10 baselines:"
ls -la "$OUT"/*.png 2>/dev/null || true
log "DONE"
