//! OKT-67 spike: React 19 + Tailwind 4 mounted in the existing wry webview.
//!
//! Additive host half of the spike. It reuses the sanctioned desktop
//! custom-protocol transport (a dioxus asset handler hit by same-origin
//! `fetch`) and leaves the plugin bridge (`/openkite`), kube reflect/watch,
//! and the plugin host untouched:
//!
//! - the React app reads its kube snapshot through the EXISTING
//!   `window.openkite.api.list` bridge;
//! - `/openkite-spike` is a read-only spike endpoint that answers the active
//!   kubeconfig context, because the existing op set has no equivalent.
//!
//! The bundle is vendored under `assets/vendored/openkite-react-spike/` (the
//! same convention as `tools/build-xterm`) so `include_str!` sees a committed
//! file at compile time.

use dioxus::desktop::wry;
use dioxus::desktop::{use_asset_handler, AssetRequest, RequestAsyncResponder};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::plugin_api::ApiResponse;
use crate::router::json_response;

const SPIKE_JS: &str = include_str!("../assets/vendored/openkite-react-spike/app.js");
const SPIKE_CSS: &str = include_str!("../assets/vendored/openkite-react-spike/app.css");

/// The spike-only request op; responses reuse the bridge's [`ApiResponse`].
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum SpikeRequest {
    Context,
}

#[derive(Debug, Serialize)]
struct SpikeContext {
    context: Option<String>,
    connected: bool,
}

/// Mount the React spike: render its container, then evaluate the vendored
/// bundle once the container is in the DOM (the bundle self-mounts).
#[component]
pub fn ReactSpike() -> Element {
    use_asset_handler(
        "openkite-spike",
        |req: AssetRequest, responder: RequestAsyncResponder| {
            dispatch(req, responder);
        },
    );

    use_effect(move || {
        document::eval(SPIKE_JS);
    });

    rsx! {
        style { dangerous_inner_html: SPIKE_CSS }
        div { id: "openkite-react-spike-root", class: "openkite-react-spike" }
    }
}

/// Answer one `/openkite-spike` POST. Never panics; errors are answered.
fn dispatch(req: AssetRequest, responder: RequestAsyncResponder) {
    if req.method() != wry::http::Method::POST {
        responder.respond(json_response(ApiResponse::Error {
            error: "method not allowed: spike requests must be POST".into(),
        }));
        return;
    }
    let Ok(text) = std::str::from_utf8(req.body()) else {
        responder.respond(json_response(ApiResponse::Error {
            error: "request body is not utf-8".into(),
        }));
        return;
    };
    let outcome = match serde_json::from_str::<SpikeRequest>(text) {
        Ok(SpikeRequest::Context) => serde_json::to_value(SpikeContext {
            context: crate::runtime::context_name(),
            connected: crate::runtime::client().is_some(),
        })
        .map_err(|err| format!("serialize context: {err}")),
        Err(err) => Err(format!("parse spike request: {err}")),
    };
    let response = match outcome {
        Ok(result) => ApiResponse::Ok { result },
        Err(error) => ApiResponse::Error { error },
    };
    responder.respond(json_response(response));
}
