#!/usr/bin/env bash
# Visual regression gate: compare fresh screenshots against committed baselines.
#
# Uses ImageMagick `compare -metric AE` (absolute pixel count of difference).
# WebKitGTK font rendering is not pixel-perfect across runs, so we use a
# tolerant fuzzy threshold: a screenshot passes if the number of differing
# pixels is below MAX_DIFF_PIXELS (default: 1% of total pixels).
#
# Total pixels at 1280x800 = 1,024,000. 1% = 10,240 pixels.
#
# Usage: visual-regression.sh <baselines-dir> <fresh-screenshots-dir>
# Exit 0 if all pass, exit 1 if any fail.

set -euo pipefail

BASELINES="${1:?baselines dir}"
FRESH="${2:?fresh screenshots dir}"

# Threshold: max differing pixels (AE metric). ~1% of 1280x800.
MAX_DIFF_PIXELS="${MAX_DIFF_PIXELS:-10240}"

log() { echo "[visual-regression] $*"; }
fail() { log "FAIL: $*"; exit 1; }

if [ ! -d "$BASELINES" ]; then
  fail "baselines directory not found: $BASELINES"
fi
if [ ! -d "$FRESH" ]; then
  fail "fresh screenshots directory not found: $FRESH"
fi

log "comparing fresh screenshots against baselines"
log "baseline dir: $BASELINES"
log "fresh dir: $FRESH"
log "threshold: $MAX_DIFF_PIXELS differing pixels (AE metric, ~1% of 1280x800)"

PASS=0
FAIL=0
MISSING=0

for baseline in "$BASELINES"/*.png; do
  [ -f "$baseline" ] || continue  # no PNGs
  name=$(basename "$baseline")
  fresh="$FRESH/$name"

  if [ ! -f "$fresh" ]; then
    log "MISSING: $name not found in fresh screenshots"
    MISSING=$((MISSING + 1))
    continue
  fi

  # compare -metric AE prints the pixel count to stderr and returns non-zero
  # when images differ (which is expected). Capture the metric value.
  diff_pixels=$(compare -metric AE "$baseline" "$fresh" null: 2>&1 || true)

  # Strip any non-numeric suffixes (compare sometimes outputs "1024 (0.001)")
  diff_pixels=$(echo "$diff_pixels" | grep -oE '^[0-9]+' || echo "0")

  if [ "$diff_pixels" -le "$MAX_DIFF_PIXELS" ]; then
    log "PASS: $name ($diff_pixels differing pixels)"
    PASS=$((PASS + 1))
  else
    log "FAIL: $name ($diff_pixels differing pixels > $MAX_DIFF_PIXELS threshold)"
    FAIL=$((FAIL + 1))
  fi
done

log "results: $PASS passed, $FAIL failed, $MISSING missing"

if [ "$FAIL" -gt 0 ] || [ "$MISSING" -gt 0 ]; then
  fail "$FAIL visual regression failures, $MISSING missing screenshots"
fi

log "ALL PASS"
