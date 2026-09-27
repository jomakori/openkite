
pr = os.getenv('OPENKITE_PR', '')
dev_loop = os.getenv('OPENKITE_DEV_LOOP', '1') != '0'

if pr != '':
    namespace = os.getenv('OPENKITE_NAMESPACE', 'openkite-pr' + pr)
    host = os.getenv('OPENKITE_PREVIEW_HOST', 'pr' + pr + '-openkite.maklab.net')
    image_repository = os.getenv('OPENKITE_IMAGE_REPOSITORY', 'ghcr.io/jomakori/openkite')
    sync_root = os.getenv('OPENKITE_SYNC_ROOT', '/usr/share/nginx/html')

    if not os.path.exists('tilt/out/bundle'):
        local('mkdir -p tilt/out/bundle')

    coordinates = {
        'OPENKITE_PR': pr,
        'OPENKITE_NAMESPACE': namespace,
    }

    k8s_custom_deploy(
        'openkite-preview',
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
            initial_sync(),
            sync('./tilt/out/bundle', sync_root),
        ],
    )

    k8s_resource(
        'openkite-preview',
        port_forwards='8080:8080',
        links=['https://' + host],
    )

if dev_loop:
    docker_build(
        'openkite-dev',
        'tilt/dev',
        dockerfile='tilt/dev/Dockerfile',
        only=['tilt/dev/Dockerfile'],
    )

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

    local_resource(
        'openkite-ui',
        serve_cmd='cd crates/openkite-desktop && dx serve',
        deps=['crates', 'web/src', 'Cargo.toml', 'Cargo.lock'],
    )

if pr == '' and not dev_loop:
    fail('Nothing to run: set OPENKITE_PR=<number> for the preview loop, '
         + 'or leave OPENKITE_DEV_LOOP unset for the dev loop.')
