#!/usr/bin/env bash
# Shared helpers for the OKT-61 user-flow bats suite (e2e/user_flows.bats).
#
# Loaded by bats (bats `load`), NOT executed standalone. Each @test boots
# its own Xvfb + openbox + app instance via setup()/teardown() (bats runs
# both in the same subshell as the test body, so exports and PIDs persist).
#
# Env consumed:
#   BIN      — path to the openkite binary (required)
#   ART_ROOT — artifact root dir (required; per-test ART = $ART_ROOT/flow-NN)
#
# xdotool/import/openbox all read DISPLAY; we export a unique :NN per test
# (derived from BATS_TEST_NUMBER) so tests never collide on a shared Xvfb.

export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export LIBGL_ALWAYS_SOFTWARE=1
export RUST_BACKTRACE=full

SCREEN="1280x800x24"

XVFB_PID=""
WM_PID=""
APP_PID=""
WINDOW_ID=""

flow_log() { echo "[$FLOW_NAME] $*"; }

# --- boot / teardown -------------------------------------------------------

flow_setup() {
  : "${BIN:?BIN must be set to the openkite binary}"
  : "${ART_ROOT:?ART_ROOT must be set}"
  export FLOW_NAME="flow-$(printf '%02d' "${BATS_TEST_NUMBER:-1}")"
  export ART="$ART_ROOT/$FLOW_NAME"
  export DISPLAY=":$((90 + ${BATS_TEST_NUMBER:-1}))"
  mkdir -p "$ART"

  flow_log "starting Xvfb on $DISPLAY ($SCREEN)"
  Xvfb "$DISPLAY" -screen 0 "$SCREEN" -nolisten tcp >"$ART/xvfb.log" 2>&1 &
  XVFB_PID=$!
  sleep 1

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
  # WebKit paint settle
  sleep 3
}

flow_teardown() {
  kill "$APP_PID" "$WM_PID" "$XVFB_PID" 2>/dev/null || true
}

# --- window helpers ---------------------------------------------------------

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
  [ -n "$WINDOW_ID" ] || return 1
  flow_log "window found: $WINDOW_ID"
}

focus_window() {
  xdotool windowactivate --sync "$WINDOW_ID" 2>/dev/null || true
  sleep 1
  xdotool mousemove 640 400 click 1 2>/dev/null || true
  sleep 1
}

# --- screenshot + pixel assertions -----------------------------------------

shot() {
  import -window "$WINDOW_ID" "$ART/$1" 2>/dev/null || return 1
}

pixel_stddev() {
  identify -format "%[fx:standard_deviation]" "$1" 2>/dev/null
}

assert_rendered() {
  local png="$1" label="$2" std
  std=$(pixel_stddev "$png")
  flow_log "$label stddev=$std"
  awk -v s="$std" 'BEGIN { exit !(s > 0.01) }'
}

assert_pixels_changed() {
  local before="$1" after="$2" label="$3" threshold="${4:-500}" diff
  diff=$(compare -metric AE "$before" "$after" null: 2>&1 || true)
  flow_log "$label differing pixels: $diff (threshold: $threshold)"
  [ -n "$diff" ] && [ "$diff" -gt "$threshold" ]
}

assert_pixels_unchanged() {
  local before="$1" after="$2" label="$3" threshold="${4:-500}" diff
  diff=$(compare -metric AE "$before" "$after" null: 2>&1 || true)
  flow_log "$label differing pixels: $diff (threshold: $threshold)"
  [ -n "$diff" ] && [ "$diff" -le "$threshold" ]
}

assert_disconnected() {
  grep -q "no kubeconfig; starting disconnected" "$ART/app.log" || return 1
  flow_log "disconnected state confirmed"
}
