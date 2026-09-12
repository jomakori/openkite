#!/usr/bin/env bash
#
# locate-release-artifacts.sh — decide whether a reusable artifact set exists
# for a merged commit, or whether release.yml must fall back to a rebuild.
#
# Build-once reuse (OKT-104): PRs build the six native packages once. On a
# release, `release.yml` looks here for that build instead of rebuilding.
#
# Squash merges produce a brand-new commit SHA, so the PR build's run is keyed
# to the PR HEAD SHA, not the merged SHA. The merge is mapped back to its PR's
# head SHA through the API, then the newest successful `build-artifacts` run for
# that SHA is checked for a complete, unexpired artifact set.
#
# Usage: locate-release-artifacts.sh <merged-sha>
#
# Writes to $GITHUB_OUTPUT (or stdout when unset):
#   reuse=true|false   whether the caller may download instead of rebuilding
#   run_id=<id>        the run to download from when reuse=true (empty otherwise)
#   source_sha=<sha>   the PR head SHA the build ran on (empty when unmapped)
#
# Requires: GH_TOKEN with actions:read (+ contents/pull-requests read).
set -euo pipefail

merged="${1:?usage: locate-release-artifacts.sh <merged-sha>}"
repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
out="${GITHUB_OUTPUT:-/dev/stdout}"

reuse=false
run_id=""
source_sha=""

# Map the merged commit to the PR that produced it. A direct push to main has
# no PR, so there is nothing to reuse and the rebuild fallback must engage.
source_sha="$(gh api "repos/${repo}/commits/${merged}/pulls" \
  --jq '.[0].head.sha // empty' 2>/dev/null || true)"

if [ -n "$source_sha" ]; then
  # Newest successful PR build for that head SHA. `build-artifacts` is the
  # caller workflow name (`.github/workflows/build-artifacts.yml`).
  run_id="$(gh run list --repo "$repo" --commit "$source_sha" --limit 30 \
    --json databaseId,name,conclusion \
    --jq '[.[] | select(.name == "build-artifacts" and .conclusion == "success")][0].databaseId // empty')"
fi

if [ -n "$run_id" ]; then
  present="$(gh api "repos/${repo}/actions/runs/${run_id}/artifacts?per_page=100" \
    --jq '[.artifacts[] | select(.expired == false) | .name] | join(",")')"
  missing=""
  for want in linux_amd64 linux_arm64 macos_arm64 macos_amd64 windows_amd64 windows_arm64; do
    case ",${present}," in
      *",openkite_${want},"*) ;;
      *) missing="${missing} openkite_${want}" ;;
    esac
  done
  if [ -n "$missing" ]; then
    echo "::notice title=Rebuilding release artifacts::incomplete reusable set on run ${run_id}; missing:${missing}" >&2
    run_id=""
  else
    reuse=true
    echo "reusing complete artifact set from run ${run_id} (PR head ${source_sha})" >&2
  fi
fi

if [ "$reuse" != "true" ]; then
  echo "::notice title=Rebuilding release artifacts::no reusable set for ${merged}; rebuild fallback engaged" >&2
fi

{
  echo "reuse=${reuse}"
  echo "run_id=${run_id}"
  echo "source_sha=${source_sha}"
} >> "$out"
