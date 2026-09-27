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

cleanup() {
  "$kubectl" -n "$namespace" delete pod "$pod" --ignore-not-found --wait=false >/dev/null 2>&1 || true
}
trap cleanup EXIT

mkdir -p "$out"

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

tar -C "$root" -cf - \
  --exclude=target --exclude=.git --exclude='tilt/out' \
  Cargo.toml Cargo.lock crates web \
  | "$kubectl" -n "$namespace" exec -i "$pod" -- tar -C /src -xf -

"$kubectl" -n "$namespace" exec "$pod" -- sh -c "cd /src && cargo build --release -p $crate" >&2

"$kubectl" -n "$namespace" cp "$pod:/src/target/release/$crate" "$out/$crate" >&2

size="$(wc -c <"$out/$crate" | tr -d ' ')"
echo "build-server: $out/$crate (${size} bytes) — compile-and-swap, not a sync"
