//! Bridge runtime (OKT-46): webview ⇄ kube dispatch core.
//!
//! The headless half of the plugin bridge. [`OPENKITE_BRIDGE_JS`](crate::plugin_api::OPENKITE_BRIDGE_JS)
//! turns `openkite.api.*` / `openkite.register*` calls into same-origin
//! `fetch` POSTs against the `/openkite` asset handler; this module parses
//! those envelopes ([`Bridge::handle_post`]) and executes them:
//!
//! - `register` ops merge UI contributions into the
//!   [`RegistrationStore`](crate::plugin_api::RegistrationStore) — the store
//!   the app shell renders from, shared via [`Bridge::store`];
//! - kube ops (`list` / `get` / `watch` / `logs`) dispatch through kube-rs
//!   with the app's client and the user's RBAC; plugins never see cluster
//!   credentials.
//!
//! Transport decision + the dioxus 0.7.10 ipc limitation: see
//! `docs/plugin-architecture.md`. The static Rust plugin registry
//! (`plugin_host`) and the on-disk JS host half (`plugin_js.rs`) are
//! unchanged; mounting this behind the shell view's asset handler is the
//! interactive remainder (OKT-31).

use std::sync::{Arc, Mutex};

use k8s_openapi::api::core::v1::Pod;
use kube::api::LogParams;
use kube::core::DynamicObject;
use kube::discovery::{ApiResource, Discovery, Scope};
use kube::{Api, Client};
use serde_json::{json, Value};

use crate::plugin_api::{
    ApiRequest, ApiResponse, BridgeRequest, RegistrationStore, RouteSpec, SidebarItem, StatusItem,
};

const NO_CLUSTER: &str = "no cluster connected";

/// The shared plugin bridge: optional kube client + plugin UI registrations.
///
/// Built at bootstrap; the shell view captures it behind an `Arc` and mounts
/// the `/openkite` asset handler fronting [`Bridge::handle_post`] (OKT-31).
/// The [`RegistrationStore`] is shared so views render registrations live.
pub struct Bridge {
    client: Option<Client>,
    store: Arc<Mutex<RegistrationStore>>,
}

impl Default for Bridge {
    fn default() -> Self {
        Self {
            client: None,
            store: Arc::new(Mutex::new(RegistrationStore::new())),
        }
    }
}

impl Bridge {
    /// Disconnected bridge: register ops work, kube ops error cleanly.
    pub fn new() -> Self {
        Self::default()
    }

    /// Connected bridge over the app's kube client.
    pub fn connected(client: Client) -> Self {
        Self {
            client: Some(client),
            ..Self::default()
        }
    }

    /// Swap the kube client (cluster switch / disconnect).
    pub fn set_client(&mut self, client: Option<Client>) {
        self.client = client;
    }

    /// The active kube client, if connected.
    pub fn client(&self) -> Option<&Client> {
        self.client.as_ref()
    }

    /// Shared handle to the UI registration store (shell views read this).
    pub fn store(&self) -> Arc<Mutex<RegistrationStore>> {
        self.store.clone()
    }

    /// A point-in-time copy of the registration store, for views that render
    /// from it without holding the lock across render (OKT-31).
    pub fn snapshot(&self) -> RegistrationStore {
        self.store.lock().expect("registration store lock").clone()
    }

    /// Handle one raw POST body from the webview: parse the envelope, then
    /// execute the request. Always answers (never panics) — the fetch side
    /// of the JS bridge turns the response into reject/resolve.
    pub async fn handle_post(&self, body: &str) -> ApiResponse {
        let envelope: BridgeRequest = match serde_json::from_str(body) {
            Ok(envelope) => envelope,
            Err(err) => {
                return ApiResponse::Error {
                    error: format!("parse envelope: {err}"),
                }
            }
        };
        self.execute(&envelope.plugin, envelope.request).await
    }

    /// Execute one bridge request against the store and (when needed) the
    /// cluster. `register` ops never touch the cluster; `exec` is deferred.
    pub async fn execute(&self, plugin: &str, request: ApiRequest) -> ApiResponse {
        let outcome: Result<Value, String> = match request {
            ApiRequest::Exec { .. } => Err("exec is not supported yet".into()),
            ApiRequest::Register { kind, payload } => {
                let mut store = self.store.lock().expect("registration store lock");
                apply_register(&mut store, plugin, &kind, payload)
            }
            ApiRequest::List { kind, ns } => match self.client.clone() {
                Some(client) => list_resource(&client, &kind, ns.as_deref()).await,
                None => Err(NO_CLUSTER.into()),
            },
            // Snapshot until reflector-backed plugin views land; the wire
            // contract (promise resolving once) stays the same either way.
            ApiRequest::Watch { kind, ns } => match self.client.clone() {
                Some(client) => list_resource(&client, &kind, ns.as_deref()).await,
                None => Err(NO_CLUSTER.into()),
            },
            ApiRequest::Get { kind, ns, name } => match self.client.clone() {
                Some(client) => get_resource(&client, &kind, &ns, &name).await,
                None => Err(NO_CLUSTER.into()),
            },
            ApiRequest::Logs {
                name,
                ns,
                container,
            } => match self.client.clone() {
                Some(client) => pod_logs(&client, &name, &ns, container).await,
                None => Err(NO_CLUSTER.into()),
            },
        };
        match outcome {
            Ok(result) => ApiResponse::Ok { result },
            Err(error) => ApiResponse::Error { error },
        }
    }
}

