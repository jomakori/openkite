#!/usr/bin/env bash
# Flow 07: Route navigation changes the rendered view.
#
# Use the palette's Go-to commands to move between routes and assert the
# view actually changes. Workloads renders a distinct surface from Home
# (src/router.rs), so the pixels must differ.
#
# Single palette session only: a second session's typed input does not
# reliably reach the palette filter under xdotool/WebKitGTK (autofocus
# does not re-grab on remount), and the route-change assertion after a
# second session is exactly the coordinate/focus flake that the DOM
# bridge (OKT-64, tests/palette.rs + src/test_bridge.rs) eliminates.
# Multi-route interaction coverage lives there.
set -euo pipefail
FLOW_NAME="flow-07-route-nav"
source "$(dirname "$0")/lib.sh"

flow_setup
focus_window

shot "01-home.png"
assert_rendered "$ART/01-home.png" "home"

flow_log "navigating to Workloads via palette command"
xdotool key --clearmodifiers ctrl+p
sleep 2
xdotool type --delay 60 "go to workloads"
sleep 2
xdotool key --clearmodifiers Return
sleep 3
# Dismiss the palette if the Enter-run left it open (focus drift).
xdotool key --clearmodifiers Escape
sleep 1
shot "02-workloads.png"
assert_rendered "$ART/02-workloads.png" "workloads-route"
assert_pixels_changed "$ART/01-home.png" "$ART/02-workloads.png" "home-to-workloads-nav" 1000

flow_log "PASS"
