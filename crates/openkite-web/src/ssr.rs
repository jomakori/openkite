//! SSR + hydration rendering for the shared console.
//!
//! One axum process serves the SSR markup (so the browser gets a fully
//! rendered page before any JavaScript runs) and a same-origin gateway route
//! the client calls when it needs fresh cluster data. The browser holds no
//! credentials: the kube client lives in this process, the gateway implementation
//! runs server-side, and the wasm client posts its gateway calls back to
//! `/api/gateway` over the same origin.
//!
//! [`Snapshot`], [`SecretRef`] and the `App` props they feed are built by both
//! targets, so they compile for `wasm32` too. The document itself is native
//! only: `render_body` and `render_page` are behind the `ssr` feature, which is
//! what pulls the `dioxus-ssr` stack and the CBOR/base64 hydration payload.

use std::sync::Arc;

use openkite_api::gateway::Gateway;
use openkite_ui::components::resource_pane::ResourceRef;
use openkite_ui::components::resource_table::{ColumnDef, ResourceRow};

#[cfg(all(feature = "ssr", not(target_arch = "wasm32")))]
use base64::Engine as _;
#[cfg(all(feature = "ssr", not(target_arch = "wasm32")))]
use dioxus::core::VirtualDom;

#[cfg(all(feature = "ssr", not(target_arch = "wasm32")))]
use crate::app::{App, AppProps};

/// A snapshot of the cluster the SSR pass and the wasm client both start from.
///
/// Embedded in the SSR page as JSON (`<script id="openkite-snapshot">`) so the
/// client rehydrates against the same tree the server rendered, and so a curl
/// of `/` carries the cluster context the host reached without the host
/// trusting the browser with anything it does not already show.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    /// Gateway capabilities the host advertises (cluster kind, plugins, etc.).
    pub capabilities: openkite_api::capability::Capabilities,
    /// Connection state the user is looking at.
    pub connected: bool,
    /// Cluster identity label (in-cluster, kubeconfig context, etc.).
    pub context: Option<String>,
    /// Secret names available in the gateway's `secret()` op, so the SSR
    /// surface shows them and the client doesn't have to round-trip just to
    /// paint.
    pub secrets: Vec<SecretRef>,
    /// The resource detail pane's selection, restored from the query string of
    /// the address this host served (`/workloads?kind=Pod&ns=default&name=…`).
    /// It rides the snapshot so the SSR pass and the hydrating client both
    /// paint the pane open — a reload or a shared link reopens the same pane.
    #[serde(default)]
    pub selection: Option<ResourceRef>,
    /// The route the host resolved for this document.
    #[serde(default = "home_route")]
    pub route: String,
    /// The table the `/workloads` route paints, or why the host could not list.
    #[serde(default)]
    pub workloads: WorkloadsTable,
}

/// The table a route paints, in the console's own column/row vocabulary.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct WorkloadsTable {
    pub columns: Vec<ColumnDef>,
    pub rows: Vec<ResourceRow>,
    /// The gateway's refusal, when it had one.
    #[serde(default)]
    pub error: Option<String>,
}

/// The route a snapshot with no address of its own lands on.
fn home_route() -> String {
    "/".to_string()
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            capabilities: openkite_api::capability::Capabilities::server_side(),
            connected: false,
            context: None,
            secrets: Vec::new(),
            selection: None,
            route: home_route(),
            workloads: WorkloadsTable::default(),
        }
    }
}

impl Snapshot {
    /// Build a snapshot from a connected gateway.
    pub async fn from_gateway(
        gateway: &Arc<dyn Gateway>,
        connected: bool,
        context: Option<String>,
    ) -> Self {
        let capabilities = gateway.capabilities();
        let secrets = list_secret_refs(gateway).await.unwrap_or_default();
        Self {
            capabilities,
            connected,
            context,
            secrets,
            selection: None,
            route: home_route(),
            workloads: WorkloadsTable::default(),
        }
    }

    /// The same snapshot, with the detail pane's selection taken from the
    /// address the request carried.
    pub fn with_selection(mut self, selection: Option<ResourceRef>) -> Self {
        self.selection = selection;
        self
    }

