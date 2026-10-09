#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATE="$SCRIPT_DIR/compare.sh"
REQUIRE_IMAGEMAGICK=0

log() { echo "[visual-gate-selftest] $*"; }
fail() { log "FAIL: $*"; exit 1; }
usage() {
  echo "Usage: $0 [--require-imagemagick] [--gate <path-to-compare.sh>]" >&2
  exit 2
}

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --require-imagemagick)
      REQUIRE_IMAGEMAGICK=1
      shift
      ;;
    --gate)
      [[ "$#" -ge 2 && -n "$2" ]] || usage
      GATE="$2"
      shift 2
      ;;
    *) usage ;;
  esac
done

[[ -f "$GATE" ]] || fail "gate script not found: $GATE"

# Keep decision checks independent of the caller's shell environment.
unset MAX_DIFF_PIXELS MAX_DIFF_FUZZ

PASSED=0
FAILED=0
record_case() {
  local id="$1" result="$2" description="$3"
  if [[ "$result" == "PASS" ]]; then
    PASSED=$((PASSED + 1))
  else
    FAILED=$((FAILED + 1))
  fi
  printf 'CASE %s %s: %s\n' "$id" "$result" "$description"
}

TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT
BASELINES="$TEMP_DIR/baselines"
FRESH="$TEMP_DIR/fresh"
FAKE_BIN="$TEMP_DIR/fake-bin"
mkdir -p "$BASELINES" "$FRESH" "$FAKE_BIN"
printf 'placeholder baseline\n' > "$BASELINES/01-home.png"
printf 'placeholder fresh image\n' > "$FRESH/01-home.png"

cat > "$FAKE_BIN/compare" <<'FAKE_COMPARE'
#!/usr/bin/env bash
set -euo pipefail
printf '%q ' "$@" >> "$FAKE_ARGV_LOG"
printf '\n' >> "$FAKE_ARGV_LOG"
metric="${FAKE_AE:-0}"
printf '%s\n' "$metric" >&2
if [[ "$metric" -eq 0 ]]; then
  exit 0
fi
exit 1
FAKE_COMPARE
chmod +x "$FAKE_BIN/compare"
ORIGINAL_PATH="$PATH"
export FAKE_ARGV_LOG="$TEMP_DIR/fake-argv.log"
: > "$FAKE_ARGV_LOG"
export PATH="$FAKE_BIN:$PATH"

GATE_RC=0
GATE_OUTPUT=""
run_gate() {
  GATE_OUTPUT=""
  GATE_RC=0
  GATE_OUTPUT=$("$GATE" "$1" "$2" 2>&1) || GATE_RC=$?
}
case_gate() {
  local id="$1" description="$2" expected_rc="$3" expected_text="$4"
  local result=PASS
  if [[ "$GATE_RC" -ne "$expected_rc" ]] || ! grep -Fq -- "$expected_text" <<<"$GATE_OUTPUT"; then
    result=FAIL
    log "$id details: expected exit $expected_rc and '$expected_text'; got exit $GATE_RC"
    log "$id gate output: ${GATE_OUTPUT//$'\n'/ | }"
  fi
  record_case "$id" "$result" "$description"
}

FAKE_AE=0 run_gate "$BASELINES" "$FRESH"
case_gate A1 "metric 0 passes" 0 "PASS: 01-home.png"
FAKE_AE=100 run_gate "$BASELINES" "$FRESH"
case_gate A2 "metric 100 passes" 0 "PASS: 01-home.png"
FAKE_AE=16000 run_gate "$BASELINES" "$FRESH"
case_gate A3 "metric at the default threshold passes" 0 "PASS: 01-home.png"
FAKE_AE=16001 run_gate "$BASELINES" "$FRESH"
case_gate A4 "metric above threshold detects controlled mutation" 1 "FAIL: 01-home.png"
FAKE_AE=16001 MAX_DIFF_PIXELS=20000 run_gate "$BASELINES" "$FRESH"
case_gate A5 "MAX_DIFF_PIXELS environment override is honored" 0 "PASS: 01-home.png"
rm "$FRESH/01-home.png"
FAKE_AE=0 run_gate "$BASELINES" "$FRESH"
case_gate A6 "missing fresh screenshot fails" 1 "MISSING:"
printf 'placeholder fresh image\n' > "$FRESH/01-home.png"
EMPTY_BASELINES="$TEMP_DIR/empty-baselines"
mkdir -p "$EMPTY_BASELINES"
FAKE_AE=0 run_gate "$EMPTY_BASELINES" "$FRESH"
case_gate A7 "empty baseline fixture fails clearly" 1 "no PNG baselines found"
FAKE_AE=0 run_gate "$BASELINES" "$FRESH"
ARGV_RESULT=PASS
if ! grep -Fq -- '-metric AE' "$FAKE_ARGV_LOG" || ! grep -Eq -- '(^| )-fuzz [^ ]+' "$FAKE_ARGV_LOG"; then
  ARGV_RESULT=FAIL
  log "A8 fake compare argv: $(tr '\n' '|' < "$FAKE_ARGV_LOG")"
