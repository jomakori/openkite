#!/usr/bin/env bash
#
# delete_cmd for the preview path (Tiltfile: k8s_custom_deploy).
#
# `tilt down` for a preview returns the cluster to nothing: the objects the
# render owns (Deployment, Service, VirtualService, the Cloudflare Access
# AuthorizationPolicy) and the namespace itself. No registry cleanup is owed —
# the preview never published an image, which is the whole point of standing on a
# released one.
#
# The namespace is only deleted when it is demonstrably this preview's: it must
# carry the base annotation tilt/select-base.sh writes. A namespace without it
# belongs to someone else (an ArgoCD-generated environment, a hand-built one),
# and `tilt down` is not a reason to delete another owner's environment.
#
# Idempotent: every step tolerates "already gone".
#
# Env: OPENKITE_PR (required), OPENKITE_NAMESPACE, OPENKITE_BASE_TAG,
#      OPENKITE_BASE_ANNOTATION, OPENKITE_FORCE (1 = delete the namespace even
#      without the annotation), KUBECTL.
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

# The object set is only reproducible from the same render, so the base is
# resolved the same way it was at apply time unless the caller pinned it.
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
