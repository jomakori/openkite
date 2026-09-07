#!/usr/bin/env bats
# OKT-61 user-flow E2E suite (10 scenarios).
#
# Boots the REAL binary under Xvfb per test and drives X11 input with
# xdotool, asserting via screenshots + pixel diffs (see flows_lib.bash).
# bats provides structure, --filter, TAP/JUnit output for the dorny
# report in the GHA job summary — the shell-file alternative (#80's
# original run-flows.sh + flows/*.sh) was dropped per user decision:
# bats-core over bespoke shell scripts.
#
# Run:  BIN=../target/debug/openkite ART_ROOT=artifacts-flows bats user_flows.bats
# CI:   e2e.yml user-flows job (installs bats, feeds JUnit to dorny).

load flows_lib

setup() {
  flow_setup
  focus_window
}

teardown() {
  flow_teardown
}

@test "flow 01: app launches to a visible, non-blank window" {
  shot "01-shell.png"
  assert_rendered "$ART/01-shell.png" "shell"
  # Hermetic CI: no kubeconfig → disconnected mode confirmed in app.log.
  assert_disconnected
}

@test "flow 02: palette opens with Ctrl+P and closes with Escape" {
  shot "01-baseline.png"
  assert_rendered "$ART/01-baseline.png" "baseline"

  xdotool key --clearmodifiers ctrl+p
  sleep 2
  shot "02-palette-open.png"
  assert_rendered "$ART/02-palette-open.png" "palette-open"
  assert_pixels_changed "$ART/01-baseline.png" "$ART/02-palette-open.png" "palette-open" 500

  xdotool key --clearmodifiers Escape
  sleep 2
  shot "03-palette-closed.png"
  assert_pixels_unchanged "$ART/01-baseline.png" "$ART/03-palette-closed.png" "palette-closed" 500
}

@test "flow 03: palette filters the command list" {
  xdotool key --clearmodifiers ctrl+p
  sleep 2
  shot "01-palette-unfiltered.png"
  assert_rendered "$ART/01-palette-unfiltered.png" "palette-unfiltered"

  xdotool type --delay 50 "work"
  sleep 2
  shot "02-palette-filtered.png"
  assert_rendered "$ART/02-palette-filtered.png" "palette-filtered"
  assert_pixels_changed "$ART/01-palette-unfiltered.png" "$ART/02-palette-filtered.png" "palette-filter" 500

  xdotool key --clearmodifiers Escape
  sleep 1
}

@test "flow 04: palette action navigates to a route" {
  shot "01-home.png"
  assert_rendered "$ART/01-home.png" "home"

  xdotool key --clearmodifiers ctrl+p
  sleep 2
  shot "02-palette.png"
  assert_rendered "$ART/02-palette.png" "palette-open"

  xdotool type --delay 60 "logs"
  sleep 2
  shot "03-filtered.png"
  assert_rendered "$ART/03-filtered.png" "palette-filtered"
  assert_pixels_changed "$ART/02-palette.png" "$ART/03-filtered.png" "filter-narrowed" 300

  xdotool key --clearmodifiers Return
  sleep 3
  shot "04-after-nav.png"
  assert_rendered "$ART/04-after-nav.png" "after-navigation"
  assert_pixels_changed "$ART/01-home.png" "$ART/04-after-nav.png" "route-changed" 1000
}

@test "flow 05: palette Cycle Theme changes the surface" {
  shot "01-before.png"
  assert_rendered "$ART/01-before.png" "before-theme"

  xdotool key --clearmodifiers ctrl+p
  sleep 2
  shot "02-palette.png"
  assert_rendered "$ART/02-palette.png" "palette-open"

  xdotool type --delay 60 "theme"
  sleep 2
  shot "03-filtered.png"
  assert_rendered "$ART/03-filtered.png" "filtered"

  xdotool key --clearmodifiers Return
  sleep 3
  shot "04-after-theme.png"
  assert_rendered "$ART/04-after-theme.png" "after-theme"
  assert_pixels_changed "$ART/01-before.png" "$ART/04-after-theme.png" "theme-cycled" 1000
}

