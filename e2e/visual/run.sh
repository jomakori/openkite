#!/usr/bin/env bash
# Capture fresh screenshots of all 10 surfaces and compare against baselines.
# This is the visual regression gate used in CI.
#
# Boots the app under Xvfb (same as visual/capture.sh), navigates to all
# 10 surfaces, saves fresh screenshots to a temp dir, then compares each
# against the committed baselines in e2e/visual/baselines/ using ImageMagick compare.
#
# Usage: e2e/visual/run.sh <path-to-openkite-binary> <baselines-dir> <artifact-dir>

set -euo pipefail

BIN="${1:?path to openkite binary}"
BASELINES="${2:?baselines dir}"
ART="${3:?artifact dir}"
mkdir -p "$ART"

FRESH="$ART/fresh"
mkdir -p "$FRESH"

# Reuse capture.sh to produce the 10 fresh screenshots.
log() { echo "[visual-regression-gate] $*"; }

log "capturing fresh screenshots"
"$(dirname "$0")/capture.sh" "$BIN" "$FRESH"

log "comparing against baselines"
"$(dirname "$0")/compare.sh" "$BASELINES" "$FRESH"
