#!/usr/bin/env bash
# Flow 10: Cluster switcher opens with Ctrl+Tab and Escape-closes clean.
#
# The ClusterSwitcher overlay (src/switcher.rs) is a webview Ctrl+Tab /
# Escape keybind mounted by the app shell. In disconnected mode it shows
# the "no matching context" empty state — still a visible overlay that
# must open and dismiss cleanly.
set -euo pipefail
FLOW_NAME="flow-10-switcher-open-close"
source "$(dirname "$0")/lib.sh"

flow_setup
focus_window

shot "01-baseline.png"
assert_rendered "$ART/01-baseline.png" "baseline"

flow_log "opening cluster switcher with Ctrl+Tab"
xdotool key --clearmodifiers ctrl+Tab
sleep 2
shot "02-switcher-open.png"
assert_rendered "$ART/02-switcher-open.png" "switcher-open"
assert_pixels_changed "$ART/01-baseline.png" "$ART/02-switcher-open.png" "switcher-opened" 300

flow_log "closing switcher with Escape"
xdotool key --clearmodifiers Escape
sleep 2
shot "03-switcher-closed.png"
assert_pixels_unchanged "$ART/01-baseline.png" "$ART/03-switcher-closed.png" "switcher-closed" 500

flow_log "PASS"
