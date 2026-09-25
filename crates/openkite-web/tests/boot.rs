//! Smoke test: boot the host the way the binary boots it, against a fake API
//! server, and drive it over HTTP.
//!
//! `list pods` must answer a real kube `List` and `watch pods` must answer the
//! reflector snapshot — those two are exactly what the console's fixture
//! fallback rides on, so asserting them here is the browser build's acceptance.

mod support;

use std::time::Duration;

use openkite_web::serve;
use serde_json::json;
use support::{fake_api_client, raw_http, wait_until_serving, POD_NAME, POD_NAMESPACE};

const SHELL: &str = "<!doctype html><title>OpenKite console</title>";

/// The console's `list pods` call, as `web/src/bridge.ts` builds it.
fn list_pods_body() -> String {
    json!({
        "id": 1,
        "plugin": "console",
        "request": {"op": "list", "kind": "pods", "ns": null},
    })
    .to_string()
}

#[tokio::test]
async fn booted_host_answers_the_console_over_http() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("index.html"), SHELL).expect("write shell");

    let client = fake_api_client().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind host");
    let addr = listener.local_addr().expect("host address");
    tokio::spawn(serve(listener, dir.path().to_path_buf(), client));
    wait_until_serving(addr).await;

    // The console bundle is served, and a client-side route falls back to it.
    let shell = raw_http(addr, "GET", "/", None).await;
    assert!(shell.contains("200 OK"), "shell response: {shell}");
    assert!(
        shell.contains("OpenKite console"),
        "shell response: {shell}"
    );
    let deep_link = raw_http(addr, "GET", "/cluster", None).await;
    assert!(deep_link.contains("200 OK"), "deep link: {deep_link}");
    assert!(
        deep_link.contains("OpenKite console"),
        "deep link: {deep_link}"
    );

    // `list pods` — the console's live-data path — answers a real kube List.
    let response = raw_http(addr, "POST", "/openkite", Some(&list_pods_body())).await;
    assert!(response.contains("200 OK"), "list response: {response}");
    assert!(
        response.contains(r#""status":"ok""#),
        "list response: {response}"
    );
    assert!(
        response.contains(r#""kind":"PodList""#),
        "list response: {response}"
    );
    assert!(response.contains(POD_NAME), "list response: {response}");
    assert!(
        response.contains(POD_NAMESPACE),
        "list response: {response}"
    );

    // `list pods` in one namespace resolves the namespaced route too.
    let namespaced = json!({
        "id": 2,
        "plugin": "console",
        "request": {"op": "list", "kind": "pods", "ns": POD_NAMESPACE},
    })
    .to_string();
    let response = raw_http(addr, "POST", "/openkite", Some(&namespaced)).await;
    assert!(
        response.contains(r#""kind":"PodList""#),
        "namespaced list response: {response}"
    );

    // `watch pods` is answered from the reflector snapshot rather than a fresh
    // list, so it only carries rows once the reflectors have synced.
    let watch = json!({
        "id": 3,
        "plugin": "console",
        "request": {"op": "watch", "kind": "pods", "ns": null},
    })
    .to_string();
    let mut served_rows = false;
    for _ in 0..50 {
        let response = raw_http(addr, "POST", "/openkite", Some(&watch)).await;
        if response.contains(POD_NAME) {
            served_rows = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(served_rows, "watch op never served the reflector snapshot");

    // The console's context call reports the host it booted with.
    let response = raw_http(addr, "POST", "/openkite-spike", Some(r#"{"op":"context"}"#)).await;
    assert!(
        response.contains(r#""connected":true"#),
        "context response: {response}"
    );
    assert!(
        response.contains(r#""mutations":false"#),
        "context response: {response}"
    );
}

#[tokio::test]
async fn booted_host_refuses_a_missing_bundle() {
    // A directory with no index.html is what a failed COPY into the runtime looks
    // like; booting from it would answer 404 for every path.
    let dir = tempfile::tempdir().expect("tempdir");
    let client = fake_api_client().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind host");

    let err = serve(listener, dir.path().to_path_buf(), client)
        .await
        .expect_err("a bundle-less host must refuse to serve");
    let message = format!("{err:#}");
    assert!(
        message.contains("no console bundle"),
        "unexpected error: {message}"
    );
}
