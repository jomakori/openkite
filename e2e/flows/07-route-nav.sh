#!/usr/bin/env bash
# Flow 07: Route navigation changes the rendered view.
#
# Use the palette's Go-to commands to move between routes and assert the
# view actually changes. Logs renders a distinct surface from Home
# (src/router.rs), so the pixels must differ; returning Home must restore
# the baseline. (Sidebar coordinate clicks are deliberately avoided —
# see e2e/README.md; navigation is asserted through palette commands,
# the same deterministic primitive as flow 04.)
set -euo pipefail
FLOW_NAME="flow-07-route-nav"
source "$(dirname "$0")/lib.sh"

flow_setup
focus_window

shot "01-home.png"
assert_rendered "$ART/01-home.png" "home"

flow_log "navigating to Logs via palette command"
xdotool key --clearmodifiers ctrl+p
sleep 2
xdotool type --delay 60 "go to logs"
sleep 2
xdotool key --clearmodifiers Return
sleep 3
shot "02-logs.png"
assert_rendered "$ART/02-logs.png" "logs-route"
assert_pixels_changed "$ART/01-home.png" "$ART/02-logs.png" "home-to-logs-nav" 1000

flow_log "navigating back Home via palette command"
xdotool key --clearmodifiers ctrl+p
sleep 2
xdotool type --delay 60 "go to home"
sleep 2
xdotool key --clearmodifiers Return
sleep 3
# Dismiss the palette if the Enter-run left it open (focus can drift
# between sessions).
xdotool key --clearmodifiers Escape
sleep 1
shot "03-home.png"
assert_rendered "$ART/03-home.png" "home-returned"
# Navigating Logs -> Home must change the view again (leaves Logs).
assert_pixels_changed "$ART/02-logs.png" "$ART/03-home.png" "logs-to-home-nav" 1000

flow_log "PASS"
