#!/usr/bin/env bash
#
# prune-preview-image-versions.sh — delete the GHCR versions carrying a closed
# PR's preview tags.
#
# Teardown has one owner per side: the gke_GitOps generator prunes the cluster
# objects (Application, namespace, workload), and Cloudflare has nothing per PR
# (the zone wildcard plus one wildcard Access application answer every host,
# including one that never existed). Neither touches the registry — every
# preview build publishes `pr-<N>` and `pr-<N>-<sha>`, and they outlive the PR.
#
# Deletes address a version id, never a tag: one version carries BOTH tags, so
# a tag-scoped delete leaves the version alive under the other one.
#
# Usage:
#   prune-preview-image-versions.sh --pr <N> [--dry-run]   one closed PR
#   prune-preview-image-versions.sh --sweep  [--dry-run]   every closed PR
#
# `--pr` refuses a PR that is not closed, so a stale number cannot prune the
# image a running preview serves. `--sweep` is the stand-in for the retention
# rule that cannot exist here: GitHub's package retention settings are
# organisation-only and this package is user-owned.
#
# Requires: GH_TOKEN with packages:write (a classic PAT also needs
# delete:packages). The token is read from the environment, never passed as an
# argument and never printed.
set -euo pipefail

repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must be set, e.g. jomakori/openkite}"
owner="${PACKAGE_OWNER:-${repo%%/*}}"
package="${PACKAGE_NAME:-${repo##*/}}"
versions_path="/users/${owner}/packages/container/${package}/versions"

failures=0
dry_run=false

usage() {
  echo "usage: $0 --pr <N> [--dry-run] | --sweep [--dry-run]" >&2
}

# One line per tag, "<version-id>\t<tag>", so a version's whole tag set is
# visible and a version with no tags simply contributes nothing.
list_version_tags() {
  gh api --paginate "$versions_path" \
    --jq '.[] | .id as $id | (.metadata.container.tags // [])[] | "\($id)\t\(.)"'
}

# Tags are space-free and the version id is numeric, so `cut -f1` is safe.
tags_for_id() {
  local id="$1"
  printf '%s\n' "$selected" | awk -F'\t' -v id="$id" '$1 == id { printf "%s ", $2 }'
}

delete_version() {
  local id="$1"
  if [ "$dry_run" = true ]; then
    echo "would delete version ${id} (tags: $(tags_for_id "$id"))"
    return 0
  fi
  if gh api --method DELETE "${versions_path}/${id}" >/dev/null; then
    echo "deleted version ${id} (tags: $(tags_for_id "$id"))"
  else
    echo "FAILED to delete version ${id} (tags: $(tags_for_id "$id"))" >&2
    failures=$((failures + 1))
  fi
}

prune_pr() {
  local pr="$1"
  # `selected` (id<TAB>tag per line) stays in scope for tags_for_id's logging.
  # `pr-<N>` and `pr-<N>-<sha>`, anchored so pr-13 never selects pr-131. A
  # v<X.Y.Z> release tag cannot match this selector by construction.
  selected="$(list_version_tags | awk -F'\t' -v re="^pr-${pr}(-.*)?\$" '$2 ~ re')"
  if [ -z "$selected" ]; then
    echo "pr-${pr}: no image versions tagged"
    return 0
  fi
  local ids count
  ids="$(printf '%s\n' "$selected" | cut -f1 | sort -u)"
  count="$(printf '%s\n' "$ids" | wc -l | tr -d ' ')"
  echo "pr-${pr}: ${count} version(s)"
  local id
  for id in $ids; do
    delete_version "$id"
  done
}

# The PR behind an image is whichever PR the tag names — the API is the only
# authority on whether it is still open, so it decides, not the tag's age.
pr_state() {
  local pr="$1" state
  state="$(gh api "repos/${repo}/pulls/${pr}" --jq '.state' 2>/dev/null || true)"
  printf '%s' "${state:-unknown}"
}

tagged_pr_numbers() {
  list_version_tags | awk -F'\t' '$2 ~ /^pr-[0-9]+(-.*)?$/ { n = $2; sub(/^pr-/, "", n); sub(/-.*$/, "", n); print n }' | sort -un
}

mode=""
pr=""
while [ $# -gt 0 ]; do
  case "$1" in
    --pr) mode=pr; pr="${2:?--pr needs a PR number}"; shift 2 ;;
    --sweep) mode=sweep; shift ;;
    --dry-run) dry_run=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) usage; echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

case "$mode" in
  pr)
    if ! [[ "$pr" =~ ^[0-9]+$ ]]; then
      echo "--pr must be a PR number, got: '${pr}'" >&2
      exit 2
    fi
    state="$(pr_state "$pr")"
    if [ "$state" != "closed" ]; then
      echo "pr-${pr} is ${state}, not closed: refusing to delete the image a live preview may serve" >&2
      exit 1
    fi
    prune_pr "$pr"
    ;;
  sweep)
    for n in $(tagged_pr_numbers); do
      state="$(pr_state "$n")"
      if [ "$state" = "closed" ]; then
        prune_pr "$n"
      else
        echo "pr-${n}: ${state} — kept"
      fi
    done
    ;;
  *)
    usage
    exit 2
    ;;
esac

if [ "$failures" -gt 0 ]; then
  echo "${failures} version(s) could not be deleted" >&2
  exit 1
fi
