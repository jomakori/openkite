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

# --- Desktop app: `dx serve` (Dioxus dev server + hot reload) ---
# Runs on the host (it's a GUI). Needs the `dx` CLI (`cargo install dioxus-cli`).
# On macOS Dioxus uses the native webview, so no extra system deps.
local_resource(
    'openkite-desktop',
    serve_cmd='dx serve',
    deps=['src', 'crates', 'assets', 'Dioxus.toml', 'Cargo.toml'],
)

# --- Sample workloads, deployed to the cluster Tilt is connected to ---
# (nginx, podinfo, crashloop) — what the OpenKite UI renders against.
k8s_yaml([
    'dev/manifests/nginx-deployment.yaml',
    'dev/manifests/podinfo.yaml',
    'dev/manifests/crashloop-pod.yaml',
])

# TODO(metrics ticket): k8s_yaml metrics-server so the metrics widget has data.
# TODO(plugins ticket): plugin build/install local_resources (auto_init=False).
