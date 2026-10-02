#![allow(non_snake_case)]

#[cfg(feature = "desktop")]
use std::sync::Arc;

#[cfg(feature = "desktop")]
use openkite_api::gateway::Gateway;
#[cfg(feature = "desktop")]
use openkite_host::gateway::KubeGateway;

// The shared halves of the app live in their own crates and are re-exported
// here, so every host-side path (`crate::components::…`, `crate::bridge::…`)
// and every test path (`openkite::…`) keeps working unchanged:
// `openkite-ui` is the console, `openkite-host` the kube-side runtime this host
// runs with.
pub use openkite_host::{bridge, config, push, state};
pub use openkite_ui::{
    components, design, plugin_api, secrets, shell, theme, theme_catalog, theme_opaline,
};

pub mod cluster;
pub mod crud;
pub mod logs;
pub mod menubar;
pub mod metrics;
pub mod network;
pub mod plugin_host;
pub mod plugin_js;
pub mod pod;
pub mod prometheus;
pub mod promql;
pub mod router;
pub mod runtime;
pub mod terminal;
pub mod titlebar;
pub mod version;
pub mod workloads;

/// Bootstrap OpenKite: load config, plugins, and kubeconfig, then launch the UI.
#[cfg(feature = "desktop")]
pub fn run() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let config = config::OpenKiteConfig::load();
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    let menu_bar_hidden = config.menu_bar == config::MenuBarVisibility::Hide;
    let mut registry = plugin_host::PluginRegistry::new();
    registry.load_static(&config);
    for plugin in registry.plugins() {
        tracing::info!(name = %plugin.metadata().name, "plugin loaded");
    }

    let sections = registry.sidebar_entries();
    let routes = registry
        .routes()
        .into_iter()
        .map(|r| (r.path, r.render))
        .collect();

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

    let bridge = match cluster.client() {
        Some(client) => crate::bridge::Bridge::connected(client.clone()),
        None => crate::bridge::Bridge::new(),
    };

    let root = plugin_js::plugins_dir();
    let (bundles, errors) = plugin_js::collect_bundles(&root, |name| config.is_enabled(name));
    for error in &errors {
        tracing::warn!(error = %error, "js plugin discovery failed");
    }
    tracing::info!(count = bundles.len(), "js plugins discovered");

    let head = bootstrap_head();

    let client = cluster.client().cloned();
    let active = cluster.active().map(str::to_string);
    let contexts = cluster.contexts().to_vec();
    let _ = cluster::SHARED.set(tokio::sync::Mutex::new(cluster));

    #[allow(unused_mut)]
    let mut desktop_config = dioxus::desktop::Config::new().with_custom_head(head);

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    if menubar::hideable() {
        let menu = menubar::build_menu();
        menubar::install(menu.clone());
        desktop_config = if menu_bar_hidden {
            desktop_config.with_menu(None::<dioxus::desktop::muda::Menu>)
        } else {
            desktop_config.with_menu(menu)
        };
    } else if menu_bar_hidden {
        tracing::warn!("menu bar cannot be hidden on this platform; keeping the system menu");
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    if config.title_bar_theme != config::TitleBarTheme::System {
        desktop_config =
            desktop_config.with_window(titlebar::window_builder(config.title_bar_theme));
    }

    let vdom = dioxus::prelude::VirtualDom::new(router::app);
    vdom.in_runtime(|| {
        // Publish host capabilities before any gateway exists (disconnected
        // boot). The descriptor is what the shared chrome gates on, so it
        // states what this platform can render rather than what the desktop
        // host is in general.
        let mut caps = openkite_api::capability::Capabilities::in_process();
        caps.native_menu_bar = menubar::hideable();
        caps.title_bar_override = titlebar::overridable();
        crate::runtime::set_published_capabilities(Some(caps));
        router::install_plugins(sections, routes);
        crate::runtime::set_client(client.clone());
        // The console reaches the cluster through the contract, never through
        // the client: install the in-process kube adapter next to it.
        crate::runtime::set_gateway(
            client.map(|client| Arc::new(KubeGateway::new(client)) as Arc<dyn Gateway>),
        );
        if let Some(active) = active {
            crate::runtime::set_context(Some(active));
        }
        crate::runtime::set_bridge(bridge);
        if let Some(client) = crate::runtime::client() {
            runtime.block_on(crate::runtime::refresh_cluster_meta(&client));
        }
        crate::runtime::set_contexts(contexts);
        crate::runtime::set_js_plugins(bundles);
    });
    dioxus::desktop::launch::launch_virtual_dom(vdom, desktop_config);
}

/// Non-desktop builds have no native renderer to launch; the browser UI is the
/// static bundle built from `web/` (see `web/README.md`).
#[cfg(not(feature = "desktop"))]
pub fn run() {}

/// The page-`<head>` bootstrap handed to the desktop window: the shell
/// stylesheet with the vendored typefaces embedded as `data:` URIs (a webview
/// has no origin to fetch them from), plus the `window.openkite` bridge script —
/// inlined so shell chrome paints before any plugin bundle evaluates. Pure — the
/// sole sub-logic of [`run`] that is testable without a desktop event loop.
#[cfg(feature = "desktop")]
fn bootstrap_head() -> String {
    format!(
        "<style>{}</style>\n<script>{}</script>",
        openkite_ui::assets::embedded_css(),
        plugin_api::OPENKITE_BRIDGE_JS,
    )
}

#[cfg(all(test, feature = "desktop"))]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_head_wraps_css_and_bridge_script() {
        let head = bootstrap_head();
        assert!(head.starts_with("<style>"));
        assert!(head.contains("</style>\n<script>"));
        assert!(head.ends_with("</script>"));
    }

    #[test]
    fn bootstrap_head_carries_shell_css_and_openkite_global() {
        let head = bootstrap_head();
        assert!(head.contains(".app {"));
        assert!(head.contains("var(--font-sans)"));
        assert!(head.contains("window.openkite"));
        assert!(head.contains(&format!(
            "<script>{}</script>",
            plugin_api::OPENKITE_BRIDGE_JS
        )));
    }

    #[test]
    fn bootstrap_head_embeds_every_face_instead_of_fetching_it() {
        let head = bootstrap_head();
        let faces = openkite_ui::assets::FACES;
        assert_eq!(head.matches("data:font/woff2;base64,").count(), faces.len());
        assert!(
            !head.contains(openkite_ui::assets::FONT_ROUTE),
            "the window has no origin to serve the faces from"
        );
        for face in faces {
            assert!(
                head.contains(&format!("font-family: \"{}\"", face.family)),
                "{} not declared in the window head",
                face.file
            );
        }
    }
}
