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
pub mod palette;
pub mod plugin_api;
pub mod plugin_host;
pub mod plugin_js;
pub mod pod;
pub mod prometheus;
pub mod promql;
pub mod router;
pub mod runtime;
pub mod secrets;
pub mod shell;
pub mod state;
pub mod switcher;
pub mod terminal;
// Test-only DOM bridge (OKT-64). Compiled only in dev/test builds —
// release binaries strip it (the bridge answers DOM queries over a
// localhost HTTP port and must never ship).
#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
pub mod test_bridge;
pub mod theme;
pub mod theme_catalog;
pub mod theme_opaline;
pub mod views;
pub mod workloads;
pub mod yaml;

/// Bootstrap OpenKite: load config, plugins, and kubeconfig, then launch the UI.
pub fn run() {
    // Route tracing to stderr, not stdout: under wry/WebKit (and headless CI
    // in particular) stdout is not reliably flushed to a redirected log,
    // while stderr is. CI assertions grep app.log for connection state.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    // Load the static (feature-gated) plugins.
    let config = config::OpenKiteConfig::load();
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
    let head = format!(
        "<style>{}</style>\n<script>{}</script>",
        include_str!("../assets/main.css"),
        plugin_api::OPENKITE_BRIDGE_JS,
    );

    // Dioxus global signals are backed by the *runtime* (not process-wide)
    // in 0.7.10 — reading or writing them outside an active runtime panics.
    // Bootstrap data is therefore published inside the VirtualDom's runtime,
    // right after it is created and before the desktop event loop starts.
    let client = cluster.client().cloned();
    let active = cluster.active().map(str::to_string);
    let contexts = cluster.contexts().to_vec();
    let _ = cluster::SHARED.set(tokio::sync::Mutex::new(cluster));

    // Test-only DOM bridge (OKT-64): listens on OPENKITE_TEST_PORT when
    // set; no-op in production (env unset, module stripped from release
    // builds via cfg(debug_assertions)). Start the HTTP listener now —
    // on the bootstrap runtime's handle, since this thread is not inside
    // a runtime context — and the dioxus-side worker is spawned by the
    // app shell's mount effect once the webview is live.
    #[cfg(debug_assertions)]
    crate::test_bridge::install_bridge(runtime.handle());

    let config = dioxus::desktop::Config::new().with_custom_head(head);
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
    dioxus::desktop::launch::launch_virtual_dom(vdom, config);
}
