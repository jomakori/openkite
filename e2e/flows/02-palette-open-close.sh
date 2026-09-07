#!/usr/bin/env bash
# Flow 02: Palette opens with Ctrl+P and closes with Escape.
#
# The palette keybind (document::eval JS listener) toggles on Ctrl+P.
# Opening the palette overlay must measurably change the screen (the
# centered modal + backdrop appears). Closing with Escape should return
# close to the pre-palette baseline.
set -euo pipefail
FLOW_NAME="flow-02-palette-open-close"
source "$(dirname "$0")/lib.sh"

flow_setup
focus_window

# Baseline screenshot
shot "01-baseline.png"
assert_rendered "$ART/01-baseline.png" "baseline"

# Open palette with Ctrl+P
flow_log "opening palette with Ctrl+P"
xdotool key --clearmodifiers ctrl+p
sleep 2
shot "02-palette-open.png"
assert_rendered "$ART/02-palette-open.png" "palette-open"
assert_pixels_changed "$ART/01-baseline.png" "$ART/02-palette-open.png" "palette-open" 500

# Close palette with Escape
flow_log "closing palette with Escape"
xdotool key --clearmodifiers Escape
sleep 2
shot "03-palette-closed.png"
assert_pixels_unchanged "$ART/01-baseline.png" "$ART/03-palette-closed.png" "palette-closed" 500

flow_log "PASS"
