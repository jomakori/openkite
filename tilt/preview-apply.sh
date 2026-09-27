#!/usr/bin/env bash
#
# apply_cmd for the preview path (Tiltfile: k8s_custom_deploy).
#
# 1. The base this preview stands on: the newest release tag that is an ancestor
#    of the head (tilt/select-base.sh).
# 2. Render the app spec at pr<N> coordinates with that tag as the image
#    (tilt/preview-render.sh).
# 3. kubectl apply it — the Kubernetes API only. No Docker daemon, no image
#    build, no registry write: the base is a released image, and what differs on
#    this branch rides in over live_update.
# 4. Record the base as a namespace annotation, so the environment states what it
#    stands on.
#
# stdout is the applied YAML and nothing else: Tilt reads apply_cmd's stdout to
# learn which objects it manages and to attach pod logs, so kubectl's own
# "created/configured" chatter is sent to stderr.
#
# Idempotent: the render is deterministic, kubectl apply is idempotent, the
# annotation uses --overwrite. Safe to re-run on every `tilt up`.
#
# Env: OPENKITE_PR (required), OPENKITE_HEAD (default HEAD), OPENKITE_NAMESPACE,
#      KUBECTL, plus everything tilt/preview-render.sh documents.
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

# A base is only useful if it is recorded; the tag rides the annotation, not the
# Tiltfile, so a new release does not require editing the Tiltfile to be picked
# up (a re-apply does, and that is what apply_cmd is for).
base="$("$here/select-base.sh" --head "$head")"
echo "preview-apply: ${namespace} stands on ${base}" >&2

manifest="$(OPENKITE_PR="$pr" OPENKITE_NAMESPACE="$namespace" OPENKITE_BASE_TAG="$base" \
  "$here/preview-render.sh")"

printf '%s\n' "$manifest" | "$kubectl" apply -f - >&2

"$here/select-base.sh" --annotate "$namespace" --tag "$base" >&2

printf '%s\n' "$manifest"
