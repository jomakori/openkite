//! The HTTP surface the console calls.
//!
//! Each route mirrors a transport the desktop already has: `POST /openkite` is
//! the bridge envelope (there it is the wry asset handler's job), `POST
//! /openkite-spike` carries the console's context and settings ops, and every
//! other path is the bundle itself with an SPA fallback.

use std::path::Path;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use openkite::bridge::Bridge;
use openkite::plugin_api::ApiResponse;
use tower_http::services::{ServeDir, ServeFile};

use crate::spike;

/// The shared bridge every bridge request dispatches through.
pub type SharedBridge = Arc<Bridge>;

/// Build the host router: the console's two endpoints, then the bundle.
pub fn router(bridge: SharedBridge, web_root: &Path) -> Router {
    // The console routes client-side (`/cluster`, `/workloads`), so an unknown
    // path has to answer the shell — the same fallback the preview image's nginx
    // config provides. Without it a deep link 404s and the console never mounts.
    let assets = ServeDir::new(web_root).fallback(ServeFile::new(web_root.join("index.html")));
    Router::new()
        .route("/openkite", post(bridge_post))
        .route("/openkite-spike", post(spike_post))
        .fallback_service(assets)
        .with_state(bridge)
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
