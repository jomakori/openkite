//! Shared runtime state bridging `run()` (bootstrap) to the UI views.

use dioxus::prelude::*;
use k8s_openapi::api::core::v1::{Namespace, Service};
use kube::{Api, Client};
use std::sync::{Arc, OnceLock};

use crate::bridge::Bridge;

/// The active cluster client, published by `run()` after connect and read by
/// views that need a live `Api`.
pub static CLIENT: GlobalSignal<Option<Client>> = Signal::global(|| None);

/// The active context name, published by `run()` (status bar/footer).
pub static CONTEXT: GlobalSignal<Option<String>> = Signal::global(|| None);

/// All kubeconfig context names, published by `run()` (switcher overlay).
pub static CONTEXTS: GlobalSignal<Vec<String>> = Signal::global(Vec::new);

/// Namespaces on the active cluster (for the multi-select chips).
pub static NAMESPACES: GlobalSignal<Vec<String>> = Signal::global(Vec::new);

/// The selected namespaces (defaults to `["default"]`).
pub static SELECTED_NAMESPACES: GlobalSignal<Vec<String>> =
    Signal::global(|| vec!["default".into()]);

/// Detected Prometheus service name, if any (status-bar indicator).
pub static PROMETHEUS: GlobalSignal<Option<String>> = Signal::global(|| None);

/// Mirror of the bridge's registration store: refreshed by the
/// `/openkite` asset handler after register POSTs; the sidebar and status
/// footer render from it.
pub static REGISTRATIONS: GlobalSignal<crate::plugin_api::RegistrationStore> =
    Signal::global(crate::plugin_api::RegistrationStore::new);

/// The plugin bridge, shared between bootstrap and the app shell's asset
/// handler. `OnceLock`: set once before launch, read from the
/// handler thread and the UI alike; never swapped in place.
static BRIDGE: OnceLock<Arc<Bridge>> = OnceLock::new();

/// Publish the active client (or `None` when disconnected).
pub fn set_client(client: Option<Client>) {
    *CLIENT.write() = client;
}

/// Publish the active context name (or `None` when disconnected).
pub fn set_context(name: Option<String>) {
    *CONTEXT.write() = name;
}

/// Publish the kubeconfig context list (ctrl-tab switcher).
pub fn set_contexts(names: Vec<String>) {
    *CONTEXTS.write() = names;
}

/// The current client, if connected.
pub fn client() -> Option<Client> {
    CLIENT.read().clone()
}

/// The current context name, if a kubeconfig is loaded.
pub fn context_name() -> Option<String> {
    CONTEXT.read().clone()
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

/// Refresh cluster metadata: namespace list + Prometheus detection.
pub async fn refresh_cluster_meta(client: &Client) {
    // Namespaces
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

    // Prometheus detection: scan all services for common prometheus names.
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
