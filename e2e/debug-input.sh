#!/usr/bin/env bash
# Input diagnostic for OpenKite desktop E2E: determines whether the app
# responds to *any* X11 input under Xvfb (mouse clicks on nav items) and
# whether keyboard events land. Exits 0 always — it's a diagnostic.
set -uo pipefail
BIN="${1:?path to openkite binary}"
ART="${2:?artifact dir}"
mkdir -p "$ART"
export WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1

Xvfb :99 -screen 0 1280x800x24 -nolisten tcp >"$ART/xvfb.log" 2>&1 &
XV=$!
trap 'kill $XV $WM $APP 2>/dev/null || true' EXIT
sleep 1; export DISPLAY=:99
openbox >"$ART/wm.log" 2>&1 &
WM=$!
sleep 1
"$BIN" >"$ART/app.log" 2>&1 &
APP=$!

WID=""
for _ in $(seq 1 60); do
  WID=$(xdotool search --onlyvisible --name "OpenKite" 2>/dev/null | head -1 || true)
  [ -n "$WID" ] && break
  sleep 1
done
[ -n "$WID" ] || { echo "DBG: no window"; exit 0; }
sleep 4
import -window "$WID" "$ART/d0-baseline.png" 2>/dev/null
echo "DBG: window=$WID geom=$(xdotool getwindowgeometry "$WID" | tr '\n' ' ')"

xdotool windowactivate --sync "$WID" 2>/dev/null || true
sleep 1
echo "DBG: focused window after activate = $(xdotool getwindowfocus 2>/dev/null || echo none)"

# Click sweep: sidebar nav items sit at x~90 in a ~800px window. Probe y.
for Y in 80 120 160 200 240 280 320; do
  xdotool mousemove 90 "$Y" click 1 2>/dev/null
  sleep 1
  import -window "$WID" "$ART/dY${Y}.png" 2>/dev/null
  D=$(compare -metric AE "$ART/d0-baseline.png" "$ART/dY${Y}.png" null: 2>&1 || true)
  echo "DBG: click y=$Y diff_vs_baseline=$D"
done

# Keyboard: ctrl+p after a click into content.
xdotool mousemove 640 300 click 1 2>/dev/null
sleep 1
xdotool key --clearmodifiers ctrl+p
sleep 2
import -window "$WID" "$ART/dpalette.png" 2>/dev/null
D=$(compare -metric AE "$ART/d0-baseline.png" "$ART/dpalette.png" null: 2>&1 || true)
echo "DBG: ctrl+p after content click diff=$D"
