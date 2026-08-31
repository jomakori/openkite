#![allow(non_snake_case)]

pub mod bridge;
pub mod cluster;
pub mod components;
pub mod config;
pub mod crud;
pub mod design;
pub mod fuzzy;
pub mod logs;
pub mod metrics;
pub mod network;
pub mod plugin_api;
pub mod plugin_host;
pub mod plugin_js;
pub mod pod;
pub mod prometheus;
pub mod router;
pub mod runtime;
pub mod secrets;
pub mod shell;
pub mod state;
pub mod switcher;
pub mod terminal;
pub mod theme;
pub mod theme_catalog;
pub mod theme_opaline;
pub mod views;
pub mod workloads;
pub mod yaml;

/// Bootstrap OpenKite: load config, plugins, and kubeconfig, then launch the UI.
pub fn run() {
    tracing_subscriber::fmt::init();

    // Load the static (feature-gated) plugins.
    let config = config::OpenKiteConfig::load();
    let mut registry = plugin_host::PluginRegistry::new();
    registry.load_static(&config);
    for plugin in registry.plugins() {
        tracing::info!(name = %plugin.metadata().name, "plugin loaded");
    }

    // Install plugin navigation entries and routes into the router.
    let sections = registry.sidebar_entries();
    let routes = registry
        .routes()
        .into_iter()
        .map(|r| (r.path, r.render))
        .collect();
    router::install_plugins(sections, routes);

    // Load the kubeconfig and connect to the current context. The bootstrap
    // runtime outlives the UI so reflectors and plugin tasks share one handle.
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut cluster = cluster::ClusterState::load().unwrap_or_else(|err| {
        tracing::warn!(error = ?err, "no kubeconfig; starting disconnected");
        cluster::ClusterState::default()
    });
    tracing::info!(contexts = ?cluster.contexts(), "kubeconfig loaded");
    if let Some(active) = cluster.active().map(str::to_string) {
        match runtime.block_on(cluster.connect(&active)) {
            Ok(_) => {
                tracing::info!(context = %active, "cluster connected");
                if let Some(ctx) = cluster.plugin_context(runtime.handle().clone()) {
                    registry.on_cluster_connect(&ctx);
                }
            }
            Err(err) => tracing::error!(context = %active, error = ?err, "cluster connect failed"),
        }
    }

    // Publish the active client + context name to the UI before launching.
    crate::runtime::set_client(cluster.client().cloned());
    if let Some(active) = cluster.active().map(str::to_string) {
        crate::runtime::set_context(Some(active));
    }

    // Install the plugin bridge before launch: the app shell's
    // `/openkite` asset handler reads it via `runtime::bridge()`.
    let bridge = match cluster.client() {
        Some(client) => crate::bridge::Bridge::connected(client.clone()),
        None => crate::bridge::Bridge::new(),
    };
    crate::runtime::set_bridge(bridge);

    // Refresh cluster metadata (namespace list + Prometheus detection)
    // before launching the UI so the namespace chips and status bar are
    // populated on first render.
    if let Some(client) = crate::runtime::client() {
        runtime.block_on(crate::runtime::refresh_cluster_meta(&client));
    }

    // Hand the cluster registry to the UI: the ctrl-tab switcher connects
    // context switches through it, reusing cached clients per context.
    crate::runtime::set_contexts(cluster.contexts().to_vec());
    let _ = cluster::SHARED.set(tokio::sync::Mutex::new(cluster));

    // Discover JS plugins; the shell evals their bundles after mount and
    // their `register` POSTs flow back through the bridge at runtime.
    let root = plugin_js::plugins_dir();
    let (bundles, errors) = plugin_js::collect_bundles(&root, |name| config.is_enabled(name));
    for error in &errors {
        tracing::warn!(error = %error, "js plugin discovery failed");
    }
    tracing::info!(count = bundles.len(), "js plugins discovered");
    crate::runtime::set_js_plugins(bundles);

    // Launch with the bridge bootstrap injected into the page head: the
    // inline style loads the shell chrome, the script defines `window.openkite`
    // before any plugin bundle evaluates.
    let head = format!(
        "<style>{}</style>\n<script>{}</script>",
        include_str!("../assets/main.css"),
        plugin_api::OPENKITE_BRIDGE_JS,
    );
    dioxus::LaunchBuilder::new()
        .with_cfg(dioxus::desktop::Config::new().with_custom_head(head))
        .launch(router::app);
}
