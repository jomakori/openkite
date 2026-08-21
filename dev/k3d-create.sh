#!/usr/bin/env bash
set -euo pipefail

# Create the OpenKite dev cluster + local registry.
#
# k3d talks to Docker over its socket (OrbStack provides this), so no extra
# networking config is needed. Run once, then `tilt up`.

CLUSTER="${CLUSTER:-openkite-dev}"
REGISTRY="${REGISTRY:-openkite-dev-registry}"
PORT="${PORT:-5050}"

cd "$(dirname "$0")/.."

command -v k3d >/dev/null 2>&1 || {
    echo "k3d not found — install it: https://k3d.io" >&2
    exit 1
}

echo "==> Creating cluster '${CLUSTER}' with registry '${REGISTRY}' (localhost:${PORT}) ..."
k3d cluster create "${CLUSTER}" \
    --registry-create "${REGISTRY}:${PORT}" \
    --servers 1 \
    --agents 0 \
    --k3s-arg '--disable=traefik@server:0' \
    --wait

echo "==> Merging kubeconfig ..."
mkdir -p dev/.kube
k3d kubeconfig merge "${CLUSTER}" -d -o dev/.kube/config

echo
echo "==> Ready. Start the dev loop with:"
echo "    KUBECONFIG=dev/.kube/config tilt up"
