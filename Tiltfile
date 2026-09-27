load('ext://helm_resource', 'helm_resource')

pr = os.getenv('OPENKITE_PR', '')
dev_loop = os.getenv('OPENKITE_DEV_LOOP', '1') != '0'
chart = os.getenv('OPENKITE_CHART', '../gke_GitOps/apps/helm')

if pr != '':
    if not os.path.exists(chart):
        fail('No chart at ' + chart + '. Point OPENKITE_CHART at the gke_GitOps apps/helm checkout.')

    namespace = 'openkite-pr' + pr
    host = 'pr' + pr + '-openkite.maklab.net'
    base = os.getenv('OPENKITE_BASE_TAG', local('git tag --merged HEAD --sort=-v:refname --list "v*" | head -1'))

    if base == '':
        fail('No release tag is an ancestor of this branch. Run: git fetch --tags')

    if not os.path.exists('web/dist'):
        local('bash web/build.sh')

    flags = [
        '--create-namespace',
        '--set=appName=openkite',
        '--set=openkite.namespaceOverride=' + namespace,
        '--set=openkite.createNamespace=true',
        '--set=openkite.enable_domain=true',
        '--set=openkite.enable_staging=false',
        '--set=openkite.environments.production.tag=' + base,
        '--set=openkite.environments.production.subdomain=pr' + pr + '-openkite',
        '--set=openkite.environments.production.dopplerConfig=svc_openagent',
        '--set=openkite.environments.staging.dopplerConfig=svc_openagent',
    ]

    # Helm ignores a value key the chart revision does not know, which would render this preview at prod coordinates.
    rendered = local(
        'helm template openkite-preview-' + pr + ' ' + chart + ' ' + ' '.join(flags)
        + ' 2>/dev/null || true',
        echo_off=True,
    )
    for needle in [namespace, host, base]:
        if needle not in rendered:
            fail('The chart did not render ' + needle + '. It is stale or the values moved; refusing to install.')

    helm_resource(
        'openkite-preview',
        chart,
        namespace=namespace,
        release_name='openkite-preview-' + pr,
        flags=flags,
        deps=['web/dist'],
        container_selector='openkite',
        live_update=[
            initial_sync(),
            sync('./web/dist', '/usr/share/nginx/html'),
        ],
        port_forwards=['8080:8080'],
        links=['https://' + host],
    )

    local_resource(
        'openkite-bundle',
        cmd='bash web/build.sh',
        deps=['web/src', 'web/index.html', 'web/index.web.html', 'web/vite.config.ts', 'web/package.json'],
    )

if dev_loop:
    local_resource(
        'cargo-check',
        cmd='cargo check --workspace',
        deps=['Cargo.toml', 'Cargo.lock', 'crates'],
    )

    local_resource(
        'openkite-host',
        serve_cmd='OPENKITE_ADDR=0.0.0.0:8090 cargo run -p openkite-web',
        deps=['Cargo.toml', 'Cargo.lock', 'crates'],
        links=['http://localhost:8090'],
    )

    local_resource(
        'openkite-ui',
        serve_cmd='cd crates/openkite-desktop && dx serve',
        deps=['crates', 'web/src', 'Cargo.toml', 'Cargo.lock'],
    )

if pr == '' and not dev_loop:
    fail('Nothing to run: set OPENKITE_PR=<number> for the preview loop, '
         + 'or leave OPENKITE_DEV_LOOP unset for the dev loop.')
