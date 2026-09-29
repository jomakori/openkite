//! HTTP surface the console calls.
//!
//! Each route mirrors a transport the desktop already has: `POST /openkite` is
//! the bridge envelope (there it is the wry asset handler's job), `POST
//! /openkite-spike` carries the console's context and settings ops, and every
//! other path is the bundle itself with an SPA fallback. `GET /` is the one
//! route the crate renders itself, `POST /api/gateway` is the same-origin
//! round-trip the hydrating client refreshes through, and
//! `GET /assets/fonts/{file}` is where the stylesheet's `@font-face` URLs land.

use std::path::Path;
use std::sync::Arc;

use axum::extract::{Path as UrlPath, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use openkite_api::bridge::ApiResponse;
use openkite_host::bridge::Bridge;
use tower_http::services::{ServeDir, ServeFile};

use crate::spike;
use crate::ssr;

/// The shared bridge every bridge request dispatches through.
pub type SharedBridge = Arc<Bridge>;

/// Build the host router: the crate-rendered root, the console's endpoints,
/// the vendored typefaces, then the bundle.
pub fn router(bridge: SharedBridge, web_root: &Path) -> Router {
    let assets = ServeDir::new(web_root).fallback(ServeFile::new(web_root.join("index.html")));
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
/// The page is SSR-only: it carries the snapshot and the markup, and the
/// hydrating client is what attaches to it once the image serves the crate.
async fn ssr_root(State(bridge): State<SharedBridge>) -> Response {
    let snapshot = ssr_snapshot(&bridge).await;
    let page = ssr::render_page(&snapshot, &ssr::RenderOptions::ssr_only());
    ([(CONTENT_TYPE, "text/html; charset=utf-8")], page).into_response()
}

/// Answer the hydrating client's refresh: a fresh snapshot in the envelope
/// `GatewayResponse` defines.
async fn gateway_post(State(bridge): State<SharedBridge>, body: String) -> Response {
    let request: Result<crate::app::GatewayRequest, _> = serde_json::from_str(&body);
    let snapshot = match request {
        Ok(_) => ssr_snapshot(&bridge).await,
        Err(_) => ssr::Snapshot::default(),
    };
    let response = crate::app::GatewayResponse { snapshot };
    Json(response).into_response()
}

/// Build the snapshot the SSR pass renders against: the host's capabilities
/// and identity when a cluster is connected, the disconnected default when not.
async fn ssr_snapshot(bridge: &Bridge) -> ssr::Snapshot {
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
    if let Some(client) = client {
        let gateway: Arc<dyn openkite_api::gateway::Gateway> =
            Arc::new(openkite_host::gateway::KubeGateway::server_side(client));
        ssr::Snapshot::from_gateway(&gateway, connected, context).await
    } else {
        ssr::Snapshot::default()
    }
}
