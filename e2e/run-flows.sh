#!/usr/bin/env bash
# Run all user-flow E2E scenarios sequentially against one binary build.
#
# Each flow boots its own Xvfb + openbox + app instance (see flows/lib.sh),
# so a failure is isolated to that flow and the run continues to the next.
# Every flow's screenshots/logs land in a per-flow artifact subdirectory.
#
# Usage: run-flows.sh <path-to-openkite-binary> <artifact-root>
#   Optional env: EXPECT_CONNECTED=1 to assert the app joined the cluster.
#
# Exit 0 only if every flow passed; otherwise lists the failures.

set -euo pipefail

BIN="${1:?path to openkite binary}"
ART_ROOT="${2:?artifact root dir}"
mkdir -p "$ART_ROOT"

FLOW_DIR="$(cd "$(dirname "$0")" && pwd)/flows"

log() { echo "[run-flows] $*"; }
fail() { echo "[run-flows] FAIL: $*"; exit 1; }

[ -d "$FLOW_DIR" ] || fail "flows dir not found: $FLOW_DIR"
[ -x "$BIN" ] || fail "binary not executable: $BIN"

shopt -s nullglob
FLOWS=("$FLOW_DIR"/*.sh)
shopt -u nullglob
[ "${#FLOWS[@]}" -gt 0 ] || fail "no flow scripts found in $FLOW_DIR"

log "found ${#FLOWS[@]} flows"

PASSED=0
FAILED=0
declare -a FAILURES=()

for flow in "${FLOWS[@]}"; do
  name=$(basename "$flow" .sh)
  art="$ART_ROOT/$name"
  log "=== running flow: $name ==="
  if ART="$art" BIN="$BIN" FLOW_NAME="$name" bash "$flow"; then
    log "flow $name PASSED"
    PASSED=$((PASSED + 1))
  else
    log "flow $name FAILED (artifacts: $art)"
    FAILED=$((FAILED + 1))
    FAILURES+=("$name")
  fi
done

log "results: $PASSED passed, $FAILED failed"
if [ "$FAILED" -gt 0 ]; then
  log "failed flows: ${FAILURES[*]}"
  exit 1
fi
log "ALL FLOWS PASSED"
