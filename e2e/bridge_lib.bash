# Shared helpers for bats-based DOM E2E tests (OKT-64).
#
# Each bats @test boots its own app instance under Xvfb with the test
# bridge enabled (OPENKITE_TEST_PORT), waits for the window + bridge,
# then drives the DOM via curl against the bridge. Per-test artifacts
# land in $BATS_TEST_TMPDIR.

# Port for the bridge. Per-test override via PORT is supported for
# parallelism; otherwise a fixed default keeps logs greppable.
BRIDGE_PORT="${PORT:-39871}"
BRIDGE="http://127.0.0.1:${BRIDGE_PORT}"

# bin/curl helpers ------------------------------------------------------

# dom <selector> <op> [extra-json] -> prints .result (raw)
# Example: dom '.palette' count   -> 1
dom() {
  local selector="$1" op="$2" extra="${3:-}"
  local payload
  payload="$(printf '{"selector":%s,"op":"%s"%s}' \
    "$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$selector")" \
    "$op" "$extra")"
  curl -s -m 5 -X POST "$BRIDGE/" -d "$payload" \
    | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d.get("result", "") if d.get("ok") else "BRIDGE_ERROR: " + str(d.get("error")))'
}

# Assertions ------------------------------------------------------------

# assert_dom_count <selector> <expected>
assert_dom_count() {
  local got
  got="$(dom "$1" count)"
  [ "$got" = "$2" ] || fail "selector '$1' count: expected $2, got '$got'"
}

# assert_dom_text <selector> <expected-substring>
assert_dom_text() {
  local got
  got="$(dom "$1" text)"
  case "$got" in
    *"$2"*) ;;
    *) fail "selector '$1' text: expected substring '$2', got '$got'" ;;
  esac
}

# assert_dom_visible <selector>
assert_dom_visible() {
  local got
  got="$(dom "$1" visible)"
  [ "$got" = "true" ] || fail "selector '$1' expected visible, got '$got'"
}

# assert_dom_hidden <selector>
assert_dom_hidden() {
  local got
  got="$(dom "$1" visible)"
  [ "$got" = "false" ] || fail "selector '$1' expected hidden, got '$got'"
}

# App bootstrap ---------------------------------------------------------

# start_app [extra-app-env...] — boot Xvfb + openbox + the binary with
# OPENKITE_TEST_PORT set; waits for the bridge to answer.
# Requires: BIN (binary path), ART (artifact dir).
start_app() {
  : "${BIN:?BIN must be set}"
  : "${ART:?ART must be set}"
  mkdir -p "$ART"

  export WEBKIT_DISABLE_COMPOSITING_MODE=1
  export WEBKIT_DISABLE_DMABUF_RENDERER=1
  export LIBGL_ALWAYS_SOFTWARE=1
  export RUST_BACKTRACE=full

  Xvfb :99 -screen 0 1280x800x24 -nolisten tcp >"$ART/xvfb.log" 2>&1 &
  XVFB_PID=$!
  trap 'kill $APP_PID $WM_PID $XVFB_PID 2>/dev/null || true' EXIT
  sleep 1
  export DISPLAY=:99

  openbox >"$ART/wm.log" 2>&1 &
  WM_PID=$!
  sleep 1

  # shellcheck disable=SC2086
  OPENKITE_TEST_PORT="$BRIDGE_PORT" "$@" stdbuf -oL -eL "$BIN" >"$ART/app.log" 2>&1 &
  APP_PID=$!

  wait_for_bridge
}

# wait_for_bridge — poll until the bridge answers (window is up).
wait_for_bridge() {
  local i
  for i in $(seq 1 60); do
    if curl -s -m 1 -X POST "$BRIDGE/" -d '{"selector":".app-shell","op":"count"}' >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "bridge never came up; app.log tail:" >&2
  tail -15 "$ART/app.log" 2>/dev/null >&2 || true
  return 1
}

# fail <msg>
fail() {
  echo "FAIL: $*" >&2
  return 1
}
