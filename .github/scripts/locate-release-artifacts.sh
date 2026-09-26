#!/usr/bin/env bash
#
# locate-release-artifacts.sh — decide whether a reusable artifact set exists
# for a merged commit, or whether release.yml must fall back to a rebuild.
#
# Build-once reuse (OKT-104): pull requests build the six native packages once.
# On a release, `release.yml` looks here for that build instead of rebuilding.
#
# Squash merges produce a brand-new commit SHA, so the PR build's run is keyed
# to the PR HEAD SHA, not the merged SHA. The merge is mapped back to its PR
# through the API, then the newest successful `build-e2e` run for that SHA is
# checked for a complete, unexpired artifact set.
#
# The same mapping yields the preview image tag the release promotes instead of
# rebuilding: `pr-<N>-<short sha>`, where the short sha is the image build's own
# 7-character pin (docker/metadata-action `type=sha,format=short` on the
# checked-out commit).
#
# Usage: locate-release-artifacts.sh <merged-sha>
#
# Writes to $GITHUB_OUTPUT (or stdout when unset):
#   reuse=true|false   whether the caller may download instead of rebuilding
#   run_id=<id>        the run to download from when reuse=true (empty otherwise)
#   source_sha=<sha>   the PR head SHA the build ran on (empty when unmapped)
#   preview_tag=<tag>  the PR's sha-pinned image tag (empty when unmapped)
#
# Requires: GH_TOKEN with actions:read (+ contents/pull-requests read).
set -euo pipefail

merged="${1:?usage: locate-release-artifacts.sh <merged-sha>}"
repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
out="${GITHUB_OUTPUT:-/dev/stdout}"

reuse=false
run_id=""
source_sha=""
preview_tag=""

# Map the merged commit to the PR that produced it. A direct push to main has
# no PR, so there is nothing to reuse and the rebuild fallback must engage.
pr_number=""
IFS=$'\t' read -r pr_number source_sha < <(
  gh api "repos/${repo}/commits/${merged}/pulls" \
    --jq '[(.[0].number // "" | tostring), (.[0].head.sha // "")] | @tsv' 2>/dev/null || true
)

if [ -n "$source_sha" ] && [ -n "$pr_number" ]; then
  preview_tag="pr-${pr_number}-${source_sha:0:7}"
fi

if [ -n "$source_sha" ]; then
  # Newest successful pull-request build for that head SHA. `build-e2e` is the
  # caller workflow name (`.github/workflows/build-e2e.yml`), which owns the
  # six-package matrix.
  run_id="$(gh run list --repo "$repo" --commit "$source_sha" --limit 30 \
    --json databaseId,name,conclusion \
    --jq '[.[] | select(.name == "build-e2e" and .conclusion == "success")][0].databaseId // empty')"
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
  echo "preview_tag=${preview_tag}"
} >> "$out"
