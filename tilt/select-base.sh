#!/usr/bin/env bash
#
# Choose the released image tag a preview stands on.
#
# The rule (env ladder, §5): the base is the NEWEST release tag whose commit is an
# ancestor of the head being previewed. Anything else renders a branch on top of
# main that never existed — a preview that lies about the branch. When the head
# predates every release tag (a branch cut before the first release, or a shallow
# clone that cannot see the tag commits), the fallback is the newest release, and
# the reason is printed on stderr.
#
# Modes:
#   select-base.sh [--head <rev>]                     -> the tag on stdout
#   select-base.sh [--head <rev>] --annotate <ns>     -> annotate the namespace
#   select-base.sh --annotate <ns> --tag <tag>        -> annotate a known tag
#
# Only the tag itself goes to stdout: callers render manifests from it, so
# diagnostics belong on stderr.
#
# Env: OPENKITE_HEAD (default HEAD), OPENKITE_RELEASE_TAG_GLOB (default 'v[0-9]*'),
#      OPENKITE_BASE_ANNOTATION (default openkite.maklab.net/preview-base),
#      KUBECTL (default kubectl).
#
# Exits 0 with a tag on stdout; 1 when no release tag is visible at all.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"

glob="${OPENKITE_RELEASE_TAG_GLOB:-v[0-9]*}"
annotation_key="${OPENKITE_BASE_ANNOTATION:-openkite.maklab.net/preview-base}"
kubectl="${KUBECTL:-kubectl}"

head=""
namespace=""
tag=""
mode="print"

usage() {
  sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --head)
      [ "$#" -ge 2 ] || { echo "select-base: --head needs a revision" >&2; exit 2; }
      head="$2"
      shift 2
      ;;
    --annotate)
      [ "$#" -ge 2 ] || { echo "select-base: --annotate needs a namespace" >&2; exit 2; }
      mode="annotate"
      namespace="$2"
      shift 2
      ;;
    --tag)
      [ "$#" -ge 2 ] || { echo "select-base: --tag needs a tag" >&2; exit 2; }
      tag="$2"
      shift 2
      ;;
    --print)
      mode="print"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "select-base: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

head="${OPENKITE_HEAD:-${head:-HEAD}}"

# git's own version sort, newest first: v0.33.0 before v0.9.0, which lexical
# order gets wrong, and portable where `sort -V` is not.
tags="$(git -C "$root" tag --list "$glob" --sort=-v:refname)"
if [ -z "$tags" ]; then
  echo "select-base: no release tag matching '$glob' is visible in $root." >&2
  echo "select-base: fetch them before previewing: git fetch --tags" >&2
  exit 1
fi

if [ -z "$tag" ]; then
  for candidate in $tags; do
    # A shallow clone can carry the tag name without the commit; skipping is
    # honest, silently treating it as "not an ancestor" is not.
    if ! git -C "$root" rev-parse --verify --quiet "$candidate^{commit}" >/dev/null; then
      echo "select-base: skipping $candidate — its commit is not present locally (git fetch --tags)." >&2
      continue
    fi
    if git -C "$root" merge-base --is-ancestor "$candidate^{commit}" "$head" 2>/dev/null; then
      tag="$candidate"
      echo "select-base: $candidate is the newest release that is an ancestor of $head." >&2
      break
    fi
  done

  if [ -z "$tag" ]; then
    # The branch predates every tag: nothing to stand on, so stand on the newest
    # release and say so.
    tag="$(printf '%s\n' "$tags" | head -n 1)"
    echo "select-base: no release tag is an ancestor of $head; falling back to the newest release, $tag." >&2
  fi
fi

if [ "$mode" = "annotate" ]; then
  if [ -z "$namespace" ]; then
    echo "select-base: --annotate needs a namespace" >&2
    exit 2
  fi
  # The environment states what it stands on: an operator debugging a preview
  # reads the base off the namespace instead of guessing from the image tag.
  # --overwrite keeps the apply idempotent across re-runs.
  "$kubectl" annotate namespace "$namespace" "$annotation_key=$tag" --overwrite >&2
fi

printf '%s\n' "$tag"