fi
if [[ "$GATE_RC" -ne 0 ]] || ! grep -Fq -- "PASS: 01-home.png" <<<"$GATE_OUTPUT"; then
  ARGV_RESULT=FAIL
  log "A8 expected a passing gate invocation; got exit $GATE_RC"
fi
record_case A8 "$ARGV_RESULT" "compare argv retains -metric AE and a -fuzz value"

export PATH="$ORIGINAL_PATH"
if command -v compare >/dev/null 2>&1 && command -v convert >/dev/null 2>&1; then
  REAL_BASELINE="$SCRIPT_DIR/baselines/01-home.png"
  REAL_FRESH="$TEMP_DIR/real-fresh"
  REAL_BASELINES="$TEMP_DIR/real-baselines"
  mkdir -p "$REAL_FRESH" "$REAL_BASELINES"
  if [[ ! -f "$REAL_BASELINE" ]]; then
    record_case B1 FAIL "committed reference baseline exists"
    record_case B2 FAIL "controlled ImageMagick mutation is detected"
    record_case B3 FAIL "restored ImageMagick baseline passes"
  else
    cp "$REAL_BASELINE" "$REAL_FRESH/01-home.png"
    cp "$REAL_BASELINE" "$REAL_BASELINES/01-home.png"
    GATE_OUTPUT=""
    GATE_RC=0
    GATE_OUTPUT=$("$GATE" "$REAL_BASELINES" "$REAL_FRESH" 2>&1) || GATE_RC=$?
    result=PASS
    if [[ "$GATE_RC" -ne 0 ]] || ! grep -Fq -- "PASS: 01-home.png" <<<"$GATE_OUTPUT"; then
      result=FAIL
      log "B1 gate output: ${GATE_OUTPUT//$'\n'/ | }"
    fi
    record_case B1 "$result" "pristine byte-copy passes the real gate"

    CONVERT_OUTPUT=""
    CONVERT_RC=0
    CONVERT_OUTPUT=$(convert "$REAL_BASELINE" -fill '#ff00ff' -draw 'rectangle 0,0 250,250' "$REAL_FRESH/01-home.png" 2>&1) || CONVERT_RC=$?
    GATE_OUTPUT=""
    GATE_RC=0
    if [[ "$CONVERT_RC" -eq 0 ]]; then
      GATE_OUTPUT=$("$GATE" "$SCRIPT_DIR/baselines" "$REAL_FRESH" 2>&1) || GATE_RC=$?
    fi
    MUTATION_PIXELS=""
    if [[ "$GATE_OUTPUT" =~ FAIL:\ 01-home\.png\ \(([0-9]+)[[:space:]]differing\ pixels ]]; then
      MUTATION_PIXELS="${BASH_REMATCH[1]}"
    fi
    result=PASS
    if [[ "$CONVERT_RC" -ne 0 || "$GATE_RC" -ne 1 || -z "$MUTATION_PIXELS" || "$MUTATION_PIXELS" -le 16000 ]]; then
      result=FAIL
      log "B2 expected convert success, gate exit 1, and >16000 differing pixels; convert exit=$CONVERT_RC, gate exit=$GATE_RC, pixels=${MUTATION_PIXELS:-unreported}"
      [[ -z "$CONVERT_OUTPUT" ]] || log "B2 convert output: ${CONVERT_OUTPUT//$'\n'/ | }"
      log "B2 gate output: ${GATE_OUTPUT//$'\n'/ | }"
    fi
    record_case B2 "$result" "large high-contrast mutation fails above 16000 pixels"

    cp "$REAL_BASELINE" "$REAL_FRESH/01-home.png"
    GATE_OUTPUT=""
    GATE_RC=0
    GATE_OUTPUT=$("$GATE" "$SCRIPT_DIR/baselines" "$REAL_FRESH" 2>&1) || GATE_RC=$?
    result=PASS
    if [[ "$GATE_RC" -ne 0 ]] || ! grep -Fq -- "PASS: 01-home.png" <<<"$GATE_OUTPUT"; then
      result=FAIL
      log "B3 gate output: ${GATE_OUTPUT//$'\n'/ | }"
    fi
    record_case B3 "$result" "restored pristine byte-copy passes the real gate"
  fi
else
  if [[ "$REQUIRE_IMAGEMAGICK" -eq 1 ]]; then
    record_case B prerequisites FAIL "--require-imagemagick set but compare and/or convert is unavailable"
  else
    log "Part B SKIP: ImageMagick compare and/or convert unavailable"
  fi
fi

log "results: $PASSED passed, $FAILED failed"
if [[ "$FAILED" -gt 0 ]]; then
  exit 1
fi
