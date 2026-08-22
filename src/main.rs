#![allow(non_snake_case)]

mod cluster;
mod config;
mod plugin_host;

use dioxus::prelude::*;

fn main() {
    tracing_subscriber::fmt::init();

    // Plugin host: load the static (feature-gated) plugins.
    let config = config::OpenKiteConfig::load();
    let mut registry = plugin_host::PluginRegistry::new();
    registry.load_static(&config);
    for plugin in registry.plugins() {
        tracing::info!(name = %plugin.metadata().name, "plugin loaded");
    }

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

    dioxus::launch(App);
}

/// Root component — sidebar shell + placeholder content.
/// Core routes and plugin route merging land in the sidebar/router ticket (OKT-7).
fn App() -> Element {
    rsx! {
        div { class: "app-shell",
            aside { class: "sidebar",
                h1 { class: "brand", "OpenKite" }
                span { class: "tagline", "Kubernetes from above." }
                nav { class: "nav" }
            }
            main { class: "content",
                h2 { "No cluster connected" }
                p { "Connect a kubeconfig context to begin." }
            }
        }
    }
}
