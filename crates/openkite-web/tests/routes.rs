//! Route-level tests for the host's HTTP surface.
//!
//! Every bridge case asserts the envelope the console's `fetch` reads, because
//! that is the contract a browser build depends on: HTTP 200 plus a
//! `{status: "ok" | "error"}` body — never a bare status code, which
//! `web/src/bridge.ts` reads as "no host here" and answers with fixtures.

mod support;

use std::sync::Arc;

use axum::http::StatusCode;
use openkite::bridge::Bridge;
use serde_json::{json, Value};
use support::{app, get_body, list_pods_envelope, post_json, unreachable_client};

fn temp_root() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

#[tokio::test]
async fn bridge_route_answers_the_no_cluster_envelope() {
    let dir = temp_root();
    let (status, body) = post_json(
        app(Arc::new(Bridge::new()), dir.path()),
        "/openkite",
        list_pods_envelope(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "error");
    assert_eq!(body["error"], "no cluster connected");
}

#[tokio::test]
async fn bridge_route_merges_register_posts() {
    let dir = temp_root();
    let bridge = Arc::new(Bridge::new());
    let (status, body) = post_json(
        app(bridge.clone(), dir.path()),
        "/openkite",
        json!({
            "id": 1,
            "plugin": "argocd",
            "request": {
                "op": "register",
                "kind": "sidebar",
                "payload": {"label": "Applications", "route": "/argocd/apps"},
            },
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");

    let store = bridge.store();
    let store = store.lock().expect("store lock");
    let registration = store.get("argocd").expect("argocd registered");
    assert_eq!(registration.sidebar.len(), 1);
}

#[tokio::test]
async fn bridge_route_answers_kube_failures_in_the_envelope() {
    let dir = temp_root();
    let bridge = Arc::new(Bridge::connected(unreachable_client()));
    let (status, body) =
        post_json(app(bridge, dir.path()), "/openkite", list_pods_envelope()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "error");
    let error = body["error"].as_str().expect("error string");
    // Kind resolution runs discovery first, so a dead API server fails there.
    assert!(error.starts_with("discovery"), "unexpected error: {error}");
}

#[tokio::test]
async fn bridge_route_rejects_an_unparsable_envelope() {
    let dir = temp_root();
    let (status, body) = post_json(
        app(Arc::new(Bridge::new()), dir.path()),
        "/openkite",
        json!({"id": 1, "plugin": "console"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "error");
    assert!(body["error"]
        .as_str()
        .expect("error string")
        .starts_with("parse envelope"));
}

#[tokio::test]
async fn spike_context_reports_the_host_identity() {
    let dir = temp_root();
    let bridge = Arc::new(Bridge::connected(unreachable_client()));
    let (status, body) = post_json(
        app(bridge, dir.path()),
        "/openkite-spike",
        json!({"op": "context"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    let result = &body["result"];
    assert_eq!(result["connected"], true);
    assert_eq!(result["mutations"], false);
    assert!(result["context"].is_string(), "context label: {result}");
    assert_eq!(result["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn spike_context_reports_a_disconnected_host() {
    let dir = temp_root();
    let (_, body) = post_json(
        app(Arc::new(Bridge::new()), dir.path()),
        "/openkite-spike",
        json!({"op": "context"}),
    )
    .await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["result"]["connected"], false);
}

#[tokio::test]
async fn spike_settings_get_answers_the_console_snapshot() {
    let dir = temp_root();
    let (status, body) = post_json(
        app(Arc::new(Bridge::new()), dir.path()),
        "/openkite-spike",
        json!({"op": "settings_get"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    let result = &body["result"];
    for key in [
        "theme",
        "fontSize",
        "metricsEnabled",
        "menuBar",
        "titleBarTheme",
        "themeVars",
        "themes",
        "menuBarHideable",
        "titleBarOverridable",
        "version",
        "capabilities",
    ] {
        assert!(result.get(key).is_some(), "missing {key} in {result}");
    }
    // A server has no menu bar and no window chrome to toggle.
    assert_eq!(result["menuBarHideable"], false);
    assert_eq!(result["titleBarOverridable"], false);
    assert_eq!(result["capabilities"]["mutations"], false);
    assert!(!result["themes"]
        .as_array()
        .expect("theme catalog")
        .is_empty());
    assert!(!result["themeVars"]
        .as_object()
        .expect("theme vars")
        .is_empty());
}

#[tokio::test]
async fn spike_settings_set_rejects_an_unrenderable_font_size() {
    let dir = temp_root();
    let (status, body) = post_json(
        app(Arc::new(Bridge::new()), dir.path()),
        "/openkite-spike",
        json!({
            "op": "settings_set",
            "theme": null,
            "fontSize": 99,
            "metricsEnabled": true,
            "menuBar": "show",
            "titleBarTheme": "system",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "error");
    let error = body["error"].as_str().expect("error string");
    assert!(error.contains("outside 9-24"), "unexpected error: {error}");
}

#[tokio::test]
async fn spike_route_answers_an_unparsable_request() {
    let dir = temp_root();
    let (status, body) = post_json(
        app(Arc::new(Bridge::new()), dir.path()),
        "/openkite-spike",
        json!({"op": "no_such_op"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "error");
    assert!(body["error"]
        .as_str()
        .expect("error string")
        .starts_with("parse spike request"));
}

#[tokio::test]
async fn static_assets_are_served_with_an_spa_fallback() {
    let dir = temp_root();
    std::fs::write(dir.path().join("index.html"), "<!doctype html>SHELL").expect("write index");
    std::fs::write(dir.path().join("app.js"), "console.log('console')").expect("write asset");
    let app = app(Arc::new(Bridge::new()), dir.path());

    let (status, body) = get_body(app.clone(), "/app.js").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "console.log('console')");

    // A client-side route is not a file: it must answer the shell.
    let (status, body) = get_body(app, "/cluster").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "<!doctype html>SHELL");
}

#[tokio::test]
async fn reflectors_start_in_the_hosts_headless_runtime() {
    // The host has no Dioxus runtime of its own, and a signal cannot be created
    // without one — so this asserts the wiring that makes the reflector set
    // startable here at all, and that it is live afterwards.
    let started = openkite_web::headless::start_reflectors(unreachable_client());
    assert!(started > 0, "reflectors started: {started}");
    assert!(openkite::state::live::is_watching());
}

#[tokio::test]
async fn list_pods_envelope_matches_the_console_body_shape() {
    // Guards the harness the cases above rely on: the console's own body shape
    // must round-trip through the envelope, not a lookalike.
    let body: Value = list_pods_envelope();
    assert_eq!(body["request"]["op"], "list");
    assert_eq!(body["request"]["kind"], "pods");
    assert!(body["request"]["ns"].is_null());
}
