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

/// Read a pod's log the way the panel paints it: the whole tail, as lines.
///
/// The browser host has no push transport, so the panel re-reads this on its
/// poll interval and keeps only the lines the stream added
/// (`crate::logs::LogFeed`). `Ok(None)` on native targets, where the SSR pass
/// already put the same lines in the snapshot.
#[cfg(not(target_arch = "wasm32"))]
pub async fn fetch_logs(
    _pod: &openkite_api::pod::PodObject,
    _container: Option<&str>,
) -> Result<Option<Vec<String>>, String> {
    Ok(None)
}

/// Read a pod's log over a same-origin GET against `/api/logs`.
#[cfg(target_arch = "wasm32")]
pub async fn fetch_logs(
    pod: &openkite_api::pod::PodObject,
    container: Option<&str>,
) -> Result<Option<Vec<String>>, String> {
    use wasm_bindgen::JsCast as _;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, RequestMode, Response};

    let window = match web_sys::window() {
        Some(window) => window,
        None => return Ok(None),
    };
    let mut url = format!(
        "/api/logs?pod={}",
        js_sys::encode_uri_component(&pod.name)
    );
    if let Some(namespace) = pod.namespace.as_deref() {
        url.push_str(&format!(
            "&ns={}",
            js_sys::encode_uri_component(namespace)
        ));
    }
    if let Some(container) = container {
        url.push_str(&format!(
            "&container={}",
            js_sys::encode_uri_component(container)
        ));
    }
    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::SameOrigin);
    let request = match Request::new_with_str_and_init(&url, &opts) {
        Ok(request) => request,
        Err(err) => return Err(format!("build request: {err:?}")),
    };
    let response_value = match JsFuture::from(window.fetch_with_request(&request)).await {
        Ok(value) => value,
        Err(err) => return Err(format!("fetch: {err:?}")),
    };
    let response: Response = match response_value.dyn_into() {
        Ok(response) => response,
        Err(err) => return Err(format!("response cast: {err:?}")),
    };
    if !response.ok() {
        return Err(format!("logs http {}", response.status()));
    }
    let text_promise = match response.text() {
        Ok(text) => text,
        Err(err) => return Err(format!("text: {err:?}")),
    };
    let text_value = match JsFuture::from(text_promise).await {
        Ok(value) => value,
        Err(err) => return Err(format!("text await: {err:?}")),
    };
    let Some(text) = text_value.as_string() else {
        return Ok(None);
    };
    let pane: crate::ssr::LogsPane = match serde_json::from_str(&text) {
        Ok(pane) => pane,
        Err(err) => return Err(format!("parse logs: {err}")),
    };
    Ok(Some(pane.lines))
}
