#!/usr/bin/env bash
# Gate G3 — cross-host parity: computed classes and token values, per core
# route on the desktop and the browser host. Pixels are graded elsewhere
# (`e2e/visual`, G2); this gate grades what the design contract is written in.
#
# Usage: e2e/parity/run.sh <openkite-desktop-binary> <artifact-dir>
#
# Required environment:
#   OPENKITE_WEB_BIN   built openkite-web executable
#   OPENKITE_WEB_ROOT  directory holding openkite-web-client.js and
#                      openkite-web-client_bg.wasm (the wasm-bindgen output)
# Optional:
#   OPENKITE_BROWSER        Chromium/Chrome executable (auto-detected if unset)
#   OPENKITE_PARITY_SETTLE_MS  post-mount settle before each snapshot (1200)
#   OPENKITE_ROUTE_TIMEOUT     per-route desktop timeout in seconds (120)
#
# Both hosts are snapshotted at the desktop window's frame (800x600): the gate
# requires the same breakpoint band, not the same pixel size.

set -euo pipefail

usage() {
  sed -n '2,17p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi
if [[ $# -ne 2 ]]; then
  usage >&2
  exit 2
fi

BIN="$(cd -- "$(dirname -- "$1")" && pwd)/$(basename -- "$1")"
ART="$2"
mkdir -p "$ART"

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"

log() { echo "[parity-gate] $*"; }
fail() { log "FAIL: $*"; exit 1; }

# --- prerequisites -------------------------------------------------------
[[ -x "$BIN" ]] || fail "desktop binary is not executable: $BIN"
for name in OPENKITE_WEB_BIN OPENKITE_WEB_ROOT; do
  [[ -n "${!name:-}" ]] || fail "$name must name the built web host / hydration asset directory"
done
[[ -x "$OPENKITE_WEB_BIN" ]] || fail "OPENKITE_WEB_BIN is not executable: $OPENKITE_WEB_BIN"
for asset in openkite-web-client.js openkite-web-client_bg.wasm; do
  [[ -f "$OPENKITE_WEB_ROOT/$asset" ]] || fail "OPENKITE_WEB_ROOT is missing $asset: $OPENKITE_WEB_ROOT"
done
command -v node >/dev/null 2>&1 || fail "node is required"
if (( $(node -p 'Number(process.versions.node.split(".")[0])') < 22 )); then
  fail "Node.js 22+ required (built-in WebSocket); found $(node --version)"
fi
command -v Xvfb >/dev/null 2>&1 || fail "Xvfb is required to run the desktop host headless"
command -v openbox >/dev/null 2>&1 || fail "openbox is required (WebKit needs an activated window)"

if [[ -z "${OPENKITE_BROWSER:-}" ]]; then
  for candidate in chromium chromium-browser google-chrome google-chrome-stable chrome; do
    if command -v "$candidate" >/dev/null 2>&1; then
      OPENKITE_BROWSER="$(command -v "$candidate")"
      break
    fi
  done
fi
[[ -n "${OPENKITE_BROWSER:-}" && -x "${OPENKITE_BROWSER:-}" ]] \
  || fail "Chromium/Chrome executable not found; set OPENKITE_BROWSER"
export OPENKITE_BROWSER OPENKITE_WEB_BIN OPENKITE_WEB_ROOT

DESKTOP_DIR="$ART/desktop"
WEB_DIR="$ART/web"
mkdir -p "$DESKTOP_DIR" "$WEB_DIR"

# --- desktop host: one boot per route -----------------------------------
# The desktop hook (crates/openkite-desktop/src/parity.rs) writes the snapshot
# and exits; a route that produces no file is a gate failure, never a skip.
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export LIBGL_ALWAYS_SOFTWARE=1

DISPLAY_NUM=":98"
SCREEN="1280x800x24"
log "starting Xvfb on $DISPLAY_NUM ($SCREEN)"
Xvfb "$DISPLAY_NUM" -screen 0 "$SCREEN" -nolisten tcp >"$ART/xvfb.log" 2>&1 &
XVFB_PID=$!
trap 'kill $XVFB_PID ${WM_PID:-} 2>/dev/null || true' EXIT
sleep 1
export DISPLAY="$DISPLAY_NUM"
log "starting openbox"
openbox >"$ART/wm.log" 2>&1 &
WM_PID=$!
sleep 1

ROUTE_TIMEOUT="${OPENKITE_ROUTE_TIMEOUT:-120}"
for route in / /cluster /workloads /config; do
  slug="${route#/}"
  slug="${slug:-home}"
  snapshot="$DESKTOP_DIR/$slug.desktop.json"
  rm -f "$snapshot"
  log "desktop: $route"
  set +e
  OPENKITE_ROUTE="$route" \
  OPENKITE_PARITY_SNAPSHOT="$DESKTOP_DIR" \
  OPENKITE_PARITY_SETTLE_MS="${OPENKITE_PARITY_SETTLE_MS:-1200}" \
    timeout "$ROUTE_TIMEOUT" "$BIN" >>"$ART/desktop.log" 2>&1
  status=$?
  set -e
  if [[ ! -s "$snapshot" ]]; then
    log "desktop host produced no snapshot for $route (exit $status); log tail:"
    tail -40 "$ART/desktop.log" >&2 || true
    fail "missing $snapshot"
  fi
  log "desktop: $route -> $(basename "$snapshot") (exit $status)"
done

# --- browser host: one capture run, all routes --------------------------
log "browser: $(timeout 10 "$OPENKITE_BROWSER" --version 2>&1 | head -1)"
SNAPSHOT_DIR="$WEB_DIR" SNAPSHOT_VIEWPORT="${SNAPSHOT_VIEWPORT:-800x600}" \
  node "$repo_root/e2e/visual/browser/capture.mjs" "$WEB_DIR/shots" \
  || fail "browser host snapshot run failed"

# --- grade ---------------------------------------------------------------
log "grading computed classes and token values"
node "$script_dir/compare.mjs" \
  --desktop "$DESKTOP_DIR" \
  --web "$WEB_DIR" \
  --allowlist "$script_dir/allowlist.json" \
  --report "$ART/parity-report.json"
