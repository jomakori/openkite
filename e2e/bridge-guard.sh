#!/usr/bin/env bash
# OKT-95 bridge-dispatch regression guard.
#
# Boots the real desktop binary under Xvfb with a test-only JS plugin
# installed, then asserts the /openkite dispatch path answered with a
# well-formed JSON envelope, that the Dioxus-side registration mirror wrote,
# and that NO panic reached app.log.
#
# The register POST from the plugin walks the path a mirror regression breaks:
#   dispatch_bridge_post (tokio worker) -> Bridge::handle_post -> MIRROR_TX
#     -> refresh_registrations on the Dioxus side.
# The no-panic assertion is the one that catches such a regression: the panic
# is contained, so the surface stays healthy and only app.log shows it.
#
# Usage: bridge-guard.sh <path-to-openkite-binary> <artifact-dir>

set -euo pipefail

BIN="${1:?path to openkite binary}"
ART="${2:?artifact dir}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURE="${3:-$HERE/fixtures/okt95-guard}"
mkdir -p "$ART"
# Absolute: $ART becomes the isolated $HOME below, so a relative path would
# resolve against whatever CWD the app is launched with.
ART="$(cd "$ART" && pwd)"

export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export LIBGL_ALWAYS_SOFTWARE=1
export RUST_BACKTRACE=full

SCREEN="1280x800x24"
DISPLAY_NUM=":99"
XVFB_PID=""
WM_PID=""
APP_PID=""

log() { echo "[bridge-guard] $*"; }
fail() {
  log "FAIL: $*"
  if [ -f "$ART/app.log" ]; then
    log "app.log tail:"
    tail -40 "$ART/app.log" || true
  fi
  exit 1
}
cleanup() { kill "$APP_PID" "$WM_PID" "$XVFB_PID" 2>/dev/null || true; }
trap cleanup EXIT

# Isolate HOME so the test-only plugin is the only JS plugin discovered.
export HOME="$ART/home"
mkdir -p "$HOME/.openkite/plugins"
cp -R "$FIXTURE" "$HOME/.openkite/plugins/okt95-guard"

log "starting Xvfb on $DISPLAY_NUM ($SCREEN)"
Xvfb "$DISPLAY_NUM" -screen 0 "$SCREEN" -nolisten tcp >"$ART/xvfb.log" 2>&1 &
XVFB_PID=$!
sleep 1
export DISPLAY="$DISPLAY_NUM"

log "starting openbox"
openbox >"$ART/wm.log" 2>&1 &
WM_PID=$!
sleep 1

log "launching $BIN (HOME=$HOME)"
if command -v stdbuf >/dev/null 2>&1; then
  stdbuf -oL -eL "$BIN" >"$ART/app.log" 2>&1 &
else
  "$BIN" >"$ART/app.log" 2>&1 &
fi
APP_PID=$!

log "waiting for window"
WINDOW_ID=""
for _ in $(seq 1 60); do
  WINDOW_ID=$(xdotool search --onlyvisible --name "OpenKite" 2>/dev/null | head -1 || true)
  [ -z "$WINDOW_ID" ] && WINDOW_ID=$(xdotool search --onlyvisible --class "openkite" 2>/dev/null | head -1 || true)
  [ -n "$WINDOW_ID" ] && break
  sleep 1
done
[ -n "$WINDOW_ID" ] || fail "app window never appeared (see app.log)"
log "window found: $WINDOW_ID"

# AppShell evals discovered bundles after mount; poll for the test plugin.
for _ in $(seq 1 60); do
  if grep -q "evaluating js plugin bundle" "$ART/app.log" 2>/dev/null; then break; fi
  sleep 1
done
if ! awk '/evaluating js plugin bundle/ && /okt95-guard/ { found = 1 } END { exit !found }' "$ART/app.log"; then
  log "js plugin line(s):"
  grep -n "js plugin" "$ART/app.log" || true
  fail "test plugin 'okt95-guard' was never evaluated (discovery or enable failure?)"
fi

# The mirror write happens on the Dioxus side after the tokio handler pings it.
MIRRORED=0
for _ in $(seq 1 30); do
  if grep -q "registration mirror updated" "$ART/app.log" 2>/dev/null; then MIRRORED=1; break; fi
  sleep 1
done

# (1) The register dispatch returned a well-formed JSON envelope.
if ! grep -q '"status":"ok"' "$ART/app.log"; then
  log "bridge register response line(s):"
  grep -n "bridge register response" "$ART/app.log" || true
  fail "no ok JSON envelope in app.log"
fi
if ! grep -q '"registered":"status"' "$ART/app.log"; then
  log "bridge register response line(s):"
  grep -n "bridge register response" "$ART/app.log" || true
  fail "register envelope did not report the status registration"
fi
log "envelope OK: status ok, registered status"

# (2) The Dioxus-side mirror write ran with the test plugin.
if [ "$MIRRORED" != "1" ]; then
  log "registration mirror line(s):"
  grep -n "registration mirror" "$ART/app.log" || true
  fail "registration mirror never updated on the Dioxus side"
fi
if ! awk '/registration mirror updated/ && /okt95-guard/ { found = 1 } END { exit !found }' "$ART/app.log"; then
  log "registration mirror line(s):"
  grep -n "registration mirror" "$ART/app.log" || true
  fail "mirror updated without okt95-guard"
fi
log "mirror write OK: okt95-guard registered"

# (3) NO panic. This is the assertion that catches a runtime-mirror regression:
# the panic is contained, so the surface stays healthy and only app.log shows,
# e.g. "thread 'tokio-rt-worker' panicked ... Must be called from inside a
# Dioxus runtime".
if grep -Eq "panicked at|Must be called from inside a Dioxus runtime" "$ART/app.log"; then
  log "offending log line(s):"
  grep -En "panicked at|Must be called from inside a Dioxus runtime" "$ART/app.log" || true
  fail "panic detected in app.log (bridge-mirror regression)"
fi
log "no panic in app.log"

log "PASS"
