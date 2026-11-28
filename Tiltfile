load('ext://restart_process', 'docker_build_with_restart')

pr = os.getenv('OPENKITE_PR', '')
dev_loop = os.getenv('OPENKITE_DEV_LOOP', '1') != '0'
chart = os.getenv('OPENKITE_CHART', '../gke_GitOps/apps/helm')
image = 'ghcr.io/jomakori/openkite'
sha7 = os.getenv('OPENKITE_SHA7', '')[:7]

context = os.getenv('OPENKITE_CONTEXT', '')
if context != '':
    allow_k8s_contexts(context)

if pr != '':
    if not os.path.exists(chart):
        fail('No chart at ' + chart + '. Point OPENKITE_CHART at the gke_GitOps apps/helm checkout.')

    namespace = 'openkite-pr' + pr
    host = 'pr' + pr + '-openkite.maklab.net'

    values = []
    if dev_loop:
        tag = 'dev'
        values = ['tilt-dev-values.yaml']
        docker_build_with_restart(
            image + ':' + tag,
            '.',
            dockerfile='Dockerfile.dev',
            entrypoint='/src/target/release/openkite-web',
            live_update=[
                sync('./crates', '/src/crates'),
                sync('./Cargo.toml', '/src/Cargo.toml'),
                sync('./Cargo.lock', '/src/Cargo.lock'),
                run('cargo build --release -p openkite-web'),
            ],
            ignore=['./target', './.git'],
        )
    else:
        if sha7 == '':
            fail('Set OPENKITE_SHA7 to the 7-character head sha so the preview can pull the staging image.')
        tag = 'pr-' + pr + '-' + sha7

    flags = [
        '--set=appName=openkite',
        '--set=openkite.namespaceOverride=' + namespace,
        '--set=openkite.createNamespace=true',
        '--set=openkite.enable_domain=true',
        '--set=openkite.enable_staging=false',
        '--set=openkite.environments.production.tag=' + tag,
        '--set=openkite.environments.production.subdomain=pr' + pr + '-openkite',
        '--set=openkite.environments.production.dopplerConfig=svc_openagent',
        '--set=openkite.environments.staging.dopplerConfig=svc_openagent',
    ]

    # allow: Helm ignores value keys the chart revision doesn't know, which would place the preview at production coords
    parts = ['helm', 'template', 'openkite-preview-' + pr, chart]
    for path in values:
        parts += ['-f', path]
    parts += flags
    rendered = str(local(' '.join(parts) + ' 2>/dev/null || true', echo_off=True))
    for needle in [namespace, host, tag]:
        if needle not in rendered:
            fail('The chart did not render ' + needle + '. It is stale or the values moved; refusing to install.')

    if dev_loop:
        # allow: the cache mount exists only for the dev loop, so the chart-render assertion above can't see it
        if 'cargo-cache' not in rendered:
            fail('The chart did not render the cargo-cache volume. The dev loop needs the emptyDir seam.')

    k8s_yaml(helm(
        chart,
        name='openkite-preview-' + pr,
        namespace=namespace,
        values=values,
        set=[flag[len('--set='):] for flag in flags],
    ))
    k8s_resource(
        'openkite-production',
        port_forwards=['8080:8080'],
        links=['https://' + host],
    )

if dev_loop:
    local_resource(
        'cargo-check',
        cmd='cargo check --workspace',
        deps=['Cargo.toml', 'Cargo.lock', 'crates'],
    )

if pr == '' and not dev_loop:
    fail('Nothing to run: set OPENKITE_PR=<number> for the preview loop, '
         + 'or leave OPENKITE_DEV_LOOP unset for the dev loop.')
