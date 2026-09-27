# ── OpenKite: two loops, one Tiltfile ────────────────────────────────────────
#
# preview — an environment for a PR, at pr<N>-openkite.maklab.net, started from
#           an ALREADY-RELEASED image (the newest release tag that is an ancestor
#           of this branch) with this branch's built artifacts overlaid by
#           live_update. Kubernetes API only: no Docker daemon, no image build,
#           no registry write, nothing to prune when the PR closes. Enable it
#           with OPENKITE_PR=<N>.
#
#           Previews prove the change, staging proves the build, prod proves the
#           release. Read tilt/README.md before changing this path.
#
# dev     — the laptop loop: a docker_build dev image to run `cargo check` in,
#           plus `dx serve` for the UI layer. OPENKITE_DEV_LOOP=0 omits it, which
#           is what a runner without a Docker daemon sets: a docker_build defined
#           here would otherwise be built by `tilt up`.
#
# Putting the preview first is deliberate: `tilt up` on a laptop with Docker
# still gets both loops, and a Docker-less runner gets only what it needs.

pr = os.getenv('OPENKITE_PR', '')
dev_loop = os.getenv('OPENKITE_DEV_LOOP', '1') != '0'

# ── preview ─────────────────────────────────────────────────────────────────
# No docker_build, no registry login, no cluster credentials in this file: the
# kubeconfig comes from the environment, and the base image is pulled by the
# cluster, not pushed by the runner.
if pr != '':
    namespace = os.getenv('OPENKITE_NAMESPACE', 'openkite-pr' + pr)
    host = os.getenv('OPENKITE_PREVIEW_HOST', 'pr' + pr + '-openkite.maklab.net')
    # Matched against the applied manifest's container image. No tag on purpose:
    # the base tag is chosen at apply time by tilt/select-base.sh, so a new
    # release is picked up by a re-apply rather than by editing this file.
    image_repository = os.getenv('OPENKITE_IMAGE_REPOSITORY', 'ghcr.io/jomakori/openkite')
    sync_root = os.getenv('OPENKITE_SYNC_ROOT', '/usr/share/nginx/html')

    # tilt/out/bundle is produced by tilt/build-bundle.sh (no Docker involved).
    # Creating it here keeps a fresh checkout from failing on a missing deps path.
    if not os.path.exists('tilt/out/bundle'):
        local('mkdir -p tilt/out/bundle', quiet=True)

    coordinates = {
        'OPENKITE_PR': pr,
        'OPENKITE_NAMESPACE': namespace,
    }

    k8s_custom_deploy(
        'openkite-preview',
        # apply_cmd renders the app spec at PR coordinates and applies it; its
        # stdout is the applied YAML, which is how Tilt learns what it manages.
        # delete_cmd reverses it, namespace included.
        apply_cmd='./tilt/preview-apply.sh',
        delete_cmd='./tilt/preview-delete.sh',
        apply_env=coordinates,
        delete_env=coordinates,
        deps=[
            'tilt/preview-apply.sh',
            'tilt/preview-delete.sh',
            'tilt/preview-render.sh',
            'tilt/select-base.sh',
            'tilt/out/bundle',
        ],
        image_selector=image_repository,
        live_update=[
            # The base image already carries a working copy of the bundle, so the
            # first sync is a full one: the pod serves this branch from the moment
            # it is running, not only after the first file edit. initial_sync() is
            # also what makes Live Update legal against a pre-built image — Tilt
            # refuses a sync-only update whose container it did not build.
            initial_sync(),
            sync('./tilt/out/bundle', sync_root),
            # Rust that runs in the container is NOT here on purpose: a compiled
            # binary's behaviour cannot be carried by a file sync, and the
            # released image today is a static server with no host binary to
            # replace. The compile-and-swap path is tilt/build-server.sh, which
            # compiles in a temporary in-cluster build pod; the two steps it
            # needs once the image carries the host are
            #   sync('./tilt/out/bin', '<the image's bin dir>'),
            #   restart_container(),
            # Adding them now would sync into a path that does not exist.
        ],
    )

    k8s_resource(
        'openkite-preview',
        port_forwards='8080:8080',
        links=['https://' + host],
    )

# ── dev (laptop; needs a Docker daemon) ─────────────────────────────────────
if dev_loop:
    docker_build(
        'openkite-dev',
        'tilt/dev',
        dockerfile='tilt/dev/Dockerfile',
        only=['tilt/dev/Dockerfile'],
    )

    # Compile-check the workspace in the container CI uses, with target/ and the
    # cargo registry in named volumes so an iteration does not re-download the
    # world. Re-runs on source change.
    local_resource(
        'cargo-check',
        cmd='docker run --rm '
            '-v "$(pwd)":/app -w /app '
            '-v openkite-target:/app/target '
            '-v openkite-cargo:/usr/local/cargo/registry '
            'openkite-dev cargo check --workspace',
        deps=['Cargo.toml', 'Cargo.lock', 'crates'],
        resource_deps=['openkite-dev'],
    )

    # The UI layer: the Dioxus dev server with hot reload. It runs on the host
    # (it is a GUI — the desktop renderer is the platform webview), so this
    # resource is the one thing here a Docker-less runner could not run even if
    # it wanted to. `dx` comes from `cargo install dioxus-cli`.
    local_resource(
        'openkite-ui',
        serve_cmd='cd crates/openkite-desktop && dx serve',
        deps=['crates', 'web/src', 'Cargo.toml', 'Cargo.lock'],
    )

if pr == '' and not dev_loop:
    fail('Nothing to run: set OPENKITE_PR=<number> for the preview loop, '
         + 'or leave OPENKITE_DEV_LOOP unset for the dev loop.')
