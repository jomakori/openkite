#!/usr/bin/env bash
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

tags="$(git -C "$root" tag --list "$glob" --sort=-v:refname)"
if [ -z "$tags" ]; then
  echo "select-base: no release tag matching '$glob' is visible in $root." >&2
  echo "select-base: fetch them before previewing: git fetch --tags" >&2
  exit 1
fi

if [ -z "$tag" ]; then
  for candidate in $tags; do
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
    tag="$(printf '%s\n' "$tags" | head -n 1)"
    echo "select-base: no release tag is an ancestor of $head; falling back to the newest release, $tag." >&2
  fi
fi

if [ "$mode" = "annotate" ]; then
  if [ -z "$namespace" ]; then
    echo "select-base: --annotate needs a namespace" >&2
    exit 2
  fi
  "$kubectl" annotate namespace "$namespace" "$annotation_key=$tag" --overwrite >&2
fi

printf '%s\n' "$tag"
