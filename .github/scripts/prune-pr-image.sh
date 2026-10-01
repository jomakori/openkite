#!/usr/bin/env bash
#
# Delete the preview image versions a pull request left behind.
#
# Previews are NOT pruned when the PR closes. The preview image is the input the
# release transports, so a merge that deletes it strands that release on a cold
# full rebuild. Transport and pruning both belong to the release workflow:
#
#   --transported <pr>   the release retagged pr-<N> as v<X.Y.Z>. The released
#                        tag and the manifest's digest are the record now, so
#                        the preview version goes away whole.
#   --sweep              every pr-* version whose PR closed WITHOUT merging. A
#                        merged PR keeps its sha pin until its release runs.
#   --merged <pr>        keep the sha-pinned version: the pre-transport record of
#                        what was reviewed. The sweep uses this.
#
# GitHub's container packages expose no tag-delete call — a version (one digest)
# is deleted whole, together with every tag on it. When `pr-<N>` and
# `pr-<N>-<sha>` are tags on the SAME version (one build, two tags, one digest),
# keeping the pin necessarily keeps both tags; the mutable tag then lives until
# the version itself is pruned. That is the conservative direction: losing the
# pin would lose the record a merged-but-unreleased PR is supposed to keep.
#
# Usage: prune-pr-image.sh <pr-number>...             # closed, unmerged
#        prune-pr-image.sh --merged <pr-number>...    # merged, keep the sha pin
#        prune-pr-image.sh --transported <pr-number>... # released, delete all
#        prune-pr-image.sh --sweep                    # every pr-* version whose PR is closed or merged
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

# <pr>\t<tags> in, <version id> out for every version this PR must lose.
ids_to_delete() {
  awk -F'\t' -v want="pr-$1" -v mode="$2" '
    {
      n = split($2, t, ",")
      mutable = 0
      pinned = 0
      for (i = 1; i <= n; i++) {
        if (t[i] == want) mutable = 1
        else if (index(t[i], want "-") == 1) pinned = 1
      }
      drop = 0
      if (mutable) { if (!(mode == "merged" && pinned)) drop = 1 }
      else if (pinned && mode != "merged") drop = 1
      if (drop) print $1
    }'
}

delete_id() {
  gh api -X DELETE "/users/${OWNER}/packages/container/${PACKAGE}/versions/$1" >/dev/null
}

prune_pr() {
  local pr="$1" mode="$2" id count=0
  while read -r id; do
    [ -n "$id" ] || continue
    delete_id "$id"
    count=$((count + 1))
  done < <(versions | ids_to_delete "$pr" "$mode")
  if [ "$count" -eq 0 ] && [ "$mode" = "merged" ]; then
    log "pr-${pr}: kept the sha-pinned version, deleted nothing"
  else
    log "pr-${pr}: deleted ${count} version(s)"
  fi
}

sweep() {
  local pr state merged=0 closed=0 kept=0
  while read -r pr; do
    [ -n "$pr" ] || continue
    state="$(gh pr view "$pr" --repo "$REPO" --json state -q .state 2>/dev/null || true)"
    case "$state" in
      MERGED)
        prune_pr "$pr" merged
        merged=$((merged + 1))
        ;;
      CLOSED)
        prune_pr "$pr" whole
        closed=$((closed + 1))
        ;;
      *)
        log "pr-${pr}: ${state:-unknown}, kept"
        kept=$((kept + 1))
        ;;
    esac
  done < <(versions | awk -F'\t' '{ n = split($2, t, ","); for (i = 1; i <= n; i++) if (t[i] ~ /^pr-[0-9]+$/) print substr(t[i], 4) }' | sort -un)
  log "sweep: ${merged} merged PR(s) kept as sha pins, ${closed} closed PR(s) pruned, ${kept} open PR(s) kept"
}

case "${1:-}" in
  --sweep) sweep ;;
  --merged)
    shift
    [ $# -gt 0 ] || {
      printf 'usage: %s --merged <pr-number>...\n' "$0" >&2
      exit 2
    }
    for pr in "$@"; do prune_pr "$pr" merged; done
    ;;
  --transported)
    shift
    [ $# -gt 0 ] || {
      printf 'usage: %s --transported <pr-number>...\n' "$0" >&2
      exit 2
    }
    # The release tag carries this digest now, so nothing is kept.
    for pr in "$@"; do prune_pr "$pr" whole; done
    ;;
  '')
    printf 'usage: %s [--merged|--transported] <pr-number>... | --sweep\n' "$0" >&2
    exit 2
    ;;
  *) for pr in "$@"; do prune_pr "$pr" whole; done ;;
esac
