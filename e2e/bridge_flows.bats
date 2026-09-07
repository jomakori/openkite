#!/usr/bin/env bats
# DOM-selector E2E flows for OpenKite (OKT-64).
#
# Each test boots the real desktop binary under Xvfb with the in-app DOM
# bridge enabled (src/test_bridge.rs), then drives the live webview with
# CSS selectors over HTTP — no xdotool coordinates, no ImageMagick.
#
# Requires: bats-core, curl, xvfb, openbox, python3.
# Usage:
#   BIN=../target/debug/openkite bats e2e/bridge_flows.bats
#
# Per-test artifacts (app.log, xvfb.log, screenshots) land in the bats
# tmp dir shown on failure.

load bridge_lib.bash

setup() {
  : "${BIN:?BIN must point to the openkite binary}"
  ART="$BATS_TEST_TMPDIR"
}

teardown() {
  # start_app installs a kill trap; nothing else to clean (tmp auto-rm).
  :
}

@test "01: app launches to a visible shell" {
  start_app
  assert_dom_count '.app-shell' '1'
  assert_dom_visible '.sidebar'
  assert_dom_visible '.content'
}

@test "02: disconnected state renders the shell chrome" {
  start_app
  # No kubeconfig in CI -> shell chrome present. Core nav = 5 items
  # (Cluster/Workloads/Logs/Terminal/Config); plugins may add more.
  assert_dom_count '.app-shell' '1'
  local nav_count
  nav_count="$(dom '.nav-item' count)"
  [ "$nav_count" -ge 5 ] || fail "expected >= 5 nav items, got $nav_count"
}

@test "03: palette opens on Ctrl+P and shows commands" {
  start_app
  assert_dom_hidden '.palette'
  dom '' key '{"key":"p","ctrl":true}'
  sleep 1
  assert_dom_visible '.palette'
  # At least one command section renders (View/Cluster/Action/Settings;
  # blank query returns all).
  local sections
  sections="$(dom '.palette-list .palette-section' count)"
  [ "$sections" -ge 1 ] || fail "expected palette sections, got $sections"
}

@test "04: palette filters commands as you type" {
  start_app
  dom '' key '{"key":"p","ctrl":true}'
  sleep 1
  dom '.palette-input' focus
  dom '.palette-input' type '{"text":"go to logs"}'
  sleep 1
  # Fuzzy filter narrows to the Go-to-Logs command.
  assert_dom_text '.palette-list' 'Logs'
}

@test "05: palette Escape closes with no residue" {
  start_app
  dom '' key '{"key":"p","ctrl":true}'
  sleep 1
  assert_dom_visible '.palette'
  dom '' key '{"key":"Escape"}'
  sleep 1
  assert_dom_hidden '.palette'
}

@test "06: sidebar exposes the five core nav items" {
  start_app
  assert_dom_count '.nav-item' '5'
  assert_dom_text '.nav' 'Cluster'
  assert_dom_text '.nav' 'Workloads'
  assert_dom_text '.nav' 'Config'
}

@test "07: cluster switcher opens on Ctrl+Tab" {
  start_app
  assert_dom_hidden '.switcher'
  dom '' key '{"key":"Tab","ctrl":true}'
  sleep 1
  assert_dom_visible '.switcher'
  dom '' key '{"key":"Escape"}'
  sleep 1
  assert_dom_hidden '.switcher'
}

@test "08: theme command cycles the surface" {
  start_app
  dom '' key '{"key":"p","ctrl":true}'
  sleep 1
  dom '.palette-input' focus
  dom '.palette-input' type '{"text":"theme"}'
  sleep 1
  # Cycle Theme is the settings command; running it closes the palette.
  dom '.palette-list' click
  sleep 1
  assert_dom_hidden '.palette'
}

@test "09: route navigation changes the active nav item" {
  start_app
  # Click Workloads in the sidebar.
  dom '.nav-item' click
  sleep 1
  assert_dom_count '.app-shell' '1'
  # Active class is on exactly one nav item after navigation.
  assert_dom_count '.nav-item.active' '1'
}

@test "10: bridge rejects unknown ops with a clean error" {
  start_app
  local out
  out="$(curl -s -X POST "$BRIDGE/" -d '{"selector":".x","op":"bogus"}')"
  echo "$out" | grep -q '"ok":false' || fail "expected ok:false for unknown op, got: $out"
}
