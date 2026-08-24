#![allow(non_snake_case)]

pub mod cluster;
pub mod components;
pub mod config;
pub mod logs;
pub mod metrics;
pub mod plugin_host;
pub mod router;
pub mod runtime;
pub mod secrets;
pub mod state;
pub mod theme;
pub mod views;
pub mod workloads;

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

    // Publish the active client to the UI before launching.
    crate::runtime::set_client(cluster.client().cloned());

    dioxus::launch(router::app);
}
