//! Shared runtime state bridging `run()` (bootstrap) to the UI views.

use dioxus::prelude::*;
use k8s_openapi::api::core::v1::{Namespace, Pod, Service};
use kube::{Api, Client};
use std::sync::{Arc, OnceLock};

use crate::bridge::Bridge;

// The console's overlay and cluster-context state is its own: it lives in
// `openkite_ui::runtime` so the crate's components read it on both hosts.
// Re-exported here because the shell, the host adapters and this crate's tests
// address them through `runtime::`.
pub use openkite_ui::runtime::{
    clear_crud_target, context_name, open_delete_for, open_editor_for, open_new_for,
    open_scale_for, set_context, set_contexts, set_gateway, set_published_capabilities, CrudTarget,
    CONTEXT, CONTEXTS, CRUD_TARGET,
};

/// The active cluster client, published by `run()` after connect and read by
/// views that need a live `Api`.
pub static CLIENT: GlobalSignal<Option<Client>> = Signal::global(|| None);

/// Namespaces on the active cluster (for the multi-select chips).
pub static NAMESPACES: GlobalSignal<Vec<String>> = Signal::global(Vec::new);

/// The selected namespaces (defaults to `["default"]`).
pub static SELECTED_NAMESPACES: GlobalSignal<Vec<String>> =
    Signal::global(|| vec!["default".into()]);

/// Detected Prometheus service name, if any (status-bar indicator).
pub static PROMETHEUS: GlobalSignal<Option<String>> = Signal::global(|| None);

/// The pod currently displayed in the detail slide-over (None = closed).
pub static SELECTED_POD: GlobalSignal<Option<Pod>> = Signal::global(|| None);

/// Mirror of the bridge's registration store: refreshed by the
/// `/openkite` asset handler after register POSTs; the sidebar and status
/// footer render from it.
pub static REGISTRATIONS: GlobalSignal<crate::plugin_api::RegistrationStore> =
    Signal::global(crate::plugin_api::RegistrationStore::new);

/// The current path the Dioxus router is rendering, published by the host
/// when the route changes and read by JS-side consumers as a fallback when
/// the `document::eval` for `_renderRoute` has not fired yet.
pub static CURRENT_ROUTE: GlobalSignal<String> = Signal::global(String::new);

/// The plugin bridge, shared between bootstrap and the app shell's asset
/// handler. `OnceLock`: set once before launch, read from the
/// handler thread and the UI alike; never swapped in place.
static BRIDGE: OnceLock<Arc<Bridge>> = OnceLock::new();

/// Publish the active client (or `None` when disconnected).
pub fn set_client(client: Option<Client>) {
    *CLIENT.write() = client;
}

/// The current client, if connected.
pub fn client() -> Option<Client> {
    CLIENT.read().clone()
}

/// Install the plugin bridge (idempotent: first write wins).
pub fn set_bridge(bridge: Bridge) {
    let _ = BRIDGE.set(Arc::new(bridge));
}

/// The shared bridge, if bootstrap has installed it.
pub fn bridge() -> Option<Arc<Bridge>> {
    BRIDGE.get().cloned()
}

/// Discovered JS plugin bundles for the shell to eval (set once in `run`).
static JS_PLUGINS: OnceLock<Vec<crate::plugin_js::JsBundle>> = OnceLock::new();

/// Publish the discovered JS plugin bundles (idempotent: first write wins).
pub fn set_js_plugins(bundles: Vec<crate::plugin_js::JsBundle>) {
    let _ = JS_PLUGINS.set(bundles);
}

/// The discovered JS plugin bundles (empty before bootstrap).
pub fn js_plugins() -> Vec<crate::plugin_js::JsBundle> {
    JS_PLUGINS.get().cloned().unwrap_or_default()
}

/// Ping channel for applying persisted settings on the Dioxus side.
///
/// Settings are persisted from the SSR/wasm console (the same `OpenKiteConfig`
/// the desktop reads). The persisted-config writer runs on the host
/// main-thread tokio worker, where `dioxus::desktop::window()` (a Dioxus
/// context lookup) and the Dioxus runtime are unavailable. It only *pings*
/// this channel; the receiver started by `router::AppShell` reads the
/// persisted config and applies the OS-level settings in-runtime.
static SETTINGS_TX: OnceLock<tokio::sync::mpsc::UnboundedSender<()>> = OnceLock::new();

/// Start the settings-apply receiver (idempotent; first caller wins).
pub fn install_settings_apply() -> Option<tokio::sync::mpsc::UnboundedReceiver<()>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    match SETTINGS_TX.set(tx) {
        Ok(()) => Some(rx),
        Err(_) => None,
    }
}

/// Ask the Dioxus-side receiver to apply the persisted OS settings.
///
/// Safe from any thread; a missing receiver (before the first AppShell mount)
/// logs and returns rather than panicking.
pub fn request_settings_apply() {
    match SETTINGS_TX.get() {
        Some(tx) => {
            let _ = tx.send(());
        }
        None => tracing::warn!("settings apply skipped: Dioxus apply task not started yet"),
    }
}

