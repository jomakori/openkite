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
    version: String,
}

/// Mount the React console: register its context endpoint, render the
/// container, then evaluate the vendored bundle once (the bundle self-mounts).
///
/// `route` is the console nav id that matches the host route; changes are
/// pushed into the already-running bundle so host-side navigation (e.g. the
/// command palette) re-points the console without a remount. The bundle also
/// reads the `data-console-route` attribute on first mount.
#[component]
pub fn ReactConsole(route: String) -> Element {
    use_asset_handler(
        "openkite-spike",
        |req: AssetRequest, responder: RequestAsyncResponder| {
            dispatch(req, responder);
        },
    );

    use_effect(move || {
        document::eval(SPIKE_JS);
    });

    use_effect(use_reactive((&route,), |(route,)| {
        let source = format!(
            "window.__openkite_react_console && \
             window.__openkite_react_console.setRoute({route:?});"
        );
        document::eval(&source);
    }));

    rsx! {
        style { dangerous_inner_html: SPIKE_CSS }
        div {
            id: "openkite-react-spike-root",
            class: "openkite-react-spike",
            "data-console-route": "{route}",
        }
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
            version: env!("CARGO_PKG_VERSION").to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_payload_carries_cluster_connection_and_version() {
        let value = serde_json::to_value(SpikeContext {
            context: Some("prod-us-east-1".into()),
            connected: true,
            version: "0.1.0".into(),
        })
        .unwrap();
        assert_eq!(value["context"], "prod-us-east-1");
        assert_eq!(value["connected"].as_bool(), Some(true));
        assert_eq!(value["version"], "0.1.0");
    }

    #[test]
    fn context_payload_serializes_a_disconnected_shell() {
        let value = serde_json::to_value(SpikeContext {
            context: None,
            connected: false,
            version: "0.1.0".into(),
        })
        .unwrap();
        assert!(value["context"].is_null());
        assert_eq!(value["connected"].as_bool(), Some(false));
    }
}