    /// The same snapshot, addressed at the route the request resolved to.
    pub fn with_route(mut self, route: impl Into<String>) -> Self {
        self.route = route.into();
        self
    }

    /// The same snapshot, carrying the table the workloads route paints.
    pub fn with_workloads(mut self, workloads: WorkloadsTable) -> Self {
        self.workloads = workloads;
        self
    }
}

/// A secret reference safe to ship to the browser: name + namespace, never
/// values. The slide-over calls `gateway.secret(ns, name)` on demand.
pub use openkite_api::secret::SecretRef;

/// The distinct namespaces the refs live in, sorted — the namespace bar's rows.
pub fn namespaces_of(secrets: &[SecretRef]) -> Vec<String> {
    secrets
        .iter()
        .map(|secret| secret.namespace.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Render the SSR body for the snapshot.
///
/// Uses `dioxus_ssr::pre_render` so the resulting HTML carries the
/// `data-node-hydration` ids the browser's hydrate step matches against.
#[cfg(all(feature = "ssr", not(target_arch = "wasm32")))]
pub fn render_body(snapshot: &Snapshot) -> String {
    let mut dom = VirtualDom::new_with_props(
        App,
        AppProps {
            snapshot: snapshot.clone(),
        },
    );
    dom.rebuild(&mut dioxus::core::NoOpMutations);
    dioxus_ssr::pre_render(&dom)
}

/// Base64(CBOR(Vec<Option<Vec<u8>>>)) — what dioxus-web expects in
/// `window.initial_dioxus_hydration_data`. Empty payload is fine: there are
/// no server functions and no streaming data; the client hydrates against the
/// embedded snapshot directly.
#[cfg(all(feature = "ssr", not(target_arch = "wasm32")))]
pub fn hydration_data() -> String {
    let empty: Vec<Option<Vec<u8>>> = Vec::new();
    let mut bytes = Vec::new();
    ciborium::into_writer(&empty, &mut bytes).expect("cbor encode hydration data");
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Options that change what the page contains without changing the snapshot.
#[cfg(all(feature = "ssr", not(target_arch = "wasm32")))]
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Emit the hydration payload + client script (true) or SSR-only fallback (false).
    pub hydrate: bool,
    /// Path the client bundle is served from, used as the import target.
    pub client_script: String,
}

#[cfg(all(feature = "ssr", not(target_arch = "wasm32")))]
impl RenderOptions {
    pub fn hydrating() -> Self {
        Self {
            hydrate: true,
            client_script: "/openkite-web-client.js".to_string(),
        }
    }

    pub fn ssr_only() -> Self {
        Self {
            hydrate: false,
            ..Self::hydrating()
        }
    }
}

/// The full HTML page: SSR body + hydration scripts + the snapshot the client
/// reads on hydrate.
#[cfg(all(feature = "ssr", not(target_arch = "wasm32")))]
pub fn render_page(snapshot: &Snapshot, options: &RenderOptions) -> String {
    let body = render_body(snapshot);
    let json = serde_json::to_string(snapshot).expect("serialize snapshot");
    let hydration = hydration_data();

    let hydration_scripts = if options.hydrate {
        format!(
            r#"<script>window.hydrate_queue = [];</script>
<script>window.initial_dioxus_hydration_data="{hydration}";</script>
<script id="openkite-snapshot" type="application/json">{json}</script>
<script type="module">
  import init from "{client}";
  init();
</script>"#,
            client = options.client_script,
        )
    } else {
        format!(
            r#"<script>window.hydrate_queue = [];</script>
<script id="openkite-snapshot" type="application/json">{json}</script>"#
        )
    };

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>OpenKite</title>
<style>{css}</style>
<script>{bridge}</script>
</head>
<body>
<div id="main">{body}</div>
{hydration_scripts}
</body>
</html>"#,
        css = openkite_ui::MAIN_CSS,
        bridge = openkite_ui::plugin_api::OPENKITE_BRIDGE_JS,
    )
}

/// The secret refs the gateway can hand out, or `Err` when it cannot list.
async fn list_secret_refs(gateway: &Arc<dyn Gateway>) -> Result<Vec<SecretRef>, ()> {
    gateway.secret_refs().await.map_err(|_| ())
}
