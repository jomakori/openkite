#!/usr/bin/env bash
# Flow 07: Sidebar route navigation changes the view.
#
# Click the "Cluster" nav item in the sidebar. The Cluster route renders
# a different surface than Home (cluster name/cards when connected, an
# empty-state panel when disconnected). Assert the view changed and the
# "Workloads" click moves on again.
set -euo pipefail
FLOW_NAME="flow-07-sidebar-nav"
source "$(dirname "$0")/lib.sh"

flow_setup
focus_window

shot "01-home.png"
assert_rendered "$ART/01-home.png" "home"

# Sidebar is ~200px wide on the left; nav items are vertical <a> links
# under the brand block. Click Cluster (~y 120), then Workloads (~y 160).
flow_log "clicking Cluster nav item"
xdotool mousemove 100 120 click 1
sleep 2
shot "02-cluster.png"
assert_rendered "$ART/02-cluster.png" "cluster-route"
assert_pixels_changed "$ART/01-home.png" "$ART/02-cluster.png" "cluster-nav" 1000

flow_log "clicking Workloads nav item"
xdotool mousemove 100 160 click 1
sleep 2
shot "03-workloads.png"
assert_rendered "$ART/03-workloads.png" "workloads-route"
assert_pixels_changed "$ART/02-cluster.png" "$ART/03-workloads.png" "workloads-nav" 1000

flow_log "PASS"
