#!/usr/bin/env bash
# Flow 05: Theme toggle cycles through the opaline theme catalog.
#
# Open the palette, type "theme", press Enter to run "Cycle Theme"
# (src/palette.rs settings.theme). The surface accent/background must
# change. Cycling again eventually returns to the original, so we only
# assert the first cycle changes the screen.
set -euo pipefail
FLOW_NAME="flow-05-theme-cycle"
source "$(dirname "$0")/lib.sh"

flow_setup
focus_window

shot "01-before.png"
assert_rendered "$ART/01-before.png" "before-theme"

flow_log "opening palette"
xdotool key --clearmodifiers ctrl+p
sleep 2
shot "02-palette.png"
assert_rendered "$ART/02-palette.png" "palette-open"

flow_log "typing 'theme' to filter Cycle Theme"
xdotool type --delay 60 "theme"
sleep 2
shot "03-filtered.png"
assert_rendered "$ART/03-filtered.png" "filtered"

flow_log "pressing Enter to run Cycle Theme"
xdotool key --clearmodifiers Return
sleep 3
shot "04-after-theme.png"
assert_rendered "$ART/04-after-theme.png" "after-theme"

# Theme switch must visibly change the surface (accent/background).
assert_pixels_changed "$ART/01-before.png" "$ART/04-after-theme.png" "theme-cycled" 1000

flow_log "PASS"
