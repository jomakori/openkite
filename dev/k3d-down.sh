#!/usr/bin/env bash
set -euo pipefail

# Tear down the OpenKite dev cluster + registry. Run after `tilt down`.
# Keeps the dev loop ephemeral — nothing runs continuously.

CLUSTER="${CLUSTER:-openkite-dev}"
REGISTRY="${REGISTRY:-openkite-dev-registry}"

cd "$(dirname "$0")/.."

command -v k3d >/dev/null 2>&1 || {
    echo "k3d not found — install it: https://k3d.io" >&2
    exit 1
}

echo "==> Deleting cluster '${CLUSTER}' ..."
k3d cluster delete "${CLUSTER}" || true

echo "==> Deleting registry '${REGISTRY}' ..."
k3d registry delete "${REGISTRY}" || true

echo "==> Removing local kubeconfig ..."
rm -f dev/.kube/config

echo "==> Done. Cluster is fully torn down."
