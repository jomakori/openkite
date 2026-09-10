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

/// OKT-93 measurement script.
///
/// The spike bundle publishes its in-page timings on
/// `window.__openkite_react_spike` (the same controller `measure/measure.mjs`
/// reads under Playwright). This runs INSIDE the real WebKitGTK webview, so the
/// numbers describe the engine wry actually uses rather than Chromium.
///
/// It waits for mount + first bridge load, times one synthetic 500-row commit,
/// then reports back over `/openkite-spike` (which logs it) and finally reloads
/// so the visible table returns to real cluster data.
///
/// Runs as one `document::eval` from the component's effect — i.e. on the UI
/// thread, which is the only place evals are valid (see dioxus-07-rsx-gotchas
/// §13); `eval` from a tokio worker silently no-ops.
const MEASURE_JS: &str = r#"
(async () => {
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const c = window.__openkite_react_spike;
  if (!c) { return 'no-controller'; }
  for (let i = 0; i < 150 && !c.ready; i++) await sleep(200);
  for (let i = 0; i < 150 && !c.loaded; i++) await sleep(200);
  let render500 = null;
  try {
    const rows = Array.from({ length: 500 }, (_, i) => ({
      name: 'bench-pod-' + i,
      namespace: 'bench',
      phase: 'Running',
      ready: '1/1',
      age: i + 'd',
    }));
    render500 = await c.renderRows(rows);
  } catch (e) { render500 = -1; }
  const payload = {
    op: 'report',
    first_paint_ms: c.firstPaintMs,
    loaded_ms: c.loadedMs,
    context_ms: c.contextMs,
    list_ms: c.listMs,
    render_ms: c.renderMs,
    render_500_ms: render500,
    render_count: c.renderCount,
    rows: c.rowCount,
    bridge_mode: c.bridgeMode,
  };
  try {
    await fetch('/openkite-spike', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(payload),
    });
  } catch (e) { /* reported via log absence */ }
  // Put the real rows back so the screenshot shows real cluster data.
  try { await c.reload(); } catch (e) { /* ignore */ }
  return 'measured';
})();
"#;

/// The spike-only request op; responses reuse the bridge's [`ApiResponse`].
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum SpikeRequest {
    Context,
    Report(MeasureReport),
}

#[derive(Debug, Serialize)]
struct SpikeContext {
    context: Option<String>,
    connected: bool,
}

/// In-page timings reported by [`MEASURE_JS`] (OKT-93).
#[derive(Debug, Deserialize)]
struct MeasureReport {
    first_paint_ms: Option<f64>,
    loaded_ms: Option<f64>,
    context_ms: Option<f64>,
    list_ms: Option<f64>,
    render_ms: Option<f64>,
    render_500_ms: Option<f64>,
    render_count: Option<u32>,
    rows: Option<usize>,
    bridge_mode: Option<String>,
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
        // OKT-93: measure the real engine once the bundle has mounted.
        document::eval(MEASURE_JS);
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
        // OKT-93: a single greppable line so the harness can lift the numbers
        // straight out of the pod log.
        Ok(SpikeRequest::Report(r)) => {
            tracing::info!(
                target: "openkite::spike_measure",
                first_paint_ms = ?r.first_paint_ms,
                loaded_ms = ?r.loaded_ms,
                context_ms = ?r.context_ms,
                list_ms = ?r.list_ms,
                render_ms = ?r.render_ms,
                render_500_ms = ?r.render_500_ms,
                render_count = ?r.render_count,
                rows = ?r.rows,
                bridge_mode = ?r.bridge_mode,
                "SPIKE_MEASURE"
            );
            Ok(serde_json::json!({ "logged": true }))
        }
        Err(err) => Err(format!("parse spike request: {err}")),
    };
    let response = match outcome {
        Ok(result) => ApiResponse::Ok { result },
        Err(error) => ApiResponse::Error { error },
    };
    responder.respond(json_response(response));
}
