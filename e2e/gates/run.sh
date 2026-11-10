#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: e2e/gates/run.sh [artifact-dir]

Run the three gates OKT-194 grades, against the browser host's own DOM:

  G4  every data surface renders an empty, a loading and an error state
      (driven by ?fixture=, read back from data-state)
  G5  every interactive target is at least 44x44 CSS px at 767px or below
      (measured from getBoundingClientRect() at 390 and 767 wide)
  G6  no literal hex in the surface rules and prefers-reduced-motion zeroes
      transitions (stylesheet read as text, then confirmed in the browser)

Required environment:
  OPENKITE_WEB_BIN   built openkite-web executable
  OPENKITE_WEB_ROOT  directory with openkite-web-client.js and
                     openkite-web-client_bg.wasm (the wasm-bindgen output)
Optional:
  OPENKITE_BROWSER   Chromium/Chrome executable (auto-detected if unset)

Exits 0 only when every gate passes; the JSON report lands in the artifact
directory (default: e2e/artifacts/gates).
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi
if [[ $# -gt 1 ]]; then
  usage >&2
  exit 2
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ART="${1:-$(cd -- "$script_dir/.." && pwd)/artifacts/gates}"
mkdir -p "$ART"
ART="$(cd -- "$ART" && pwd)"

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

log() { echo "[ui-state-gates] $*"; }

log "browser: $OPENKITE_BROWSER ($(timeout 10 "$OPENKITE_BROWSER" --version 2>&1 | head -1))"
log "G6 static: reading the stylesheet"
node "$script_dir/check-css.mjs"
log "G4/G5/G6: driving the browser host's DOM"
node "$script_dir/gates.mjs" "$ART"
log "report: $ART/report.json"
