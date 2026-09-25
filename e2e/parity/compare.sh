#!/usr/bin/env bash
# Console parity gate (OKT-129): compare the browser console render against the
# desktop console render, per surface, against committed baselines.
#
# Three passes per surface, all with the same ImageMagick `compare -metric AE`
# machinery the existing visual-regression gate uses:
#
#   (a) browser vs baseline   — the browser side must be near-deterministic,
#                               otherwise the gate is measuring its own noise.
#   (b) desktop vs baseline   — the gate. This is where a real visual
#                               divergence between the two consoles shows up.
#   (c) desktop vs browser    — reported for diagnosis, same cap: it is the
#                               direct measurement of renderer parity.
#
# Referenced images (both sides) are size-asserted against $FRAME first, so a
# decoration or a differently-sized viewport fails loudly instead of producing
# a mysterious diff. Masks are applied to all three images identically; every
# mask and every exclusion must carry the capability that explains it (see
# parity-config.sh), and an unexplained difference FAILS.
#
# Usage: compare.sh <capture-dir> <baselines-dir>
#   <capture-dir>  holds <surface>.browser.png and <surface>.desktop.png
#   <baselines-dir> holds <surface>.png (the browser reference render)
# Writes: <capture-dir>/compare.log, masks/, and <surface>.*-vs-*.diff.png on
# failure; appends a markdown table to $GITHUB_STEP_SUMMARY when set.
# Exit: 0 all surfaces pass, 1 otherwise.

set -euo pipefail

CAPTURES="${1:?capture dir (browser + desktop PNGs)}"
BASELINES="${2:?baselines dir}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=parity-config.sh
. "$HERE/parity-config.sh"

LOG="$CAPTURES/compare.log"
: >"$LOG"

log() {
  echo "[parity-compare] $*"
  echo "[parity-compare] $*" >>"$LOG"
}
fail() {
  log "FAIL: $*"
  exit 1
}

for tool in compare identify convert; do
  command -v "$tool" >/dev/null 2>&1 || fail "ImageMagick '$tool' is not installed"
done
[ -d "$CAPTURES" ] || fail "capture dir not found: $CAPTURES"
[ -d "$BASELINES" ] || fail "baselines dir not found: $BASELINES"
case "$FRAME" in
  [0-9]*x[0-9]*) ;;
  *) fail "FRAME is malformed: $FRAME" ;;
esac

FRAME_W="${FRAME%%x*}"
FRAME_H="${FRAME##*x}"
FRAME_AREA=$((FRAME_W * FRAME_H))

# --- allow-list integrity ------------------------------------------------
# Every exclusion has to name the capability that explains it. An entry
# without one is an unexplained difference waiting to happen, so it fails here
# rather than being quietly tolerated.
[ "${#EXCLUDED[@]}" -gt 0 ] || fail "EXCLUDED is empty: the allow-list must say what is excluded and why"
for entry in "${EXCLUDED[@]}"; do
  id="${entry%%:*}"
  reason="${entry#*:}"
  [ -n "$id" ] || fail "excluded entry without an id: '$entry'"
  [ "$reason" != "$entry" ] || fail "excluded '$id' carries no reason"
  [ -n "$reason" ] || fail "excluded '$id' carries an empty reason"
done
log "allow-list: ${#EXCLUDED[@]} capability-attributable exclusions (each with a reason)"

# --- masks ---------------------------------------------------------------
MASKED_AREA=0
MASKED_DIR=""
if [ "${#MASK_RECTS[@]}" -gt 0 ]; then
  MASKED_DIR="$CAPTURES/masked"
  mkdir -p "$MASKED_DIR"
  for mask in "${MASK_RECTS[@]}"; do
    id="${mask%%:*}"
    rest="${mask#*:}"
    rect="${rest%%:*}"
    rest="${rest#*:}"
    reason="${rest%%:*}"
    ticket="${rest#*:}"
    [ -n "$id" ] || fail "mask without an id: '$mask'"
    [ "$rect" != "$mask" ] || fail "mask '$id' has no rect"
    [ -n "$reason" ] && [ "$reason" != "$mask" ] || fail "mask '$id' has no reason"
    [ -n "$ticket" ] && [ "$ticket" != "$mask" ] || fail "mask '$id' has no ticket"
    case "$rect" in
      [0-9]*,[0-9]*,[0-9]*,[0-9]*) ;;
      *) fail "mask '$id' rect is malformed: '$rect' (want x,y,w,h)" ;;
    esac
    rect_w=$(echo "$rect" | cut -d, -f3)
    rect_h=$(echo "$rect" | cut -d, -f4)
    MASKED_AREA=$((MASKED_AREA + rect_w * rect_h))
    log "mask $id $rect — $reason ($ticket)"
  done
  MASKED_MAX=$((FRAME_AREA * MAX_MASKED_FRACTION / 100))
  [ "$MASKED_AREA" -le "$MASKED_MAX" ] ||
    fail "masks cover $MASKED_AREA px, over the ${MAX_MASKED_FRACTION}% cap ($MASKED_MAX px)"
  log "masks cover $MASKED_AREA px of $FRAME_AREA (cap ${MAX_MASKED_FRACTION}%)"
else
  log "masks: none (bootstrap — mask creep is capped at ${MAX_MASKED_FRACTION}% of the frame)"
