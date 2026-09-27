set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
kubectl="${KUBECTL:-kubectl}"
annotation_key="${OPENKITE_BASE_ANNOTATION:-openkite.maklab.net/preview-base}"

pr="${OPENKITE_PR:-}"
if [ -z "$pr" ]; then
  echo "preview-delete: OPENKITE_PR is required." >&2
  exit 1
fi
namespace="${OPENKITE_NAMESPACE:-openkite-pr$pr}"

base="${OPENKITE_BASE_TAG:-$("$here/select-base.sh")}"

manifest="$(OPENKITE_PR="$pr" OPENKITE_NAMESPACE="$namespace" OPENKITE_BASE_TAG="$base" \
  "$here/preview-render.sh")"

printf '%s\n' "$manifest" | "$kubectl" delete -f - --ignore-not-found >&2

if [ -z "$("$kubectl" get namespace "$namespace" --ignore-not-found -o name)" ]; then
  echo "preview-delete: ${namespace} is already gone." >&2
  exit 0
fi

recorded="$("$kubectl" get namespace "$namespace" \
  -o go-template="{{index .metadata.annotations \"$annotation_key\"}}")"

if [ -n "$recorded" ] || [ "${OPENKITE_FORCE:-0}" = "1" ]; then
  "$kubectl" delete namespace "$namespace" --ignore-not-found >&2
  echo "preview-delete: ${namespace} deleted." >&2
else
  echo "preview-delete: refusing to delete ${namespace} — it carries no $annotation_key annotation," >&2
  echo "preview-delete: so it is not this preview's namespace. Delete it by hand if you are sure:" >&2
  echo "preview-delete:   kubectl delete namespace ${namespace}" >&2
fi
