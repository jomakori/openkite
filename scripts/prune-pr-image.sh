#!/usr/bin/env bash
#
# Delete the container image versions a closed pull request left behind.
#
# Usage: prune-pr-image.sh <pr-number>...
#        prune-pr-image.sh --sweep     # every pr-* version whose PR is closed
set -euo pipefail

OWNER="${OWNER:-jomakori}"
PACKAGE="${PACKAGE:-openkite}"
REPO="${REPO:-jomakori/openkite}"

log() { printf '[prune-pr-image] %s\n' "$*"; }

# One line per version: <id>\t<comma-separated tags>. A version carries both
# pr-<N> and pr-<N>-<sha>, and only the id is deletable.
versions() {
  gh api --paginate "/users/${OWNER}/packages/container/${PACKAGE}/versions?per_page=100" \
    -q '.[]|[.id, ((.metadata.container.tags // [])|join(","))]|@tsv'
}

ids_for_pr() {
  awk -F'\t' -v want="pr-$1" '
    { n = split($2, t, ","); for (i = 1; i <= n; i++) if (t[i] == want || index(t[i], want "-") == 1) { print $1; break } }'
}

delete_id() {
  gh api -X DELETE "/users/${OWNER}/packages/container/${PACKAGE}/versions/$1" >/dev/null
}

prune_pr() {
  local pr="$1" id count=0
  while read -r id; do
    [ -n "$id" ] || continue
    delete_id "$id"
    count=$((count + 1))
  done < <(versions | ids_for_pr "$pr")
  log "pr-${pr}: deleted ${count} version(s)"
}

sweep() {
  local pr state pruned=0 kept=0
  while read -r pr; do
    [ -n "$pr" ] || continue
    state="$(gh pr view "$pr" --repo "$REPO" --json state -q .state 2>/dev/null || true)"
    case "$state" in
      CLOSED|MERGED)
        prune_pr "$pr"
        pruned=$((pruned + 1))
        ;;
      *)
        log "pr-${pr}: ${state:-unknown}, kept"
        kept=$((kept + 1))
        ;;
    esac
  done < <(versions | awk -F'\t' '{ n = split($2, t, ","); for (i = 1; i <= n; i++) if (t[i] ~ /^pr-[0-9]+$/) print substr(t[i], 4) }' | sort -un)
  log "sweep: ${pruned} closed PR(s) pruned, ${kept} PR(s) kept"
}

case "${1:-}" in
  --sweep) sweep ;;
  '')
    printf 'usage: %s <pr-number>... | --sweep\n' "$0" >&2
    exit 2
    ;;
  *) for pr in "$@"; do prune_pr "$pr"; done ;;
esac
