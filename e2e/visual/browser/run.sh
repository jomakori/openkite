#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: e2e/visual/browser/run.sh <baselines-dir> <artifact-dir>

Capture fresh hydrated-client screenshots of the browser host (via
e2e/visual/browser/capture.mjs) at the three breakpoints the OpenDesign
mockup is graded at, then compare them against the committed baselines
with e2e/visual/compare.sh (ImageMagick).

Required environment:
  OPENKITE_WEB_BIN   built openkite-web executable
  OPENKITE_WEB_ROOT  directory with openkite-web-client.js and
                     openkite-web-client_bg.wasm (the wasm-bindgen output)
Optional:
  OPENKITE_BROWSER      Chromium/Chrome executable (auto-detected if unset)
  MAX_DIFF_PERCENT      per-image differing-pixel budget as a percent of
                        that image's own pixel count (default: see compare.sh)
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi
if [[ $# -ne 2 ]]; then
  usage >&2
  exit 2
fi

BASELINES="$1"
ART="$2"
mkdir -p "$ART"

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

for tool in node; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'Required tool not found: %s\n' "$tool" >&2
    exit 2
  }
done

node_major="$(node -p 'Number(process.versions.node.split(".")[0])')"
if ((node_major < 22)); then
  printf 'Node.js 22+ required for its built-in WebSocket; found %s\n' "$(node --version)" >&2
  exit 2
fi

for name in OPENKITE_WEB_BIN OPENKITE_WEB_ROOT; do
  if [[ -z "${!name:-}" ]]; then
    printf '%s must name the built web host / hydration asset directory\n' "$name" >&2
    exit 2
  fi
done
if [[ ! -x "$OPENKITE_WEB_BIN" ]]; then
  printf 'OPENKITE_WEB_BIN is not executable: %s\n' "$OPENKITE_WEB_BIN" >&2
  exit 2
fi
if [[ ! -f "$OPENKITE_WEB_ROOT/openkite-web-client.js" || ! -f "$OPENKITE_WEB_ROOT/openkite-web-client_bg.wasm" ]]; then
  printf 'OPENKITE_WEB_ROOT must contain openkite-web-client.js and openkite-web-client_bg.wasm: %s\n' "$OPENKITE_WEB_ROOT" >&2
  exit 2
fi

if [[ -z "${OPENKITE_BROWSER:-}" ]]; then
  for candidate in chromium chromium-browser google-chrome google-chrome-stable chrome; do
    if command -v "$candidate" >/dev/null 2>&1; then
      OPENKITE_BROWSER="$(command -v "$candidate")"
      break
    fi
  done
fi
if [[ -z "${OPENKITE_BROWSER:-}" || ! -x "$OPENKITE_BROWSER" ]]; then
  printf 'Chromium/Chrome executable not found; set OPENKITE_BROWSER to a runner-provided browser. No browser was installed.\n' >&2
  exit 2
fi
export OPENKITE_BROWSER OPENKITE_WEB_BIN OPENKITE_WEB_ROOT

log() { echo "[visual-regression-browser-gate] $*"; }

FRESH="$ART/fresh"
log "browser: $OPENKITE_BROWSER ($(timeout 10 "$OPENKITE_BROWSER" --version 2>&1 | head -1))"
log "capturing fresh hydrated-client screenshots"
node "$script_dir/capture.mjs" "$FRESH"

log "comparing against baselines"
# Per-image percent budget; compare.sh's fixed-pixel default is sized for the
# desktop lane's single viewport.
MAX_DIFF_PERCENT="${MAX_DIFF_PERCENT:-1.5}" "$script_dir/../compare.sh" "$BASELINES" "$FRESH"
