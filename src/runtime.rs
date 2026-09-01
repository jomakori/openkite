//! Shared runtime state bridging `run()` (bootstrap) to the UI views.

use dioxus::prelude::*;
use k8s_openapi::api::core::v1::{Namespace, Pod, Service};
use kube::{Api, Client};
use serde_json::Value;
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

/// The resource the CRUD overlay is currently showing, or `None` when the
/// overlay is closed. Dispatched on by [`crate::components::crud_modal::CrudOverlay`].
#[derive(Debug, Clone, PartialEq)]
pub enum CrudTarget {
    /// Edit a live resource; the editor round-trips through `ApiRequest::Get`
    /// and pre-loads with the current manifest.
    Edit { doc: Value, kind: String },
    /// Destructive confirm modal (typed-name gate).
    Delete {
        kind: String,
        namespace: Option<String>,
        name: String,
    },
    /// Non-destructive scale confirm (number input + 2-button row).
    Scale {
        kind: String,
        namespace: Option<String>,
        name: String,
        current_replicas: u32,
    },
    /// Create a new resource; the editor opens with a kind-specific starter.
    New { kind: String },
}

pub static CRUD_TARGET: GlobalSignal<Option<CrudTarget>> = Signal::global(|| None);

/// Open the overlay for one of the four CRUD operations. `None` closes it.
pub fn set_crud_target(target: Option<CrudTarget>) {
    *CRUD_TARGET.write() = target;
}

/// Close the overlay.
pub fn clear_crud_target() {
    set_crud_target(None);
}

/// Open the editor for an existing resource.
pub fn open_editor_for(kind: String, doc: Value) {
    set_crud_target(Some(CrudTarget::Edit { kind, doc }));
}

/// Open the destructive confirm modal.
pub fn open_delete_for(kind: String, namespace: Option<String>, name: String) {
    set_crud_target(Some(CrudTarget::Delete {
        kind,
        namespace,
        name,
    }));
}

/// Open the scale modal.
pub fn open_scale_for(
    kind: String,
    namespace: Option<String>,
    name: String,
    current_replicas: u32,
) {
    set_crud_target(Some(CrudTarget::Scale {
        kind,
        namespace,
        name,
        current_replicas,
    }));
}

/// Open the editor for a brand-new resource.
pub fn open_new_for(kind: String) {
    set_crud_target(Some(CrudTarget::New { kind }));
}

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
