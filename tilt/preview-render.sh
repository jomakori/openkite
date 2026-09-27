#!/usr/bin/env bash
#
# Render the openkite app spec at one PR's coordinates, for the overlay preview.
#
# The spec rendered here is the GitOps repo's app chart (apps/helm), the same
# spec prod is rendered from — one source of truth, so an overlay preview shows
# the pod shape prod has. Per-PR parameters (namespaceOverride, createNamespace,
# environments.<env>.subdomain, environments.<env>.tag) are the ones the per-PR
# Application used before the runner took the render over.
#
# The guards at the bottom are the point of this script. Helm silently ignores a
# value key the chart revision does not know, so a chart without
# namespaceOverride would render this preview into openkite-production and apply
# it there. Every coordinate is therefore asserted against the rendered text, and
# the render fails loudly instead of deploying to the wrong namespace.
#
# stdout: the manifest set. stderr: diagnostics.
#
# Env: OPENKITE_PR (required), OPENKITE_BASE_TAG (required),
#      OPENKITE_NAMESPACE (default openkite-pr<N>),
#      OPENKITE_PREVIEW_SUBDOMAIN (default pr<N>-openkite),
#      OPENKITE_CLUSTER_DOMAIN (default maklab.net),
#      OPENKITE_APP_CHART (default ../gke_GitOps/apps/helm),
#      OPENKITE_IMAGE_REPOSITORY (default ghcr.io/jomakori/openkite),
#      OPENKITE_DOPPLER_CONFIG (default svc_openagent),
#      OPENKITE_APP_NAME (default openkite), HELM (default helm).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"

helm="${HELM:-helm}"
pr="${OPENKITE_PR:-}"
base="${OPENKITE_BASE_TAG:-}"

if [ -z "$pr" ]; then
  echo "preview-render: OPENKITE_PR is required (the preview's coordinates are pr<N>)." >&2
  exit 1
fi
if [ -z "$base" ]; then
  echo "preview-render: OPENKITE_BASE_TAG is required — resolve it with tilt/select-base.sh." >&2
  exit 1
fi

app="${OPENKITE_APP_NAME:-openkite}"
namespace="${OPENKITE_NAMESPACE:-openkite-pr$pr}"
subdomain="${OPENKITE_PREVIEW_SUBDOMAIN:-pr$pr-openkite}"
domain="${OPENKITE_CLUSTER_DOMAIN:-maklab.net}"
host="$subdomain.$domain"
repo="${OPENKITE_IMAGE_REPOSITORY:-ghcr.io/jomakori/openkite}"
image="$repo:$base"
doppler="${OPENKITE_DOPPLER_CONFIG:-svc_openagent}"
chart="${OPENKITE_APP_CHART:-$(dirname "$root")/gke_GitOps/apps/helm}"

if [ ! -d "$chart" ]; then
  echo "preview-render: the app spec chart is not at $chart." >&2
  echo "preview-render: check out jomakori/gke_GitOps beside this repo, or set OPENKITE_APP_CHART." >&2
  exit 1
fi

# Per-PR coordinates, mirroring the parameters the openkite-pr<N> Application
# carried: the env name stays `production` (the chart's spec has one release
# environment) and each coordinate is overridden for the preview.
manifest="$("$helm" template "$app" "$chart" \
  --set-string "appName=$app" \
  --set-string "$app.image.repository=$repo" \
  --set-string "$app.environments.production.tag=$base" \
  --set-string "$app.environments.production.subdomain=$subdomain" \
  --set-string "$app.environments.production.dopplerConfig=$doppler" \
  --set-string "$app.namespaceOverride=$namespace" \
  --set "$app.createNamespace=true" \
  --set "$app.enable_domain=true")"

require() {
  if ! printf '%s\n' "$manifest" | grep -qF -- "$1"; then
    echo "preview-render: ERROR: the rendered manifest is missing $1 — $2" >&2
    exit 1
  fi
}

require "name: $namespace" \
  "the chart revision at $chart ignored $app.namespaceOverride, so this preview would be applied to another namespace. Point OPENKITE_APP_CHART at a revision that renders per-PR coordinates."
require "namespace: $namespace" \
  "the chart revision at $chart ignored $app.namespaceOverride. Refusing to hand kubectl a manifest that is not addressed to $namespace."
require "$image" \
  "the chart revision at $chart did not render the base image $image (check $app.image.repository and $app.environments.production.tag)."
require "$host" \
  "the chart revision at $chart did not render the preview host $host."
require "kind: Namespace" \
  "$app.createNamespace=true did not produce a Namespace object."

if printf '%s\n' "$manifest" | grep -qF -- "namespace: $app-production"; then
  echo "preview-render: ERROR: the render contains the production namespace $app-production — this is the preview path, not a prod deploy." >&2
  exit 1
fi

echo "preview-render: $namespace on $image serves https://$host" >&2
printf '%s\n' "$manifest"
