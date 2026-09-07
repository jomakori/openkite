#!/usr/bin/env bash
# Flow 03: Palette filters the command list.
#
# Typing a query into the open palette filters commands by fuzzy rank
# against the label (src/palette.rs: filter_commands). Typing "work"
# should narrow to "Go to Workloads" (and any other match) — the screen
# must change from the unfiltered palette to the filtered view.
set -euo pipefail
FLOW_NAME="flow-03-palette-filter"
source "$(dirname "$0")/lib.sh"

flow_setup
focus_window

# Open palette
flow_log "opening palette with Ctrl+P"
xdotool key --clearmodifiers ctrl+p
sleep 2
shot "01-palette-unfiltered.png"
assert_rendered "$ART/01-palette-unfiltered.png" "palette-unfiltered"

# Type a filter query
flow_log "typing filter query 'work'"
xdotool type --delay 50 "work"
sleep 2
shot "02-palette-filtered.png"
assert_rendered "$ART/02-palette-filtered.png" "palette-filtered"

# The filtered view must differ from the unfiltered one (fewer items = different layout).
assert_pixels_changed "$ART/01-palette-unfiltered.png" "$ART/02-palette-filtered.png" "palette-filter" 500

# Close palette
xdotool key --clearmodifiers Escape
sleep 1

flow_log "PASS"
