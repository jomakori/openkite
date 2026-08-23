#![allow(non_snake_case)]

mod cluster;
mod components;
mod config;
mod plugin_host;
mod router;

fn main() {
    tracing_subscriber::fmt::init();

    // Plugin host: load the static (feature-gated) plugins.
    let config = config::OpenKiteConfig::load();
    let mut registry = plugin_host::PluginRegistry::new();
    registry.load_static(&config);
    for plugin in registry.plugins() {
        tracing::info!(name = %plugin.metadata().name, "plugin loaded");
    }

    // OKT-7: install plugin navigation + routes into the router.
    let sections = registry.sidebar_entries();
    let routes = registry
        .routes()
        .into_iter()
        .map(|r| (r.path, r.render))
        .collect();
    router::install_plugins(sections, routes);

    // OKT-6: load the kubeconfig and connect to the current context. The
    // bootstrap runtime stays alive for the app's lifetime; OKT-7 shares it
    // with the Dioxus UI so reflectors/plugin tasks drive off one handle.
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

    dioxus::launch(router::app);
}
