//! HTTP surface the console calls.
//!
//! Each route mirrors a transport the desktop already has: `POST /openkite` is
//! the bridge envelope (there it is the wry asset handler's job), `POST
//! /openkite-spike` carries the console's context and settings ops, and every
//! other path serves the hydration bundle, falling back to the same document
//! `GET /` renders — with the path resolved to the console's route contract, so
//! `GET /workloads` paints the workloads route. `POST /api/gateway` is the
//! same-origin round-trip the hydrating client refreshes through, and `GET
//! /assets/fonts/{file}` is where the stylesheet's `@font-face` URLs land.

use std::path::Path;
use std::sync::Arc;

use axum::extract::{OriginalUri, Path as UrlPath, RawQuery, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use k8s_openapi::api::core::v1::Pod;
use openkite_api::bridge::{ApiRequest, ApiResponse};
use openkite_host::bridge::Bridge;
use openkite_ui::components::route_views::resolve_route;
use tower_http::services::ServeDir;

use crate::spike;
use crate::ssr;

/// The shared bridge every bridge request dispatches through.
pub type SharedBridge = Arc<Bridge>;

/// Build the host router: the crate-rendered root, the console's endpoints,
/// the vendored typefaces, then the hydration bundle.
///
/// The runtime image ships no static document, so a path the bundle has no file
/// for is answered by the console itself rather than a file that is not there.
pub fn router(bridge: SharedBridge, web_root: &Path) -> Router {
    let shell: Router = Router::new()
        .route("/{*path}", get(ssr_root))
        .with_state(bridge.clone());
    let assets = ServeDir::new(web_root).fallback(shell);
    Router::new()
        .route("/", get(ssr_root))
        .route("/assets/fonts/{file}", get(font_get))
        .route("/openkite", post(bridge_post))
        .route("/openkite-spike", post(spike_post))
        .route("/api/gateway", post(gateway_post))
        .fallback_service(assets)
        .with_state(bridge)
}

/// Serve one vendored typeface the shell stylesheet declares.
///
/// The bytes are immutable for the life of the binary, so the client is told it
/// never has to ask twice: a browser fetches each face once per install.
async fn font_get(UrlPath(file): UrlPath<String>) -> Response {
    match openkite_ui::assets::face_by_file(&file) {
        Some(face) => (
            [
                (CONTENT_TYPE, "font/woff2"),
                (CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            face.bytes,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "unknown typeface").into_response(),
    }
}

/// Dispatch one bridge POST through the shared [`Bridge`].
///
/// Always answers HTTP 200 with an `ok`/`error` envelope: the JS side reads the
/// envelope, and any non-2xx is treated as "no host here", which would drop the
/// console back to fixtures.
async fn bridge_post(State(bridge): State<SharedBridge>, body: String) -> Json<ApiResponse> {
    Json(bridge.handle_post(&body).await)
}

/// Answer one console context/settings POST, in the same envelope.
async fn spike_post(State(bridge): State<SharedBridge>, body: String) -> Json<ApiResponse> {
    Json(spike::handle(&body, bridge.client().is_some()))
}

/// Render the console root server-side from the snapshot the host holds.
///
/// The page is SSR + hydrate: the snapshot and the markup are in the document
/// so a deep link paints before any JavaScript runs, and the client bundle in
/// `OPENKITE_WEB_ROOT` then attaches to the same tree, which is what makes a
/// route reached by URL interactive in the browser host.
///
/// The address is part of the render: the route the path resolved to and the
/// resource detail pane's selection both ride the snapshot, so the server
/// paints the page a reload or a shared link asked for rather than waiting for
/// a client to restore it.
async fn ssr_root(
    State(bridge): State<SharedBridge>,
    OriginalUri(uri): OriginalUri,
    RawQuery(query): RawQuery,
) -> Response {
    let route = resolve_route(uri.path(), &sidebar_sections());
    let selection = query
        .as_deref()
        .and_then(openkite_ui::components::resource_pane::ResourceRef::from_query);
    let snapshot = ssr_snapshot(&bridge, &route)
        .await
        .with_selection(selection);
    let page = ssr::render_page(&snapshot, &ssr::RenderOptions::hydrating());
    ([(CONTENT_TYPE, "text/html; charset=utf-8")], page).into_response()
}

/// Answer the hydrating client's refresh: a fresh snapshot in the envelope
/// `GatewayResponse` defines, carrying the table the workloads route paints.
async fn gateway_post(State(bridge): State<SharedBridge>, body: String) -> Response {
    let request: Result<crate::app::GatewayRequest, _> = serde_json::from_str(&body);
    let snapshot = match request {
        Ok(_) => ssr_snapshot(&bridge, "/")
            .await
            .with_workloads(workload_table(&bridge).await),
        Err(_) => ssr::Snapshot::default(),
    };
    let response = crate::app::GatewayResponse { snapshot };
    Json(response).into_response()
}

/// Build the snapshot the SSR pass renders against: the host's capabilities,
/// identity and route, plus the workloads the route paints.
async fn ssr_snapshot(bridge: &Bridge, route: &str) -> ssr::Snapshot {
    let client = bridge.client();
    let connected = client.is_some();
    let context = if crate::in_cluster() {
        Some("in-cluster".to_string())
    } else {
        kube::Config::infer()
            .await
            .ok()
            .map(|_| "kubeconfig".to_string())
    };
    let snapshot = if let Some(client) = client {
        let gateway: Arc<dyn openkite_api::gateway::Gateway> =
            Arc::new(openkite_host::gateway::KubeGateway::server_side(client));
        ssr::Snapshot::from_gateway(&gateway, connected, context).await
    } else {
        ssr::Snapshot::default()
    };
    let snapshot = snapshot.with_route(route);
    if route == "/workloads" {
        snapshot.with_workloads(workload_table(bridge).await)
    } else {
        snapshot
    }
}

/// The sidebar model the shell renders, so a path resolves against it.
fn sidebar_sections() -> Vec<openkite_ui::shell::ShellSection> {
    openkite_ui::shell::sidebar_model(&openkite_ui::plugin_api::RegistrationStore::default())
}

/// The `/workloads` table: the cluster's pods, or the gateway's refusal.
async fn workload_table(bridge: &Bridge) -> ssr::WorkloadsTable {
    let columns = openkite::workloads::pod_columns();
    let request = ApiRequest::List {
        kind: "pods".to_string(),
        ns: None,
    };
    match bridge.execute("console", request).await {
        ApiResponse::Ok { result } => match pods(&result) {
            Ok(pods) => ssr::WorkloadsTable {
                columns,
                rows: pods.iter().map(openkite::workloads::pod_row).collect(),
                error: None,
            },
            Err(error) => ssr::WorkloadsTable {
                columns,
                rows: Vec::new(),
                error: Some(error),
            },
        },
        ApiResponse::Error { error } => ssr::WorkloadsTable {
            columns,
            rows: Vec::new(),
            error: Some(error),
        },
    }
}

/// The pods a `list pods` answer carries.
fn pods(result: &serde_json::Value) -> Result<Vec<Pod>, String> {
    let items = result
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "the pod list carried no items".to_string())?;
    items
        .iter()
        .map(|item| serde_json::from_value::<Pod>(item.clone()))
        .collect::<Result<Vec<Pod>, _>>()
        .map_err(|err| format!("a pod did not parse: {err}"))
}
