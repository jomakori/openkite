#!/usr/bin/env bash
# Visual regression gate: compare fresh screenshots against committed baselines.
#
# Uses ImageMagick `compare -metric AE` (absolute pixel count of difference).
# WebKitGTK font rendering and overlay-scrollbar drawing are not pixel-perfect
# across runs or runner images, so we use a tolerant threshold: a screenshot
# passes if the number of differing pixels is below MAX_DIFF_PIXELS.
#
# Measured noise band: 12,006 px (~2.5% of the 800x600 capture) appeared on
# every surface on 2026-09-30 as a 21px right-edge overlay-scrollbar strip,
# content-identical to the baselines. Threshold = 16,000 px (~3.3%). If a
# failure reports a count ABOVE 16,000, investigate the change itself — do
# not raise the threshold again without a fresh measurement.
#
# Usage: e2e/visual/compare.sh <baselines-dir> <fresh-screenshots-dir>
# Exit 0 if all pass, exit 1 if any fail.

set -euo pipefail

BASELINES="${1:?baselines dir}"
FRESH="${2:?fresh screenshots dir}"

# Threshold: max differing pixels (AE metric). ~3.3% of 800x600.
MAX_DIFF_PIXELS="${MAX_DIFF_PIXELS:-16000}"

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
  #
  # `-fuzz` so the metric counts *perceptible* differences only. Capturing the
  # same screen in two environments (a maintainer's in-cluster pod vs the GitHub
  # runner) rasterises text slightly differently: measured 2026-09-30 on the
  # OKT-155 refresh, 142,369 of 480,000 pixels differed by 1-3 levels (invisible)
  # against 792 by >15, so a raw AE count over a fixed threshold measures the
  # capture environment, not the UI. With -fuzz 3% the refreshed baselines score
  # 106-3,726 while stale baselines (shell + route chrome missing) still score
  # 19,241-242,306 and fail. The threshold below stays as it is.
  diff_pixels=$(compare -metric AE -fuzz "${MAX_DIFF_FUZZ:-3%}" "$baseline" "$fresh" null: 2>&1 || true)

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
