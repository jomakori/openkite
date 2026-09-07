#!/usr/bin/env bash
# Flow 01: App launches to a visible window.
#
# Verifies the core launch path: the binary starts, a window titled
# "OpenKite" appears on the X display, and the rendered content is
# non-blank (has sidebar, content area, status footer).
set -euo pipefail
FLOW_NAME="flow-01-launch"
source "$(dirname "$0")/lib.sh"

flow_setup
focus_window

shot "01-shell.png"
assert_rendered "$ART/01-shell.png" "shell"

# Disconnected mode: no kubeconfig in CI → app.log confirms it.
assert_disconnected

flow_log "PASS"