fi

# --- helpers -------------------------------------------------------------
dims_of() { identify -format '%wx%h' "$1" 2>/dev/null || true; }

# assert_dims <png> <label>
assert_dims() {
  local png="$1" label="$2" dims
  [ -f "$png" ] || fail "missing $label image: $png"
  dims="$(dims_of "$png")"
  [ "$dims" = "$FRAME" ] ||
    fail "$label is ${dims:-unreadable}, expected $FRAME — the comparison would be offset"
}

# masked <src> — the image with every mask rect painted out, or the image
# itself when there are no masks.
masked() {
  local src="$1" dest="$MASKED_DIR/$(basename "$1")"
  if [ -z "$MASKED_DIR" ]; then
    echo "$src"
    return
  fi
  local draws=()
  for mask in "${MASK_RECTS[@]}"; do
    rest="${mask#*:}"
    rect="${rest%%:*}"
    x="${rect%%,*}"
    rest="${rect#*,}"
    y="${rest%%,*}"
    rest="${rest#*,}"
    w="${rest%%,*}"
    h="${rest##*,}"
    draws+=(-draw "rectangle $x,$y $((x + w)),$((y + h))")
  done
  convert "$src" -fill black "${draws[@]}" "$dest"
  echo "$dest"
}

# ae <a> <b> — absolute count of pixels differing beyond FUZZ.
ae() {
  local out
  out=$(compare -metric AE -fuzz "$FUZZ" "$1" "$2" null: 2>&1 || true)
  echo "$out" | grep -oE '^[0-9]+' || echo "0"
}

# diff_image <a> <b> <path> — highlighted difference, for the artifact.
diff_image() {
  compare -fuzz "$FUZZ" -highlight-color red -lowlight-color none "$1" "$2" "$3" 2>/dev/null || true
}

# check <surface> <a-label> <a-png> <b-label> <b-png> <cap>
check() {
  local surface="$1" a_label="$2" a_png="$3" b_label="$4" b_png="$5" cap="$6"
  local value
  value=$(ae "$a_png" "$b_png")
  CASES+=("$surface|$a_label|$b_label|$value|$cap")
  if [ "$value" -le "$cap" ]; then
    log "$surface: $a_label vs $b_label = $value px (cap $cap) PASS"
  else
    local diff="$CAPTURES/$surface.${a_label}-vs-${b_label}.diff.png"
    diff_image "$a_png" "$b_png" "$diff"
    log "$surface: $a_label vs $b_label = $value px (cap $cap) FAIL -> $diff"
    FAILURES=$((FAILURES + 1))
  fi
}

# --- gate ----------------------------------------------------------------
FAILURES=0
CASES=()
log "frame $FRAME, fuzz $FUZZ, cap $MAX_DIFF_PIXELS px, browser self cap $BROWSER_SELF_MAX px"

for surface in "${SURFACES[@]}"; do
  browser="$CAPTURES/$surface.browser.png"
  desktop="$CAPTURES/$surface.desktop.png"
  baseline="$BASELINES/$surface.png"

  assert_dims "$browser" "$surface browser capture"
  assert_dims "$desktop" "$surface desktop capture"
  assert_dims "$baseline" "$surface baseline"

  browser_masked=$(masked "$browser")
  desktop_masked=$(masked "$desktop")
  baseline_masked=$(masked "$baseline")

  check "$surface" browser baseline "$browser_masked" "$baseline_masked" "$BROWSER_SELF_MAX"
  check "$surface" desktop baseline "$desktop_masked" "$baseline_masked" "$MAX_DIFF_PIXELS"
  check "$surface" desktop browser "$desktop_masked" "$browser_masked" "$MAX_DIFF_PIXELS"
done

# --- report --------------------------------------------------------------
{
  echo "## Console parity (browser vs desktop)"
  echo
  echo "Frame \`$FRAME\` · fuzz \`$FUZZ\` · desktop cap \`$MAX_DIFF_PIXELS\` px · browser self cap \`$BROWSER_SELF_MAX\` px · masked \`$MASKED_AREA\` px of \`$FRAME_AREA\`"
  echo
  echo "| surface | comparison | AE (px) | cap | verdict |"
  echo "|---|---|---:|---:|---|"
  for row in "${CASES[@]}"; do
    IFS='|' read -r surface a_label b_label value cap <<<"$row"
    if [ "$value" -le "$cap" ]; then verdict="pass"; else verdict="**FAIL**"; fi
    echo "| \`$surface\` | $a_label vs $b_label | $value | $cap | $verdict |"
  done
  echo
  echo "Excluded (capability-attributable, never compared): ${EXCLUDED[*]}"
  echo
  if [ "$FAILURES" -gt 0 ]; then
    echo "Result: **$FAILURES failing comparison(s)** — diff images in the \`console-parity-artifacts\` artifact."
  else
    echo "Result: pass. Baselines refresh in the same PR as the visual change they follow (see \`e2e/README.md\`)."
  fi
} >>"$LOG"

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  cat "$LOG" >>"$GITHUB_STEP_SUMMARY"
fi

log "results: ${#SURFACES[@]} surfaces, $FAILURES failing comparison(s)"
[ "$FAILURES" -eq 0 ] || fail "$FAILURES parity comparison(s) over the cap"
log "ALL PASS"
