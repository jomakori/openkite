#![allow(non_snake_case)]

mod config;
mod plugin_host;

use dioxus::prelude::*;

fn main() {
    tracing_subscriber::fmt::init();

    // Plugin host: load the static (feature-gated) plugins. No bundled
    // plugins until Phase 2 — the registry gets wired into the UI in the
    // sidebar/router ticket (OKT-7) and handed a PluginContext on cluster
    // connect in the kube factory ticket (OKT-6).
    let config = config::OpenKiteConfig::load();
    let mut registry = plugin_host::PluginRegistry::new();
    registry.load_static(&config);
    for plugin in registry.plugins() {
        tracing::info!(name = %plugin.metadata().name, "plugin loaded");
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
