//! Route-level tests for the host's HTTP surface.
//!
//! Every bridge case asserts the envelope the console's `fetch` reads, because
//! that is the contract a browser build depends on: HTTP 200 plus a
//! `{status: "ok" | "error"}` body — never a bare status code, which
//! `web/src/bridge.ts` reads as "no host here" and answers with fixtures.

mod support;

use std::sync::Arc;

use axum::http::StatusCode;
use openkite_host::bridge::Bridge;
use serde_json::{json, Value};
use support::{
    app, get_body, get_bytes, get_full, list_pods_envelope, post_json, unreachable_client,
};

fn temp_root() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

#[tokio::test]
async fn root_route_renders_the_shared_console_shell() {
    let (status, body) = get_body(app(Arc::new(Bridge::new()), temp_root().path()), "/").await;
    assert_eq!(status, StatusCode::OK);
    for chrome in [
        "class=\"app\"",
        "class=\"sidebar\"",
        "class=\"brand\"",
        "class=\"nav-item\"",
        "class=\"topbar\"",
        "class=\"breadcrumbs\"",
        "class=\"view active\"",
        "class=\"panel\"",
        "class=\"kv-list\"",
        "class=\"sidebar-footer\"",
        "class=\"status-line\"",
        // The route chrome (OKT-155): the page this host serves is a route
        // like any other, so it carries the same head, toolbar and declaration
        // row the desktop's routes mount.
        "class=\"page-head\"",
        "class=\"page-sub\"",
        "class=\"page-actions\"",
        "class=\"toolbar\"",
        "class=\"chip-row\"",
        "class=\"tag-row\"",
        "data-route=\"/\"",
    ] {
        assert!(body.contains(chrome), "root route missing {chrome}: {body}");
    }
    // The page head is the route's: the home route's title is the cluster's.
    // (The SSR pass writes hydration markers inside the text nodes, so the
    //  assertion reads the head's own marker, not `<h1>Cluster</h1>`.)
    assert!(
        body.contains("data-page=\"Cluster\""),
        "root route page head: {body}"
    );
    assert!(
        !body.contains("data-state=\"empty\""),
        "the snapshot fills the route, so it has no empty state: {body}"
    );
    for declared in [
        "data-unsupported=\"cluster-switch\"",
        "context switching: off",
    ] {
        assert!(
            body.contains(declared),
            "a server-side host declares {declared}: {body}"
        );
    }
    for surface in ["app", "overview", "capabilities", "secrets"] {
        assert!(
            body.contains(&format!("data-surface=\"{surface}\"")),
            "root route missing the {surface} surface: {body}"
        );
    }
    assert!(
        body.contains("no cluster · Disconnected"),
        "root route status footer: {body}"
    );
    assert!(
        body.contains("class=\"pill danger\""),
        "root route pill: {body}"
    );
    assert!(!body.contains("class=\"surface\""), "root route: {body}");
    assert!(!body.contains("class=\"status-ok"), "root route: {body}");
    assert!(
        body.contains("id=\"openkite-snapshot\""),
        "root route must embed the snapshot: {body}"
    );
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

/// The address carries the resource detail pane's selection (OKT-175): the
/// resource named in the query string is on the page the server renders, so a
/// reload or a shared link reopens the same pane — no client needed.
#[tokio::test]
async fn the_address_carries_the_detail_pane_selection() {
    let dir = temp_root();
    let (status, body) = get_body(
        app(Arc::new(Bridge::new()), dir.path()),
        "/workloads?kind=Pod&ns=default&name=checkout-api-7d9f",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for rendered in [
        "class=\"inspector open\"",
        "data-pane=\"resource\"",
        "data-kind=\"Pod\"",
        "checkout-api-7d9f",
        "namespace: default",
        "class=\"inspector-actions\"",
    ] {
        assert!(
            body.contains(rendered),
            "the deep link must paint the pane with {rendered}: {body}"
        );
    }
}

/// Without a selection the pane is not on the page at all — the same document
/// the root route always served.
#[tokio::test]
async fn a_plain_address_renders_no_detail_pane() {
    let dir = temp_root();
    let (status, body) = get_body(app(Arc::new(Bridge::new()), dir.path()), "/workloads").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !body.contains("data-pane=\"resource\""),
        "no selection means no pane: {body}"
    );
    assert!(!body.contains("class=\"inspector-scrim"), "got: {body}");
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
    assert_eq!(result["version"], json!(openkite::version::reported()));
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
async fn every_console_route_renders_the_shell_from_an_empty_web_root() {
    let dir = temp_root();
    let app = app(Arc::new(Bridge::new()), dir.path());

    for route in ["/cluster", "/workloads", "/config", "/logs", "/terminal"] {
        let (status, body) = get_body(app.clone(), route).await;
        assert_eq!(status, StatusCode::OK, "{route} did not render the console");
        assert!(!body.is_empty(), "{route} answered an empty body");
        for chrome in [
            "class=\"app\"",
            "class=\"sidebar\"",
            "id=\"openkite-snapshot\"",
        ] {
            assert!(body.contains(chrome), "{route} missing {chrome}: {body}");
        }
    }
}

#[tokio::test]
async fn every_console_route_boots_the_client_the_image_stages() {
    let dir = temp_root();
    let app = app(Arc::new(Bridge::new()), dir.path());

    for route in [
        "/",
        "/cluster",
        "/workloads",
        "/config",
        "/logs",
        "/terminal",
        "/workloads/pods/probe-pod",
    ] {
        let (status, body) = get_body(app.clone(), route).await;
        assert_eq!(status, StatusCode::OK, "{route} did not render the console");
        for boot in [
            "id=\"openkite-snapshot\"",
            "window.initial_dioxus_hydration_data=",
            r#"import init from "/openkite-web-client.js";"#,
        ] {
            assert!(
                body.contains(boot),
                "{route} does not boot the client ({boot}): {body}"
            );
        }
    }
}

#[tokio::test]
async fn hydration_assets_in_the_web_root_are_served_directly() {
    let dir = temp_root();
    let client = "export default () => {};";
    std::fs::write(dir.path().join("openkite-web-client.js"), client).expect("write client");
    let app = app(Arc::new(Bridge::new()), dir.path());

    let (status, body) = get_body(app.clone(), "/openkite-web-client.js").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, client);

    let (status, body) = get_body(app, "/terminal").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("id=\"openkite-snapshot\""),
        "/terminal missing the console shell: {body}"
    );
}

