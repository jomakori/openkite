//! The client-side gateway transport.
//!
//! Native targets (SSR, tests) cannot reach a browser fetch — they were handed
//! the snapshot by the host. The wasm client posts a same-origin request to
//! `/api/gateway`; the server answers with the fresh snapshot.

use crate::ssr::Snapshot;

/// Fetch a fresh snapshot from the server-side gateway. Returns `Ok(None)` on
/// native targets (nothing to fetch: the host already owns the data and gave
/// us the snapshot at SSR time) and on transport failures.
#[cfg(not(target_arch = "wasm32"))]
pub async fn fetch_snapshot() -> Result<Option<Snapshot>, String> {
    Ok(None)
}

/// Fetch a fresh snapshot from the server-side gateway over a same-origin POST.
#[cfg(target_arch = "wasm32")]
pub async fn fetch_snapshot() -> Result<Option<Snapshot>, String> {
    use wasm_bindgen::JsCast as _;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, RequestMode, Response};

    let window = match web_sys::window() {
        Some(window) => window,
        None => return Ok(None),
    };
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::SameOrigin);
    let body = match serde_json::to_string(&crate::app::GatewayRequest::Snapshot) {
        Ok(body) => body,
        Err(err) => return Err(format!("serialize: {err}")),
    };
    opts.set_body(&wasm_bindgen::JsValue::from_str(&body));
    let request = match Request::new_with_str_and_init("/api/gateway", &opts) {
        Ok(request) => request,
        Err(err) => return Err(format!("build request: {err:?}")),
    };
    if let Err(err) = request.headers().set("content-type", "application/json") {
        return Err(format!("set header: {err:?}"));
    }
    let response_value = match JsFuture::from(window.fetch_with_request(&request)).await {
        Ok(value) => value,
        Err(err) => return Err(format!("fetch: {err:?}")),
    };
    let response: Response = match response_value.dyn_into() {
        Ok(response) => response,
        Err(err) => return Err(format!("response cast: {err:?}")),
    };
    if !response.ok() {
        return Err(format!("gateway http {}", response.status()));
    }
    let text_promise = match response.text() {
        Ok(text) => text,
        Err(err) => return Err(format!("text: {err:?}")),
    };
    let text_value = match JsFuture::from(text_promise).await {
        Ok(value) => value,
        Err(err) => return Err(format!("text await: {err:?}")),
    };
    let text = match text_value.as_string() {
        Some(text) => text,
        None => return Ok(None),
    };
    let resp: crate::app::GatewayResponse = match serde_json::from_str(&text) {
        Ok(resp) => resp,
        Err(err) => return Err(format!("parse response: {err}")),
    };
    Ok(Some(resp.snapshot))
}
