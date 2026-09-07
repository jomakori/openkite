#!/usr/bin/env bash
# Flow 04: Palette action navigates to a route.
#
# Open the palette (Ctrl+P), type a filter, select the top result with
# Enter, and assert the view changed (navigation happened). Uses the
# palette's "Go to Logs" command (src/palette.rs) — Logs renders a
# different surface than Home, so the screenshot must differ.
set -euo pipefail
FLOW_NAME="flow-04-palette-navigate"
source "$(dirname "$0")/lib.sh"

flow_setup
focus_window

shot "01-home.png"
assert_rendered "$ART/01-home.png" "home"

flow_log "opening palette with Ctrl+P"
xdotool key --clearmodifiers ctrl+p
sleep 2
shot "02-palette.png"
assert_rendered "$ART/02-palette.png" "palette-open"

flow_log "typing 'logs' to filter Go-to-Logs command"
xdotool type --delay 60 "logs"
sleep 2
shot "03-filtered.png"
assert_rendered "$ART/03-filtered.png" "palette-filtered"
assert_pixels_changed "$ART/02-palette.png" "$ART/03-filtered.png" "filter-narrowed" 300

flow_log "pressing Enter to run the selected command"
xdotool key --clearmodifiers Return
sleep 3
shot "04-after-nav.png"
assert_rendered "$ART/04-after-nav.png" "after-navigation"

# Navigating Home -> Logs must change the rendered view.
assert_pixels_changed "$ART/01-home.png" "$ART/04-after-nav.png" "route-changed" 1000

flow_log "PASS"