@test "flow 06: palette Escape returns to baseline (clean round-trip)" {
  shot "01-baseline.png"
  assert_rendered "$ART/01-baseline.png" "baseline"

  xdotool key --clearmodifiers ctrl+p
  sleep 2
  shot "02-palette-open.png"
  assert_rendered "$ART/02-palette-open.png" "palette-open"
  assert_pixels_changed "$ART/01-baseline.png" "$ART/02-palette-open.png" "palette-opened" 500

  xdotool key --clearmodifiers Escape
  sleep 2
  shot "03-palette-closed.png"
  assert_pixels_unchanged "$ART/01-baseline.png" "$ART/03-palette-closed.png" "roundtrip-clean" 500
}

@test "flow 07: palette Go-to-Workloads changes the route view" {
  shot "01-home.png"
  assert_rendered "$ART/01-home.png" "home"

  # Single palette session only (WebKitGTK autofocus does not re-grab on
  # remount; a second session's typing may never reach the filter).
  # Multi-session route round-trips are covered headlessly in
  # tests/palette.rs instead.
  xdotool key --clearmodifiers ctrl+p
  sleep 2
  xdotool type --delay 60 "go to workloads"
  sleep 2
  xdotool key --clearmodifiers Return
  sleep 3
  xdotool key --clearmodifiers Escape
  sleep 1
  shot "02-workloads.png"
  assert_rendered "$ART/02-workloads.png" "workloads-route"
  assert_pixels_changed "$ART/01-home.png" "$ART/02-workloads.png" "home-to-workloads-nav" 1000
}

@test "flow 08: disconnected-state banner renders without kubeconfig" {
  assert_disconnected

  shot "01-shell.png"
  assert_rendered "$ART/01-shell.png" "disconnected-shell"

  # Palette still opens offline (host-side commands).
  xdotool key --clearmodifiers ctrl+p
  sleep 2
  shot "02-palette.png"
  assert_rendered "$ART/02-palette.png" "palette-in-disconnected"
  assert_pixels_changed "$ART/01-shell.png" "$ART/02-palette.png" "palette-works-offline" 500

  xdotool key --clearmodifiers Escape
  sleep 1
}

@test "flow 09: palette opens from a non-home route (Cluster)" {
  xdotool key --clearmodifiers ctrl+p
  sleep 2
  xdotool type --delay 60 "go to cluster"
  sleep 2
  xdotool key --clearmodifiers Return
  sleep 3
  shot "01-cluster.png"
  assert_rendered "$ART/01-cluster.png" "cluster-route"

  xdotool key --clearmodifiers ctrl+p
  sleep 2
  shot "02-palette-on-cluster.png"
  assert_rendered "$ART/02-palette-on-cluster.png" "palette-on-cluster"
  assert_pixels_changed "$ART/01-cluster.png" "$ART/02-palette-on-cluster.png" "palette-overlay-on-route" 500

  xdotool key --clearmodifiers Escape
  sleep 2
  shot "03-cluster-restored.png"
  assert_pixels_unchanged "$ART/01-cluster.png" "$ART/03-cluster-restored.png" "cluster-restored" 500
}

@test "flow 10: cluster switcher opens with Ctrl+Tab and Escape-closes clean" {
  shot "01-baseline.png"
  assert_rendered "$ART/01-baseline.png" "baseline"

  xdotool key --clearmodifiers ctrl+Tab
  sleep 2
  shot "02-switcher-open.png"
  assert_rendered "$ART/02-switcher-open.png" "switcher-open"
  assert_pixels_changed "$ART/01-baseline.png" "$ART/02-switcher-open.png" "switcher-opened" 300

  xdotool key --clearmodifiers Escape
  sleep 2
  shot "03-switcher-closed.png"
  assert_pixels_unchanged "$ART/01-baseline.png" "$ART/03-switcher-closed.png" "switcher-closed" 500
}