#[tokio::test]
async fn every_vendored_typeface_is_served_byte_for_byte() {
    let dir = temp_root();
    let app = app(Arc::new(Bridge::new()), dir.path());
    for face in openkite_ui::assets::FACES {
        let url = openkite_ui::assets::src_url(face);
        let (status, headers, bytes) = get_full(app.clone(), &url).await;
        assert_eq!(status, StatusCode::OK, "{url}");
        assert_eq!(
            headers
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("font/woff2"),
            "{url}"
        );
        assert_eq!(
            headers
                .get(axum::http::header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("public, max-age=31536000, immutable"),
            "{url}"
        );
        assert_eq!(bytes, face.bytes, "{url} did not round-trip");
    }
}

#[tokio::test]
async fn an_unvendored_typeface_is_a_404_not_the_spa_shell() {
    let dir = temp_root();
    std::fs::write(dir.path().join("index.html"), "<!doctype html>SHELL").expect("write index");
    let app = app(Arc::new(Bridge::new()), dir.path());

    let (status, body) = get_body(app.clone(), "/assets/fonts/ibm-plex-sans-800.woff2").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body, "unknown typeface");

    let (status, _) = get_bytes(app, "/assets/fonts/..%2Findex.html").await;
    assert_ne!(
        status,
        StatusCode::OK,
        "the face route served a path escape"
    );
}

#[tokio::test]
async fn reflectors_start_in_the_hosts_headless_runtime() {
    let started = openkite_web::headless::start_reflectors(unreachable_client());
    assert!(started > 0, "reflectors started: {started}");
    assert!(openkite_host::state::live::is_watching());
}

#[tokio::test]
async fn list_pods_envelope_matches_the_console_body_shape() {
    let body: Value = list_pods_envelope();
    assert_eq!(body["request"]["op"], "list");
    assert_eq!(body["request"]["kind"], "pods");
    assert!(body["request"]["ns"].is_null());
}
