#!/usr/bin/env bash
# Flow 08: Disconnected-state banner renders when no kubeconfig.
#
# Hermetic CI runs have no kubeconfig. The app logs
# "no kubeconfig; starting disconnected" (src/lib.rs) and the shell
# renders a disconnected banner/status instead of cluster data. Assert
# the log line AND that the window still renders (app usable offline).
set -euo pipefail
FLOW_NAME="flow-08-disconnected-state"
source "$(dirname "$0")/lib.sh"

flow_setup
assert_disconnected

shot "01-shell.png"
assert_rendered "$ART/01-shell.png" "disconnected-shell"

# Even disconnected, the palette must still open (host-side commands).
flow_log "opening palette in disconnected state"
focus_window
xdotool key --clearmodifiers ctrl+p
sleep 2
shot "02-palette.png"
assert_rendered "$ART/02-palette.png" "palette-in-disconnected"
assert_pixels_changed "$ART/01-shell.png" "$ART/02-palette.png" "palette-works-offline" 500

xdotool key --clearmodifiers Escape
sleep 1
flow_log "PASS"
