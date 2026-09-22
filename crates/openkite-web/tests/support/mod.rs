//! Shared helpers for the host's integration tests.
//!
//! Two stand-ins for a cluster: a client pointed at a closed port, where every
//! kube call fails without touching the network, and a fake API server that
//! answers discovery plus one pod list, so the host can be driven end to end
//! with a real kube client.

#![allow(dead_code)]

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use kube::Client;
use openkite::bridge::Bridge;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;

/// The pod list a fake API server answers with.
pub const POD_NAME: &str = "probe-pod";
pub const POD_NAMESPACE: &str = "default";

/// A client pointed at a closed port, so every kube call fails without a
/// cluster to talk to. The rustls provider must be installed before the client
/// is built.
pub fn unreachable_client() -> Client {
    client_for("http://127.0.0.1:1")
}

/// A kube client for a base URL with no TLS.
pub fn client_for(base: &str) -> Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let uri: http::Uri = base.parse().expect("parse base url");
    Client::try_from(kube::Config::new(uri)).expect("build client")
}

/// The host router, as the binary mounts it.
pub fn app(bridge: Arc<Bridge>, web_root: &Path) -> Router {
    openkite_web::routes::router(bridge, web_root)
}

/// The console's `list pods` call, in the bridge envelope shape.
pub fn list_pods_envelope() -> Value {
    json!({
        "id": 1,
        "plugin": "console",
        "request": {"op": "list", "kind": "pods", "ns": null},
    })
}

/// POST a JSON body to the router and read the envelope back.
pub async fn post_json(app: Router, path: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("build request");
    let response = app.oneshot(request).await.expect("router call");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let value = serde_json::from_slice(&bytes).expect("json envelope");
    (status, value)
}

/// GET a path from the router and read the body as text.
pub async fn get_body(app: Router, path: &str) -> (StatusCode, String) {
    let request = Request::builder()
        .method("GET")
        .uri(path)
        .body(Body::empty())
        .expect("build request");
    let response = app.oneshot(request).await.expect("router call");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// A kube client over a fake API server that answers discovery and one pod list,
/// so `list pods` resolves to a real kube `List` with no cluster.
pub async fn fake_api_client() -> Client {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake api");
    let addr = listener.local_addr().expect("fake api address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, fake_api_router()).await;
    });
    client_for(&format!("http://{addr}"))
}

/// The fake API's routes: the two discovery calls `resolve_resource` makes, the
/// core resource list, and the pod list itself.
fn fake_api_router() -> Router {
    Router::new()
        .route("/api", get(core_versions))
        .route("/api/v1", get(core_resources))
        .route("/apis", get(api_groups))
        .route("/api/v1/pods", get(pod_list))
        .route("/api/v1/namespaces/{ns}/pods", get(pod_list))
        .fallback(|| async { (StatusCode::NOT_FOUND, "not found") })
}

async fn core_versions() -> Json<Value> {
    Json(json!({
        "kind": "APIVersions",
        "versions": ["v1"],
        "serverAddressByClientCIDRs": [],
    }))
}

async fn api_groups() -> Json<Value> {
    Json(json!({"kind": "APIGroupList", "apiVersion": "v1", "groups": []}))
}

async fn core_resources() -> Json<Value> {
    Json(json!({
        "kind": "APIResourceList",
        "apiVersion": "v1",
        "groupVersion": "v1",
        "resources": [
            {
                "name": "pods",
                "singularName": "pod",
                "namespaced": true,
                "kind": "Pod",
                "verbs": ["get", "list", "watch"],
            },
        ],
    }))
}

async fn pod_list() -> Json<Value> {
    Json(json!({
        "kind": "PodList",
        "apiVersion": "v1",
        "metadata": {"resourceVersion": "1"},
        "items": [
            {
                "kind": "Pod",
                "apiVersion": "v1",
                "metadata": {
                    "name": POD_NAME,
                    "namespace": POD_NAMESPACE,
                    "resourceVersion": "1",
                },
                "spec": {"containers": [{"name": "app", "image": "nginx"}]},
                "status": {"phase": "Running"},
            },
        ],
    }))
}

/// One raw HTTP/1.1 request against a server bound on `addr`, answered as text.
///
/// A raw socket keeps the tests free of an HTTP client dependency, and the
/// assertions only need substrings of the response.
pub async fn raw_http(addr: SocketAddr, method: &str, path: &str, body: Option<&str>) -> String {
    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connect to host");
    let body = body.unwrap_or("");
    let head = format!(
        "{method} {path} HTTP/1.1\r\nhost: {addr}\r\ncontent-type: application/json\r\n\
         content-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await.expect("write head");
    stream.write_all(body.as_bytes()).await.expect("write body");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .expect("read response");
    response
}

/// Wait until a server bound on `addr` answers, up to ~5s.
pub async fn wait_until_serving(addr: SocketAddr) {
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            let response = raw_http(addr, "GET", "/", None).await;
            if response.starts_with("HTTP/1.1") {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("host on {addr} never answered");
}
