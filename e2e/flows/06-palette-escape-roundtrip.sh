#!/usr/bin/env bash
# Flow 06: Palette Escape closes with a pixel-clean round-trip.
#
# Open palette (Ctrl+P) -> screen changes. Press Escape -> screen must
# return to within threshold of the pre-palette baseline (overlay fully
# dismissed, no residue). This is the strictest overlay assertion: a
# stuck backdrop or focus trap fails it.
set -euo pipefail
FLOW_NAME="flow-06-palette-escape-roundtrip"
source "$(dirname "$0")/lib.sh"

flow_setup
focus_window

shot "01-baseline.png"
assert_rendered "$ART/01-baseline.png" "baseline"

flow_log "opening palette with Ctrl+P"
xdotool key --clearmodifiers ctrl+p
sleep 2
shot "02-palette-open.png"
assert_rendered "$ART/02-palette-open.png" "palette-open"
assert_pixels_changed "$ART/01-baseline.png" "$ART/02-palette-open.png" "palette-opened" 500

flow_log "closing palette with Escape"
xdotool key --clearmodifiers Escape
sleep 2
shot "03-palette-closed.png"
assert_pixels_unchanged "$ART/01-baseline.png" "$ART/03-palette-closed.png" "roundtrip-clean" 500

flow_log "PASS"
