#!/usr/bin/env bash
# Flow 09: Palette opens from a non-home route.
#
# Navigate to Cluster via the palette (Go to Cluster), then open the
# palette there — proves the keybind works on any route (not just the
# initial one) and that overlay state is route-independent. Uses palette
# commands for navigation (deterministic) rather than sidebar coordinate
# clicks; the overlay assertion is the same palette-open pixel signal
# as flow 02.
set -euo pipefail
FLOW_NAME="flow-09-palette-route-overlay"
source "$(dirname "$0")/lib.sh"

flow_setup
focus_window

flow_log "navigating to Cluster via palette command"
xdotool key --clearmodifiers ctrl+p
sleep 2
xdotool type --delay 60 "go to cluster"
sleep 2
xdotool key --clearmodifiers Return
sleep 3
shot "01-cluster.png"
assert_rendered "$ART/01-cluster.png" "cluster-route"

flow_log "opening palette from Cluster route"
xdotool key --clearmodifiers ctrl+p
sleep 2
shot "02-palette-on-cluster.png"
assert_rendered "$ART/02-palette-on-cluster.png" "palette-on-cluster"
assert_pixels_changed "$ART/01-cluster.png" "$ART/02-palette-on-cluster.png" "palette-overlay-on-route" 500

flow_log "closing palette"
xdotool key --clearmodifiers Escape
sleep 2
shot "03-cluster-restored.png"
assert_pixels_unchanged "$ART/01-cluster.png" "$ART/03-cluster-restored.png" "cluster-restored" 500

flow_log "PASS"
