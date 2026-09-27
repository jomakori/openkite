#!/usr/bin/env bash
#
# Compile the crate that runs IN the container, in a temporary in-cluster build
# pod, and pull the binary back out for the overlay. No Docker, no registry.
#
# WHY THIS EXISTS: a file overlay cannot carry compiled behaviour. If the diff
# touches Rust that runs inside the container (crates/openkite-web — the axum +
# kube-rs host that serves the console and answers its kube bridge), no `sync`
# step can express it. The honest path is compile-and-swap: build the binary
# where it will run (the cluster's own architecture, its own deps), pull it out,
# then sync it and re-execute the process with restart_container().
#
# STATUS — read this before using it: the released image today is a static server
# (nginx + the prebuilt bundle), so it carries no host binary to replace and the
# overlay has no consumer for this artifact. The Tiltfile therefore has no sync
# step for tilt/out/bin/: adding one before the image ships the host would sync a
# file into a path that does not exist and restart a process that is not running.
# When the image does carry the host, the Tiltfile grows:
#
#   sync('./tilt/out/bin', '<the image's bin dir>'),
#   restart_container(),
#
# Usage: OPENKITE_PR=<N> tilt/build-server.sh [crate]
#
# Env: OPENKITE_PR (required), OPENKITE_NAMESPACE, OPENKITE_SERVER_OUT (default
#      tilt/out/bin), OPENKITE_BUILD_IMAGE (default rust:1-bookworm),
#      OPENKITE_BUILD_POD, KUBECTL.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
kubectl="${KUBECTL:-kubectl}"

crate="${1:-openkite-web}"
pr="${OPENKITE_PR:-}"
if [ -z "$pr" ]; then
  echo "build-server: OPENKITE_PR is required — the build pod lands in the preview namespace." >&2
  exit 1
fi
namespace="${OPENKITE_NAMESPACE:-openkite-pr$pr}"
out="${OPENKITE_SERVER_OUT:-$here/out/bin}"
pod="${OPENKITE_BUILD_POD:-openkite-preview-build-$pr}"
image="${OPENKITE_BUILD_IMAGE:-rust:1-bookworm}"

if [ ! -f "$root/Cargo.toml" ]; then
  echo "build-server: $root/Cargo.toml is missing — run this from the repository." >&2
  exit 1
fi

# A build pod that outlives its build is a leak in someone's namespace.
cleanup() {
  "$kubectl" -n "$namespace" delete pod "$pod" --ignore-not-found --wait=false >/dev/null 2>&1 || true
}
trap cleanup EXIT

mkdir -p "$out"

# A Pod, not a Job: the workspace is streamed in and the artifact copied out over
# the API, so the pod has to stay up while that happens. Sleep-infinity is the
# smallest thing that does that, and the trap above ends it.
"$kubectl" -n "$namespace" apply -f - >&2 <<MANIFEST
apiVersion: v1
kind: Pod
metadata:
  name: $pod
  labels:
    openkite.maklab.net/role: preview-build
spec:
  restartPolicy: Never
  containers:
    - name: build
      image: $image
      command: ["sleep", "infinity"]
      workingDir: /src
      resources:
        requests:
          cpu: "1"
          memory: 1Gi
        limits:
          memory: 4Gi
      volumeMounts:
        - name: src
          mountPath: /src
  volumes:
    - name: src
      emptyDir: {}
MANIFEST

"$kubectl" -n "$namespace" wait --for=condition=Ready "pod/$pod" --timeout=300s >&2

# The sources travel over the API, not through an image: target/ and .git are
# excluded because a build pod needs manifests, not history or stale artifacts.
tar -C "$root" -cf - \
  --exclude=target --exclude=.git --exclude='tilt/out' \
  Cargo.toml Cargo.lock crates web \
  | "$kubectl" -n "$namespace" exec -i "$pod" -- tar -C /src -xf -

# --release and native: the build pod is the architecture that will run it, which
# is the reason to build in the cluster rather than on the runner.
"$kubectl" -n "$namespace" exec "$pod" -- sh -c "cd /src && cargo build --release -p $crate" >&2

"$kubectl" -n "$namespace" cp "$pod:/src/target/release/$crate" "$out/$crate" >&2

size="$(wc -c <"$out/$crate" | tr -d ' ')"
echo "build-server: $out/$crate (${size} bytes) — compile-and-swap, not a sync"
