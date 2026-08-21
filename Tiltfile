# OpenKite dev loop — `tilt up` on a machine with Docker + Tilt installed.
#
# Prereqs: create the cluster first — `./dev/k3d-create.sh`, then
# `KUBECONFIG=dev/.kube/config tilt up`.

# --- Rust dev image (toolchain + Dioxus-desktop system deps) ---
docker_build(
    'openkite-dev',
    'dev',
    dockerfile='dev/Dockerfile',
    only=['dev/Dockerfile'],
)

# --- Compile-check in a container; re-runs on source change ---
local_resource(
    'cargo-check',
    cmd='docker run --rm '
        '-v "$(pwd)":/app -w /app '
        '-v openkite-target:/app/target '
        '-v openkite-cargo:/usr/local/cargo/registry '
        'openkite-dev cargo check --workspace',
    deps=['Cargo.toml', 'Cargo.lock', 'src', 'crates'],
    resource_deps=['openkite-dev'],
)

# --- Sample workloads, deployed to the cluster Tilt is connected to ---
# (nginx, podinfo, crashloop) — what the OpenKite UI renders against.
k8s_yaml([
    'dev/manifests/nginx-deployment.yaml',
    'dev/manifests/podinfo.yaml',
    'dev/manifests/crashloop-pod.yaml',
])
