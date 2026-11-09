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
use openkite_web::ssr::Snapshot;
use serde_json::{json, Value};
use support::{
    app, fake_api_client, get_body, get_bytes, get_full, list_pods_envelope, post_json,
    unreachable_client, POD_NAME, POD_NAMESPACE,
};

fn temp_root() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

/// The client bundle URL a rendered document imports.
///
/// The host versions it by the hash of the staged bytes (OKT-195), so the value
/// is only knowable from the page itself — which is the point: a document and
/// the wasm it boots are one build.
fn client_import(page: &str) -> String {
    const OPEN: &str = "import init from \"";
    let start = page.find(OPEN).expect("the page imports the client bundle") + OPEN.len();
    let rest = &page[start..];
    rest[..rest.find('"').expect("the import is a quoted URL")].to_string()
}

/// A `subscribe` request in the envelope the console's JS posts.
fn subscribe_envelope() -> Value {
    json!({
        "id": 2,
        "plugin": "console",
        "request": {"op": "subscribe", "kind": "pods", "ns": null},
    })
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
    // The page head is the route's: the landing route is the Cluster section's
    // Overview entry, so the head is "Cluster › Overview".
    // (The SSR pass writes hydration markers inside the text nodes, so the
    //  assertion reads the head's own marker, not `<h1>Overview</h1>`.)
    assert!(
        body.contains("data-page=\"Overview\""),
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
        ] {
            assert!(
                body.contains(boot),
                "{route} does not boot the client ({boot}): {body}"
            );
        }
        // The import is versioned by the bundle's own bytes, so a document can
        // only ever boot the wasm from the build that rendered it (OKT-195).
        let import = client_import(&body);
        assert!(
            import.starts_with("/assets/bundle/") && import.ends_with("/openkite-web-client.js"),
            "{route} boots an unversioned client bundle ({import}): {body}"
        );
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

/// The snapshot JSON the page embeds for the client to boot from.
fn embedded_snapshot(body: &str) -> &str {
    let (_, rest) = body
        .split_once("id=\"openkite-snapshot\"")
        .expect("the page embeds the snapshot");
    let (_, json) = rest
        .split_once('>')
        .expect("the snapshot script tag closes");
    json.split_once("</script>")
        .expect("the snapshot script closes")
        .0
}

// --- The route the host was asked for (OKT-180) -----------------------------

/// Each primary route paints its own chrome, and the sidebar marks the entry
/// for the route the address named.
#[tokio::test]
async fn the_host_resolves_the_route_it_was_asked_for() {
    let dir = temp_root();
    let app = app(Arc::new(Bridge::new()), dir.path());

    for (path, route, page) in [
        ("/", "/", "Overview"),
        ("/cluster", "/cluster", "Nodes"),
        ("/workloads", "/workloads", "Pods"),
        ("/config", "/config", "ConfigMaps"),
        ("/logs", "/logs", "Logs"),
    ] {
        let (status, body) = get_body(app.clone(), path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(
            body.contains(&format!("data-route=\"{route}\"")),
            "{path} must paint the {route} route: {body}"
        );
        assert!(
            body.contains(&format!("data-page=\"{page}\"")),
            "{path} must be titled {page}: {body}"
        );
        // `/logs` is a console route without a sidebar entry (OKT-189): the
        // panel is reached by address and by the workload rows, not by a nav
        // item the sidebar does not carry.
        if route != "/" && route != "/logs" {
            assert!(
                body.contains("class=\"nav-item active\""),
                "{path} must mark its sidebar entry: {body}"
            );
        }
    }
}

/// A path the crate does not know falls back to the route the console lands on,
/// exactly as it did when this host served one document for every path.
#[tokio::test]
async fn an_unknown_path_falls_back_to_the_home_route() {
    let dir = temp_root();
    let app = app(Arc::new(Bridge::new()), dir.path());

    for path in ["/nonsense", "/terminal", "/workloads/pods/probe-pod"] {
        let (status, body) = get_body(app.clone(), path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(
            body.contains("data-route=\"/\""),
            "{path} must fall back to the home route: {body}"
        );
        assert!(
            body.contains("data-page=\"Overview\""),
            "{path} must paint the home route's head: {body}"
        );
    }
}

/// The routes this host has not wired are declared and titled, not filled with
/// data the host does not have.
#[tokio::test]
async fn the_unwired_routes_are_declared_not_invented() {
    let dir = temp_root();
    let app = app(Arc::new(Bridge::new()), dir.path());

    for (path, page) in [("/cluster", "Nodes"), ("/config", "ConfigMaps")] {
        let (status, body) = get_body(app.clone(), path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(
            body.contains(&format!("data-page=\"{page}\"")),
            "{path} must be titled {page}: {body}"
        );
        assert!(
            body.contains("class=\"nav-item active\""),
            "{path} must mark its sidebar entry: {body}"
        );
        assert!(
            body.contains("data-state=\"empty\""),
            "{path} declares the surface it has not wired: {body}"
        );
    }
}

/// `/workloads` paints the pods the host's own gateway listed: the console's
/// table, its rows from the cluster rather than from fixtures.
#[tokio::test]
async fn the_workloads_route_paints_the_clusters_pods() {
    let dir = temp_root();
    let app = app(
        Arc::new(Bridge::connected(fake_api_client().await)),
        dir.path(),
    );
    let (status, body) = get_body(app, "/workloads").await;
    assert_eq!(status, StatusCode::OK);

    for rendered in [
        "data-route=\"/workloads\"",
        "data-page=\"Pods\"",
        "class=\"nav-item active\"",
        "data-surface=\"workloads\"",
        "class=\"resource-table\"",
        "class=\"table-header table-row\"",
        "data-label=\"Name\"",
        ">probe-pod<",
    ] {
        assert!(
            body.contains(rendered),
            "the workloads route must render {rendered}: {body}"
        );
    }
    assert!(
        body.contains(&format!("\"namespace\":\"{POD_NAMESPACE}\"")),
        "the row carries the pod's namespace: {body}"
    );
    assert!(
        !body.contains("class=\"table-error\""),
        "the list resolved, so the table states no error: {body}"
    );
}

/// The page embeds the snapshot the client boots from, table and all.
#[tokio::test]
async fn the_workloads_page_embeds_a_snapshot_the_client_boots_from() {
    let dir = temp_root();
    let app = app(
        Arc::new(Bridge::connected(fake_api_client().await)),
        dir.path(),
    );
    let (_, body) = get_body(app, "/workloads").await;

    let snapshot: Snapshot =
        serde_json::from_str(embedded_snapshot(&body)).expect("the client parses this snapshot");

    assert_eq!(snapshot.route, "/workloads");
    assert!(
        snapshot.connected,
        "the fake API answered, so the host is connected"
    );
    assert!(snapshot.workloads.error.is_none(), "the list resolved");
    assert_eq!(
        snapshot.workloads.columns.len(),
        9,
        "the console's pod columns"
    );
    assert_eq!(snapshot.workloads.rows.len(), 1);
    assert!(
        snapshot.workloads.rows[0].id.contains(POD_NAME),
        "the row is the pod the gateway listed: {:?}",
        snapshot.workloads.rows[0].id
    );
}

/// A host with no cluster credential still paints rows for `?fixture=ready`
/// (OKT-210): the preview host that the browser visual gate captures has no
/// client, so `workload_table` used to fall through to the bridge and paint
/// the error surface instead — the bulk-selection bar could never appear in a
/// screenshot. `?fixture=empty` must still paint the empty state, so the demo
/// rows do not leak into the state the fixture asked for.
#[tokio::test]
async fn fixture_ready_paints_demo_rows_and_fixture_empty_stays_empty() {
    let dir = temp_root();
    let app = app(Arc::new(Bridge::new()), dir.path());

    let (status, body) = get_body(app.clone(), "/workloads?fixture=ready").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("data-state=\"ready\""),
        "fixture=ready must paint the ready state: {body}"
    );
    let snapshot: Snapshot =
        serde_json::from_str(embedded_snapshot(&body)).expect("the embedded snapshot parses");
    assert!(
        !snapshot.workloads.rows.is_empty(),
        "fixture=ready must carry demo rows: {body}"
    );

    let (status, body) = get_body(app, "/workloads?fixture=empty").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("data-state=\"empty\""),
        "fixture=empty must still paint the empty state: {body}"
    );
}

/// A gateway that cannot answer is stated on the route, never left blank.
#[tokio::test]
async fn a_refused_gateway_is_stated_on_the_workloads_route() {
    let dir = temp_root();
    // Only a bridge with a client that then refuses the read is "refused"
    // here: a bridge with no client at all is the no-cluster-credential case
    // (OKT-210), which now paints demo rows instead of an error.
    let app = app(
        Arc::new(Bridge::connected(unreachable_client())),
        dir.path(),
    );
    let (status, body) = get_body(app, "/workloads").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("class=\"table-state table-error\""),
        "the route must state the refusal: {body}"
    );
    assert!(
        body.contains("The gateway could not list pods:"),
        "the gateway's own words reach the page: {body}"
    );
    assert!(
        !body.contains("class=\"resource-table\""),
        "a refused gateway paints no table, blank or otherwise: {body}"
    );
}

/// The client's own refresh carries the table: what it swaps in comes from
/// `POST /api/gateway`, not only from the page it hydrated with.
#[tokio::test]
async fn the_gateway_refresh_carries_the_pods_the_route_paints() {
    let dir = temp_root();
    let app = app(
        Arc::new(Bridge::connected(fake_api_client().await)),
        dir.path(),
    );
    let (status, body) = post_json(app, "/api/gateway", json!({"op": "snapshot"})).await;
    assert_eq!(status, StatusCode::OK);

    let workloads = &body["snapshot"]["workloads"];
    let columns = workloads["columns"]
        .as_array()
        .expect("the console's columns");
    assert_eq!(columns.len(), 9, "the console's pod columns: {body}");
    assert_eq!(columns[0]["label"], "Name", "the first column: {body}");
    let rows = workloads["rows"]
        .as_array()
        .expect("the pods the gateway listed");
    assert_eq!(rows.len(), 1, "one pod in the fake cluster: {body}");
    assert_eq!(
        rows[0]["id"],
        format!("{POD_NAMESPACE}/{POD_NAME}"),
        "the row the route will paint: {body}"
    );
    assert_eq!(rows[0]["namespace"], POD_NAMESPACE, "{body}");
    assert!(workloads["error"].is_null(), "the list resolved: {body}");
}

#[tokio::test]
async fn list_pods_envelope_matches_the_console_body_shape() {
    let body: Value = list_pods_envelope();
    assert_eq!(body["request"]["op"], "list");
    assert_eq!(body["request"]["kind"], "pods");
    assert!(body["request"]["ns"].is_null());
}

#[tokio::test]
async fn subscribe_op_answers_with_a_sub_id_and_initial_snapshot() {
    let dir = temp_root();
    let (status, body) = post_json(
        app(Arc::new(Bridge::new()), dir.path()),
        "/openkite",
        subscribe_envelope(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    let sub = body["result"]["sub"].as_u64().expect("subscription id");
    assert!(sub > 0, "subscription ids start at 1: {body}");
    assert_eq!(
        body["result"]["initial"],
        json!([]),
        "a host with no cluster has no initial rows: {body}"
    );
}

#[tokio::test]
async fn unsubscribe_op_removes_it() {
    let dir = temp_root();
    let bridge = Arc::new(Bridge::new());
    let (_, body) = post_json(
        app(bridge.clone(), dir.path()),
        "/openkite",
        subscribe_envelope(),
    )
    .await;
    let sub = body["result"]["sub"].as_u64().expect("subscription id");

    let unsubscribe = json!({
        "id": 3,
        "plugin": "console",
        "request": {"op": "unsubscribe", "sub": sub},
    });
    let (status, body) = post_json(
        app(bridge.clone(), dir.path()),
        "/openkite",
        unsubscribe.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["unsubscribed"], true, "{body}");

    let (_, body) = post_json(app(bridge, dir.path()), "/openkite", unsubscribe).await;
    assert_eq!(
        body["result"]["unsubscribed"], false,
        "a second cancel is a no-op: {body}"
    );
}

/// `/logs` paints the console's own log panel: the same `.log-panel` component
/// the desktop mounts, filled from the cluster's log endpoint rather than from a
/// fixture, so the lines on the panel are the pod's log and not a canned body.
#[tokio::test]
async fn the_logs_route_paints_the_shared_panel_from_the_clusters_log() {
    let dir = temp_root();
    let app = app(
        Arc::new(Bridge::connected(fake_api_client().await)),
        dir.path(),
    );
    let (status, body) = get_body(app, "/logs").await;
    assert_eq!(status, StatusCode::OK);

    for rendered in [
        "data-route=\"/logs\"",
        "data-page=\"Logs\"",
        "data-surface=\"logs\"",
        "data-state=\"ready\"",
        // The shared component's own chrome, not a second log view.
        "class=\"log-panel\"",
        "class=\"log-body\"",
        "class=\"log-line\"",
        // Level colouring by the desktop's classes.
        "class=\"log-level warn\"",
        "class=\"log-level error\"",
        // A line the cluster's log endpoint answered with.
        "upstream refused",
    ] {
        assert!(
            body.contains(rendered),
            "the logs route must render {rendered}: {body}"
        );
    }
    assert!(
        !body.contains("data-page=\"Overview\""),
        "the logs route is not the home fallback: {body}"
    );
}

/// The panel's colouring cannot drift between hosts: both mount the same
/// stylesheet, so `/logs` carries the token definitions the levels read.
#[tokio::test]
async fn the_logs_page_carries_the_shared_log_level_tokens() {
    let dir = temp_root();
    let app = app(Arc::new(Bridge::new()), dir.path());
    let (status, body) = get_body(app, "/logs").await;
    assert_eq!(status, StatusCode::OK);
    for token in [
        "--log-info:",
        "--log-method:",
        "--log-error:",
        ".log-level.warn",
        ".log-level.error",
    ] {
        assert!(
            body.contains(token),
            "the logs page must carry the shared token {token}: {body}"
        );
    }
}

/// The log a refused read cannot paint is stated, not left as an empty panel:
/// a host with no cluster says so inside the same shell.
#[tokio::test]
async fn the_logs_route_states_a_refused_log_read() {
    let dir = temp_root();
    let app = app(Arc::new(Bridge::new()), dir.path());
    let (status, body) = get_body(app, "/logs").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("data-state=\"error\""),
        "a host with no cluster states the refusal: {body}"
    );
    assert!(
        body.contains("class=\"log-panel\""),
        "the refusal paints inside the panel shell: {body}"
    );
}

/// A real stream, not a fixture: the log endpoint the panel reads grows between
/// reads, and each read answers the panel's line shape (`lines` as a list).
#[tokio::test]
async fn the_log_endpoint_streams_new_lines_into_the_panel() {
    let dir = temp_root();
    let app = app(
        Arc::new(Bridge::connected(fake_api_client().await)),
        dir.path(),
    );
    let path = "/api/logs?pod=probe-pod&ns=default";

    let (status, first) = get_body(app.clone(), path).await;
    assert_eq!(status, StatusCode::OK);
    let first: Value = serde_json::from_str(&first).expect("logs pane envelope");
    assert_eq!(first["pod"]["name"], "probe-pod", "{first}");
    let first_lines = first["lines"].as_array().expect("lines as a list").len();
    assert!(first_lines >= 4, "the tail is read whole: {first}");

    let (status, second) = get_body(app, path).await;
    assert_eq!(status, StatusCode::OK);
    let second: Value = serde_json::from_str(&second).expect("logs pane envelope");
    let second_lines = second["lines"].as_array().expect("lines as a list").len();
    assert!(
        second_lines > first_lines,
        "the stream appends between reads: {first_lines} then {second_lines}"
    );
    assert!(
        second["lines"].to_string().contains("stream tick"),
        "the panel receives the lines the stream appended: {second}"
    );
}

#[tokio::test]
async fn publish_on_the_web_host_delivers_nothing() {
    let dir = temp_root();
    let bridge = Arc::new(Bridge::new());
    let (_, body) = post_json(app(bridge, dir.path()), "/openkite", subscribe_envelope()).await;
    assert!(body["result"]["sub"].as_u64().is_some(), "{body}");

    // Nothing here calls `push::install()`, so a publish has no pump to reach
    // and answers zero. That zero is the browser's degradation signal.
    let sent =
        openkite_host::push::publish("pods", None, vec![json!({"metadata": {"name": "api-0"}})]);
    assert_eq!(sent, 0, "the web host installs no pump");
}
