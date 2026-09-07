#!/usr/bin/env bash
# Desktop E2E for OpenKite under a virtual X display.
#
# OpenKite is a Dioxus *desktop* app (wry/tao webview) — there is no web
# platform, so browser Playwright cannot drive it. Instead we run the real
# binary on Xvfb and drive the X11 layer: assert a window appears, take
# screenshots, and exercise the palette keybind (Ctrl+P) that the webview
# handles via document::eval.
#
# Requires: xvfb, xdotool, imagemagick (import/compare/identify), dbus-x11.
#
# Usage: run-desktop-e2e.sh <path-to-openkite-binary> <artifact-dir>

set -euo pipefail

BIN="${1:?path to openkite binary}"
ART="${2:?artifact dir}"
mkdir -p "$ART"

# WebKitGTK needs a few env nudges to render on a headless X server.
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export LIBGL_ALWAYS_SOFTWARE=1
export RUST_BACKTRACE=full

SCREEN="1280x800x24"
DISPLAY_NUM=":99"

log() { echo "[desktop-e2e] $*"; }
fail() { log "FAIL: $*"; exit 1; }

pixel_stddev() { # <png> -> stdout float in 0..1 (normalized; 0 = flat image)
  identify -format "%[fx:standard_deviation]" "$1" 2>/dev/null
}

# --- start Xvfb ---------------------------------------------------------
log "starting Xvfb on $DISPLAY_NUM (${SCREEN})"
Xvfb "$DISPLAY_NUM" -screen 0 "$SCREEN" -nolisten tcp >"$ART/xvfb.log" 2>&1 &
XVFB_PID=$!
trap 'kill $XVFB_PID $APP_PID 2>/dev/null || true' EXIT
sleep 1
export DISPLAY="$DISPLAY_NUM"

# --- launch app ---------------------------------------------------------
# No kubeconfig in CI -> app logs "no kubeconfig; starting disconnected".
log "launching $BIN"
"$BIN" >"$ART/app.log" 2>&1 &
APP_PID=$!

# --- wait for the window ------------------------------------------------
log "waiting for window"
WINDOW_ID=""
for _ in $(seq 1 60); do
  WINDOW_ID=$(xdotool search --onlyvisible --name "OpenKite" 2>/dev/null | head -1 || true)
  [ -n "$WINDOW_ID" ] && break
  # Some WMs/toolkits title the window differently; fall back to any new X window.
  WINDOW_ID=$(xdotool search --onlyvisible --class "openkite" 2>/dev/null | head -1 || true)
  [ -n "$WINDOW_ID" ] && break
  sleep 1
done
[ -n "$WINDOW_ID" ] || fail "app window never appeared (see app.log)"
log "window found: $WINDOW_ID"

# --- first screenshot ---------------------------------------------------
sleep 3  # let WebKit paint
import -window "$WINDOW_ID" "$ART/01-shell.png" 2>/dev/null || fail "screenshot 1 failed"
STD=$(pixel_stddev "$ART/01-shell.png")
log "shell screenshot stddev=$STD"
# identify normalizes to 0..1: a flat/blank webview is ~0.001; a rendered
# UI (text, sidebar, borders) is comfortably > 0.01.
awk -v s="$STD" 'BEGIN { exit !(s > 0.01) }' || fail "shell screenshot is blank/flat"

# --- exercise the palette (Ctrl+P opens, Escape closes) ----------------
log "opening palette with Ctrl+P"
xdotool windowactivate --sync "$WINDOW_ID" 2>/dev/null || true
xdotool key --window "$WINDOW_ID" ctrl+p
sleep 2
import -window "$WINDOW_ID" "$ART/02-palette.png" 2>/dev/null || fail "screenshot 2 failed"
PAL_STD=$(pixel_stddev "$ART/02-palette.png")
log "palette screenshot stddev=$PAL_STD"
# Palette overlay changes the pixels measurably (different layout + backdrop).
DIFF=$(compare -metric AE "$ART/01-shell.png" "$ART/02-palette.png" null: 2>&1 || true)
log "shell-vs-palette differing pixels: $DIFF"
[ -n "$DIFF" ] && [ "$DIFF" -gt 500 ] || fail "palette did not change the screen"

log "closing palette with Escape"
xdotool key --window "$WINDOW_ID" Escape
sleep 2
import -window "$WINDOW_ID" "$ART/03-closed.png" 2>/dev/null || fail "screenshot 3 failed"
CLOSE_DIFF=$(compare -metric AE "$ART/01-shell.png" "$ART/03-closed.png" null: 2>&1 || true)
log "shell-vs-closed differing pixels: $CLOSE_DIFF"

log "PASS"
