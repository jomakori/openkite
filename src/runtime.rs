//! Shared runtime state bridging `run()` (bootstrap) to the UI views.

use dioxus::prelude::*;
use k8s_openapi::api::core::v1::{Namespace, Service};
use kube::{Api, Client};
use std::sync::{Arc, OnceLock};

use crate::bridge::Bridge;
use crate::cluster::ClusterState;

/// The active cluster client, published by `run()` after connect and read by
/// views that need a live `Api`.
pub static CLIENT: GlobalSignal<Option<Client>> = Signal::global(|| None);

/// The active context name, published by `run()` (status bar/footer).
pub static CONTEXT: GlobalSignal<Option<String>> = Signal::global(|| None);

/// Every kubeconfig context name, for the multi-context selector.
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

/// The cluster state, shared for live context switching. Tokio mutex: the
/// guard is held across the async `connect`, so it must be `Send`.
static CLUSTER: OnceLock<Arc<tokio::sync::Mutex<ClusterState>>> = OnceLock::new();

/// Publish the active client (or `None` when disconnected).
pub fn set_client(client: Option<Client>) {
    *CLIENT.write() = client;
}

/// Publish the active context name (or `None` when disconnected).
pub fn set_context(name: Option<String>) {
    *CONTEXT.write() = name;
}

/// The current client, if connected.
pub fn client() -> Option<Client> {
    CLIENT.read().clone()
}

/// The current context name, if a kubeconfig is loaded.
pub fn context_name() -> Option<String> {
    CONTEXT.read().clone()
}

/// Install the shared cluster state and publish its context list (once, in
/// `run`). Enables the multi-context selector and [`switch_context`].
pub fn install_cluster(state: ClusterState) {
    *CONTEXTS.write() = state.contexts().to_vec();
    let _ = CLUSTER.set(Arc::new(tokio::sync::Mutex::new(state)));
}

/// Switch the active context: reconnect, republish the client + context, and
/// refresh cluster metadata (namespaces, Prometheus). Spawned on the ambient
/// tokio runtime the UI already runs on; errors log and leave the previous
/// connection in place.
pub fn switch_context(name: String) {
    let Some(cluster) = CLUSTER.get().cloned() else {
        return;
    };
    tokio::spawn(async move {
        let mut state = cluster.lock().await;
        match state.connect(&name).await {
            Ok(client) => {
                set_client(Some(client.clone()));
                set_context(Some(name));
                refresh_cluster_meta(&client).await;
            }
            Err(err) => tracing::error!(context = %name, error = ?err, "context switch failed"),
        }
    });
}

/// Toggle a namespace in the multi-select set.
pub fn toggle_namespace(ns: String) {
    let mut selected = SELECTED_NAMESPACES.write();
    if let Some(pos) = selected.iter().position(|s| *s == ns) {
        selected.remove(pos);
    } else {
        selected.push(ns);
    }
}

/// Refresh the namespace list and Prometheus detection for a client, then
/// prune the namespace selection to what still exists (falling back to
/// `default`). Called after the initial connect and every switch.
pub async fn refresh_cluster_meta(client: &Client) {
    match Api::<Namespace>::all(client.clone())
        .list(&Default::default())
        .await
    {
        Ok(list) => {
            let names: Vec<String> = list
                .items
                .iter()
                .filter_map(|n| n.metadata.name.clone())
                .collect();
            *NAMESPACES.write() = names.clone();
            let mut selected = SELECTED_NAMESPACES.write();
            crate::shell::prune_namespaces(&mut selected, &names);
        }
        Err(err) => tracing::warn!(error = ?err, "namespace list failed"),
    }
    match Api::<Service>::all(client.clone())
        .list(&Default::default())
        .await
    {
        Ok(list) => *PROMETHEUS.write() = crate::prometheus::detect_prometheus(&list.items),
        Err(err) => tracing::warn!(error = ?err, "service list failed"),
    }
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