/// Apply the persisted OS-level settings to the live window.
///
/// Must run on the Dioxus side: `menubar` and `titlebar` reach the window
/// through a Dioxus context lookup.
pub fn apply_persisted_os_settings() {
    let config = crate::config::OpenKiteConfig::load();
    if crate::menubar::hideable() {
        crate::menubar::set(config.menu_bar);
    }
    if crate::titlebar::overridable() {
        crate::titlebar::set(config.title_bar_theme);
    }
}

/// Publish the namespace list for the multi-select chips.
pub fn set_namespaces(ns: Vec<String>) {
    *NAMESPACES.write() = ns;
}

/// Publish the selected namespace set.
pub fn set_selected_namespaces(ns: Vec<String>) {
    *SELECTED_NAMESPACES.write() = ns;
}

/// Publish the detected Prometheus service name (or `None`).
pub fn set_prometheus(name: Option<String>) {
    *PROMETHEUS.write() = name;
}

/// Toggle a namespace in the selected set.
pub fn toggle_namespace(ns: String) {
    let mut selected = SELECTED_NAMESPACES.write();
    if let Some(pos) = selected.iter().position(|x| x == &ns) {
        selected.remove(pos);
    } else {
        selected.push(ns);
    }
}

/// Publish the current path the Dioxus router is rendering.
pub fn set_current_route(path: String) {
    *CURRENT_ROUTE.write() = path;
}

/// The current path, or `""` before the first route change.
pub fn current_route() -> String {
    CURRENT_ROUTE.read().clone()
}

/// Refresh cluster metadata: namespace list + Prometheus detection.
pub async fn refresh_cluster_meta(client: &Client) {
    let ns_api: Api<Namespace> = Api::all(client.clone());
    let ns_list: Vec<String> = ns_api
        .list(&kube::api::ListParams::default())
        .await
        .map(|list| {
            list.items
                .into_iter()
                .filter_map(|ns| ns.metadata.name)
                .collect()
        })
        .unwrap_or_default();
    set_namespaces(ns_list);

    let svc_api: Api<Service> = Api::all(client.clone());
    let all_svcs = svc_api
        .list(&kube::api::ListParams::default())
        .await
        .ok()
        .map(|list| {
            list.items
                .into_iter()
                .filter_map(|svc| svc.metadata.name)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let prom = all_svcs
        .iter()
        .find(|name| name.starts_with("prometheus"))
        .cloned();
    set_prometheus(prom);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run a closure with a Dioxus runtime installed on this thread.
    fn with_runtime<O>(f: impl FnOnce() -> O) -> O {
        fn stub() -> Element {
            rsx! { div {} }
        }
        let vdom = VirtualDom::new(stub);
        vdom.in_runtime(f)
    }

    /// Current-thread tokio runtime for building a kube client.
    fn current_thread_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread tokio runtime")
    }

    /// A client pointed at a dead cluster: every request errors, driving the
    /// fallback arms in `refresh_cluster_meta`.
    fn dead_client(rt: &tokio::runtime::Runtime) -> kube::Client {
        let config = kube::Config::new("http://127.0.0.1:1".parse().expect("valid uri"));
        rt.block_on(async { kube::Client::try_from(config).expect("client builds from config") })
    }

    #[test]
    fn client_some_publish_is_readable() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let rt = current_thread_runtime();
        let dead = dead_client(&rt);
        with_runtime(|| {
            set_client(None);
            assert!(client().is_none());
            set_client(Some(dead));
            assert!(client().is_some());
            set_client(None);
            assert!(client().is_none());
        });
    }

    #[test]
    fn refresh_cluster_meta_error_arms_clear_stale_meta() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let rt = current_thread_runtime();
        let client = dead_client(&rt);
        with_runtime(|| {
            set_namespaces(vec!["stale".into()]);
            set_prometheus(Some("stale-prom".into()));
            rt.block_on(refresh_cluster_meta(&client));
            assert!(NAMESPACES.read().is_empty());
            assert_eq!(*PROMETHEUS.read(), None);
        });
    }

    #[test]
    fn bridge_and_js_plugins_once_locks_keep_first_write() {
        with_runtime(|| {
            assert!(bridge().is_none());
            set_bridge(Bridge::new());
            let first = bridge().expect("bridge installed");
            set_bridge(Bridge::new());
            let second = bridge().expect("bridge still installed");
            assert!(Arc::ptr_eq(&first, &second));

            assert!(js_plugins().is_empty());
            set_js_plugins(vec![crate::plugin_js::JsBundle {
                name: "demo".into(),
                entry: std::path::PathBuf::from("/tmp/demo.js"),
            }]);
            let bundles = js_plugins();
            assert_eq!(bundles.len(), 1);
            assert_eq!(bundles[0].name, "demo");
            set_js_plugins(Vec::new());
            assert_eq!(js_plugins().len(), 1);
        });
    }
}
