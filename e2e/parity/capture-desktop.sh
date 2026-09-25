#!/usr/bin/env bash
# Desktop-side capture for the console parity gate (OKT-129).
#
# Boots the REAL openkite binary under Xvfb and screenshots the console
# surfaces the browser side also captures, so the two renders can be
# pixel-compared (see e2e/parity/compare.sh). Everything here exists to make
# that frame deterministic:
#
# - OPENKITE_ROUTE=/workloads|/cluster|/config boots each console-owned route
#   directly (src/router.rs hook) — no clock, no input races, no navigation.
# - OPENKITE_CONSOLE_FIXTURES points the host's bridge at the payloads the
#   browser console renders from, so the two sides differ in renderer only.
#   Without it the desktop console renders the disconnected empty state and a
#   comparison against the browser's populated tables would measure data.
# - An isolated $HOME with `menuBar = "hide"` removes the platform menu bar, so
#   the console (which is `position: fixed; inset: 0`) owns the whole client
#   area and the capture is a 1:1 frame against the browser viewport — no crop
#   constant, no alignment search. The same isolated $HOME is also what makes
#   the run hermetic: no kubeconfig, no plugin directory.
# - WebKit animations are disabled (GTK `gtk-enable-animations=0`) and the
#   settle sleep matches the existing baseline harness, because a transition
#   caught mid-flight would be a flaky diff rather than a regression.
#
# Usage: capture-desktop.sh <path-to-openkite-binary> <output-dir> [fixtures.json]
# Env:   EXPECT_DIMS  frame the capture must have (default 800x600)
# Output: <surface>.desktop.png + <surface>.app.log per surface, in <out-dir>.

set -euo pipefail

BIN="${1:?path to openkite binary}"
OUT="${2:?output dir}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES="${3:-$HERE/fixtures.json}"
EXPECT_DIMS="${EXPECT_DIMS:-800x600}"

mkdir -p "$OUT"

if [ ! -f "$BIN" ]; then
  echo "[parity] FAIL: no binary at $BIN" >&2
  exit 1
fi
if [ ! -f "$FIXTURES" ]; then
  echo "[parity] FAIL: no fixture export at $FIXTURES (run \`npm run export:parity-fixtures\`)" >&2
  exit 1
fi

export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export LIBGL_ALWAYS_SOFTWARE=1
export RUST_BACKTRACE=full

SCREEN="1280x800x24"
DISPLAY_NUM=":99"

log() { echo "[parity] $*"; }
fail() { log "FAIL: $*" >&2; exit 1; }

# The surfaces the desktop can be booted onto deterministically. They are
# exactly the console-owned routes: /workloads → pods, /cluster → overview,
# /config → configmaps (src/router.rs `console_route`). Logs, terminal and the
# plugin wildcard stay on the native shell, so they are not console surfaces.
SURFACES=(
  "01-pods:/workloads"
  "02-overview:/cluster"
  "03-configmaps:/config"
)

# --- isolated $HOME -----------------------------------------------------
# Also holds the XDG dirs, so GTK finds settings.ini there and nothing else
# from the developer's session leaks in.
export HOME="$OUT/home"
mkdir -p "$HOME/.openkite" "$HOME/.config/gtk-3.0"
OPENKITE_FIXTURES_ABS="$(cd "$(dirname "$FIXTURES")" && pwd)/$(basename "$FIXTURES")"
printf 'menuBar = "hide"\n' >"$HOME/.openkite/config.toml"
printf '[Settings]\ngtk-enable-animations=0\n' >"$HOME/.config/gtk-3.0/settings.ini"
log "isolated HOME=$HOME (menu bar hidden, no kubeconfig, fixtures=$OPENKITE_FIXTURES_ABS)"

# --- one Xvfb + one openbox for the whole run ---------------------------
# A fresh Xvfb per surface races the previous server's socket release
# (capture-baselines.sh hit this), so the display is shared and each surface
# gets a fresh app instance instead.
log "starting Xvfb on $DISPLAY_NUM ($SCREEN)"
Xvfb "$DISPLAY_NUM" -screen 0 "$SCREEN" -nolisten tcp >"$OUT/xvfb.log" 2>&1 &
XVFB_PID=$!
sleep 1
export DISPLAY="$DISPLAY_NUM"

openbox >"$OUT/wm.log" 2>&1 &
WM_PID=$!
sleep 1

trap 'kill ${APP_PID:-} $WM_PID $XVFB_PID 2>/dev/null || true' EXIT

# wait_for_window <app_log> — the visible OpenKite window id, or a hard fail.
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
    tail -12 "$app_log" 2>/dev/null || true
    fail "window never appeared (see $app_log)"
  fi
  echo "$wid"
}

# capture <surface> <route>
capture() {
  local id="$1" route="$2"
  local app_log="$OUT/$id.app.log" png="$OUT/$id.desktop.png"

  log "[$id] booting on $route"
  if command -v stdbuf >/dev/null 2>&1; then
    OPENKITE_ROUTE="$route" OPENKITE_CONSOLE_FIXTURES="$OPENKITE_FIXTURES_ABS" \
      stdbuf -oL -eL "$BIN" >"$app_log" 2>&1 &
  else
    OPENKITE_ROUTE="$route" OPENKITE_CONSOLE_FIXTURES="$OPENKITE_FIXTURES_ABS" \
      "$BIN" >"$app_log" 2>&1 &
  fi
  APP_PID=$!

  local wid
  wid=$(wait_for_window "$app_log")
  log "[$id] window $wid"

  # WebKit paint settle (the existing baseline harness uses the same 4s).
  sleep 4
  import -window "$wid" "$png" 2>/dev/null || fail "[$id] screenshot failed"

  # 1. The frame is the client area at the size the browser viewport uses. If
  #    this fires, a window decoration or a different winit default is in the
  #    capture and the comparison would be offset — fail loudly rather than
  #    crop blindly.
  local dims
  dims=$(identify -format '%wx%h' "$png")
  [ "$dims" = "$EXPECT_DIMS" ] ||
    fail "[$id] captured $dims, expected $EXPECT_DIMS (window decoration or menu bar in the frame?)"
  log "[$id] dims=$dims"

  # 2. Not blank: a black frame would compare "equal" to another black frame.
  local std
  std=$(identify -format '%[fx:standard_deviation]' "$png" 2>/dev/null)
  awk -v s="$std" 'BEGIN { exit !(s > 0.01) }' ||
    fail "[$id] screenshot is blank/flat (stddev=$std)"
  log "[$id] stddev=$std"

  # 3. The route hook fired: without it the app booted onto / and this capture
  #    is not the surface it claims to be.
  grep -q 'OPENKITE_ROUTE: booting onto route' "$app_log" ||
    fail "[$id] OPENKITE_ROUTE hook did not fire (app.log)"

  # 4. Fixture mode is live: otherwise the comparison pits fixture data against
  #    a disconnected empty state.
  grep -q 'console fixtures: enabled' "$app_log" ||
    fail "[$id] fixture mode is not active — the host did not load $OPENKITE_FIXTURES_ABS"

  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  sleep 2
  log "[$id] captured $png"
}

for entry in "${SURFACES[@]}"; do
  capture "${entry%%:*}" "${entry#*:}"
done

trap - EXIT
kill "$WM_PID" "$XVFB_PID" 2>/dev/null || true
log "captured ${#SURFACES[@]} desktop surfaces at $EXPECT_DIMS"
ls -la "$OUT"/*.desktop.png 2>/dev/null || true
log "DONE"
