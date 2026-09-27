#!/usr/bin/env bash
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
kubectl="${KUBECTL:-kubectl}"

pr="${OPENKITE_PR:-}"
if [ -z "$pr" ]; then
  echo "preview-apply: OPENKITE_PR is required." >&2
  exit 1
fi
namespace="${OPENKITE_NAMESPACE:-openkite-pr$pr}"
head="${OPENKITE_HEAD:-HEAD}"

base="$("$here/select-base.sh" --head "$head")"
echo "preview-apply: ${namespace} stands on ${base}" >&2

manifest="$(OPENKITE_PR="$pr" OPENKITE_NAMESPACE="$namespace" OPENKITE_BASE_TAG="$base" \
  "$here/preview-render.sh")"

printf '%s\n' "$manifest" | "$kubectl" apply -f - >&2

"$here/select-base.sh" --annotate "$namespace" --tag "$base" >&2

printf '%s\n' "$manifest"
