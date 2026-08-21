# OpenKite dev loop — `tilt up` on a machine with Docker + Tilt installed.
#
# `cargo-check` compiles the workspace inside the `openkite-dev` container
# and re-runs on every source change. The cargo registry and target dir are
# cached in named volumes, so only the first run downloads dependencies.

docker_build(
    'openkite-dev',
    'dev',
    dockerfile='dev/Dockerfile',
    only=['dev/Dockerfile'],
)

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
