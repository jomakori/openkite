#![allow(non_snake_case)]

pub mod bridge;
pub mod cluster;
pub mod components;
pub mod config;
pub mod crud;
pub mod design;
pub mod fuzzy;
pub mod logs;
pub mod menubar;
pub mod metrics;
pub mod network;
pub mod palette;
pub mod plugin_api;
pub mod plugin_host;
pub mod plugin_js;
pub mod pod;
pub mod prometheus;
pub mod promql;
pub mod push;
#[cfg(feature = "desktop")]
pub mod react_spike;
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
pub mod titlebar;
pub mod views;
pub mod workloads;
pub mod yaml;

/// Bootstrap OpenKite: load config, plugins, and kubeconfig, then launch the UI.
#[cfg(feature = "desktop")]
pub fn run() {
    // Route tracing to stderr, not stdout: under wry/WebKit (and headless CI
    // in particular) stdout is not reliably flushed to a redirected log,
    // while stderr is. CI assertions grep app.log for connection state.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    // Load the static (feature-gated) plugins.
    let config = config::OpenKiteConfig::load();
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    let menu_bar_hidden = config.menu_bar == config::MenuBarVisibility::Hide;
    let mut registry = plugin_host::PluginRegistry::new();
    registry.load_static(&config);
    for plugin in registry.plugins() {
        tracing::info!(name = %plugin.metadata().name, "plugin loaded");
    }

    // Collect plugin navigation entries and routes. The write into the
    // router's global signals must happen inside the Dioxus runtime (see
    // below), so compute first, publish later.
    let sections = registry.sidebar_entries();
    let routes = registry
        .routes()
        .into_iter()
        .map(|r| (r.path, r.render))
        .collect();

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

    // Install the plugin bridge before launch: the app shell's
    // `/openkite` asset handler reads it via `runtime::bridge()`.
    let bridge = match cluster.client() {
        Some(client) => crate::bridge::Bridge::connected(client.clone()),
        None => crate::bridge::Bridge::new(),
    };

    // Discover JS plugins; the shell evals their bundles after mount and
    // their `register` POSTs flow back through the bridge at runtime.
    let root = plugin_js::plugins_dir();
    let (bundles, errors) = plugin_js::collect_bundles(&root, |name| config.is_enabled(name));
    for error in &errors {
        tracing::warn!(error = %error, "js plugin discovery failed");
    }
    tracing::info!(count = bundles.len(), "js plugins discovered");

    // Launch with the bridge bootstrap injected into the page head: the
    // inline style loads the shell chrome, the script defines `window.openkite`
    // before any plugin bundle evaluates.
    let head = bootstrap_head();

    // Dioxus global signals are backed by the *runtime* (not process-wide)
    // in 0.7.10 — reading or writing them outside an active runtime panics.
    // Bootstrap data is therefore published inside the VirtualDom's runtime,
    // right after it is created and before the desktop event loop starts.
    let client = cluster.client().cloned();
    let active = cluster.active().map(str::to_string);
    let contexts = cluster.contexts().to_vec();
    let _ = cluster::SHARED.set(tokio::sync::Mutex::new(cluster));

    #[allow(unused_mut)]
    let mut desktop_config = dioxus::desktop::Config::new().with_custom_head(head);

    // OKT-99: own the platform menu bar so the palette can toggle it at
    // runtime. On Linux/Windows the host passes its own menu (or `None` to
    // suppress the default when the persisted setting is `hide`); macOS keeps
    // dioxus-desktop's default because its global menu bar cannot be hidden
    // and carries the cut/copy/paste accelerators.
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

    // OKT-100: apply the persisted OS decoration theme override at window
    // creation. `System` is tao's `None` (follow the OS), so only an explicit
    // Light/Dark reconstructs the builder; dioxus-desktop's own default stays
    // untouched otherwise.
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    if config.title_bar_theme != config::TitleBarTheme::System {
        desktop_config =
            desktop_config.with_window(titlebar::window_builder(config.title_bar_theme));
    }

    let vdom = dioxus::prelude::VirtualDom::new(router::app);
    vdom.in_runtime(|| {
        router::install_plugins(sections, routes);
        crate::runtime::set_client(client);
        if let Some(active) = active {
            crate::runtime::set_context(Some(active));
        }
        crate::runtime::set_bridge(bridge);
        // Refresh cluster metadata (namespaces + Prometheus) so the chips
        // and status bar are populated on first render. Only when connected.
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
/// stylesheet plus the `window.openkite` bridge script, inlined so shell
/// chrome paints before any plugin bundle evaluates. Pure — the sole
/// sub-logic of [`run`] that is testable without a desktop event loop.
#[cfg(feature = "desktop")]
fn bootstrap_head() -> String {
    format!(
        "<style>{}</style>\n<script>{}</script>",
        include_str!("../assets/main.css"),
        plugin_api::OPENKITE_BRIDGE_JS,
    )
}

#[cfg(all(test, feature = "desktop"))]
mod tests {
    use super::*;

    // `run()` boots a desktop Dioxus event loop (VirtualDom + wry webview +
    // tokio runtime + tracing subscriber) and structurally cannot mount
    // headless: the tracing subscriber is process-global init-once, the
    // Dioxus global signals are runtime-bound, and cluster connect needs a
    // kubeconfig. Its pure sub-logic is extracted and pinned here instead.
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
        // The stylesheet supplies the shell chrome (`.app-shell` rule) and
        // the script defines `window.openkite` before plugins evaluate.
        assert!(head.contains(".app-shell"));
        assert!(head.contains("window.openkite"));
        assert!(head.contains(&format!(
            "<script>{}</script>",
            plugin_api::OPENKITE_BRIDGE_JS
        )));
    }
}