/// Merge one UI contribution into the plugin's registration.
///
/// Re-registers are appends, not upserts — the host clears a plugin's entry
/// before re-evaluating its bundle on hot reload (`RegistrationStore::remove`).
/// A failed validation leaves the previous entry untouched.
pub fn apply_register(
    store: &mut RegistrationStore,
    plugin: &str,
    kind: &str,
    payload: Value,
) -> Result<Value, String> {
    let mut registration = store.get(plugin).cloned().unwrap_or_default();
    match kind {
        "sidebar" => {
            let item: SidebarItem = serde_json::from_value(payload)
                .map_err(|err| format!("invalid sidebar item: {err}"))?;
            registration.sidebar.push(item);
        }
        "route" => {
            let route: RouteSpec =
                serde_json::from_value(payload).map_err(|err| format!("invalid route: {err}"))?;
            registration.routes.push(route);
        }
        "status" => {
            let item: StatusItem = serde_json::from_value(payload)
                .map_err(|err| format!("invalid status item: {err}"))?;
            registration.status.push(item);
        }
        other => return Err(format!("unknown registration kind '{other}'")),
    }
    registration.validate()?;
    store.upsert(plugin, registration);
    Ok(json!({ "registered": kind }))
}

/// Resolve a bare plugin-supplied kind string ("pods", "Pod", "applications")
/// against live discovery.
///
/// The string doesn't pin group/version, so a oneshot GVK lookup won't do:
/// run full discovery and match the preferred version of every group
/// (plural or kind, case-insensitive). The core group (`""`) sorts first,
/// so "events" resolves to core before `events.k8s.io`.
pub async fn resolve_resource(client: &Client, kind: &str) -> Result<(ApiResource, Scope), String> {
    let discovery = Discovery::new(client.clone())
        .run()
        .await
        .map_err(|err| format!("discovery: {err}"))?;
    for group in discovery.groups_alphabetical() {
        for (resource, capabilities) in group.recommended_resources() {
            if resource.plural.eq_ignore_ascii_case(kind)
                || resource.kind.eq_ignore_ascii_case(kind)
            {
                return Ok((resource, capabilities.scope));
            }
        }
    }
    Err(format!("unknown resource kind '{kind}'"))
}

/// List a kind; `ns: None` (or empty) = every namespace the client can see.
pub async fn list_resource(client: &Client, kind: &str, ns: Option<&str>) -> Result<Value, String> {
    let (ar, scope) = resolve_resource(client, kind).await?;
    let ns = ns.filter(|ns| !ns.is_empty());
    let list = match (scope, ns) {
        (Scope::Namespaced, Some(ns)) => {
            Api::<DynamicObject>::namespaced_with(client.clone(), ns, &ar)
        }
        _ => Api::<DynamicObject>::all_with(client.clone(), &ar),
    }
    .list(&Default::default())
    .await
    .map_err(|err| format!("list {kind}: {err}"))?;
    serde_json::to_value(&list).map_err(|err| format!("serialize {kind} list: {err}"))
}

/// Get one resource; namespaced kinds require a namespace.
pub async fn get_resource(
    client: &Client,
    kind: &str,
    ns: &str,
    name: &str,
) -> Result<Value, String> {
    let (ar, scope) = resolve_resource(client, kind).await?;
    let api = match scope {
        Scope::Namespaced => {
            if ns.is_empty() {
                return Err(format!("get {kind}/{name}: namespace required"));
            }
            Api::<DynamicObject>::namespaced_with(client.clone(), ns, &ar)
        }
        Scope::Cluster => Api::<DynamicObject>::all_with(client.clone(), &ar),
    };
    let object = api
        .get(name)
        .await
        .map_err(|err| format!("get {kind}/{name}: {err}"))?;
    serde_json::to_value(&object).map_err(|err| format!("serialize {kind}/{name}: {err}"))
}

/// Container logs via the typed pod API — `Log` is an implicit-resource
/// marker trait implemented for `Pod` only (dynamic objects can't carry it).
pub async fn pod_logs(
    client: &Client,
    name: &str,
    ns: &str,
    container: Option<String>,
) -> Result<Value, String> {
    if ns.is_empty() {
        return Err(format!("logs {name}: namespace required"));
    }
    let params = LogParams {
        container,
        ..LogParams::default()
    };
    let text = Api::<Pod>::namespaced(client.clone(), ns)
        .logs(name, &params)
        .await
        .map_err(|err| format!("logs {name}: {err}"))?;
    Ok(Value::String(text))
}
