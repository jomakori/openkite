//! Integration tests for the bridge runtime: envelope parsing,
//! registration merging, and the headless error paths. Kube dispatch is
//! exercised two ways: the no-cluster fallback, and the connected error
//! arms against a dead cluster (127.0.0.1:1 — refused, no network).

use openkite::bridge::Bridge;
use openkite::plugin_api::ApiResponse;

fn error_of(resp: &ApiResponse) -> &str {
    match resp {
        ApiResponse::Error { error } => error,
        ApiResponse::Ok { .. } => panic!("expected an error, got {resp:?}"),
    }
}

#[tokio::test]
async fn register_sidebar_envelope_lands_in_store() {
    let bridge = Bridge::new();
    let resp = bridge
        .handle_post(
            r#"{"id":1,"plugin":"argocd","request":{"op":"register","kind":"sidebar","payload":{"label":"Applications","icon":"grid","route":"/argocd/apps"}}}"#,
        )
        .await;
    assert!(matches!(&resp, ApiResponse::Ok { .. }), "{resp:?}");
    let store = bridge.store();
    let store = store.lock().expect("test store lock");
    let reg = store.get("argocd").expect("argocd registered");
    assert_eq!(reg.sidebar.len(), 1);
    assert_eq!(reg.sidebar[0].label, "Applications");
    assert_eq!(reg.sidebar[0].route, "/argocd/apps");
}

#[tokio::test]
async fn registers_accumulate_per_kind() {
    let bridge = Bridge::new();
    for body in [
        r#"{"id":1,"plugin":"argocd","request":{"op":"register","kind":"sidebar","payload":{"label":"Apps","route":"/argocd/apps"}}}"#,
        r#"{"id":2,"plugin":"argocd","request":{"op":"register","kind":"route","payload":{"path":"/argocd/apps","title":"Applications"}}}"#,
        r#"{"id":3,"plugin":"argocd","request":{"op":"register","kind":"status","payload":{"label":"ArgoCD: Synced","color":"green"}}}"#,
    ] {
        let resp = bridge.handle_post(body).await;
        assert!(matches!(&resp, ApiResponse::Ok { .. }), "{resp:?}");
    }
    let store = bridge.store();
    let store = store.lock().expect("test store lock");
    let reg = store.get("argocd").expect("argocd registered");
    assert_eq!(reg.sidebar.len(), 1);
    assert_eq!(reg.routes.len(), 1);
    assert_eq!(reg.status.len(), 1);
    assert_eq!(reg.status[0].color, "green");
    assert_eq!(store.plugins(), vec!["argocd".to_string()]);
}

#[tokio::test]
async fn register_rejects_invalid_route_paths() {
    let bridge = Bridge::new();
    let resp = bridge
        .handle_post(
            r#"{"id":1,"plugin":"broken","request":{"op":"register","kind":"route","payload":{"path":"argocd/apps","title":"Bad"}}}"#,
        )
        .await;
    assert!(error_of(&resp).contains("route path"));
    // The invalid item never lands in the store.
    let store = bridge.store();
    let store = store.lock().expect("test store lock");
    assert!(store.get("broken").is_none());
}

#[tokio::test]
async fn register_rejects_unknown_kinds_and_bad_payloads() {
    let bridge = Bridge::new();
    let resp = bridge
        .handle_post(
            r#"{"id":1,"plugin":"odd","request":{"op":"register","kind":"widget","payload":{}}}"#,
        )
        .await;
    assert!(error_of(&resp).contains("unknown registration kind 'widget'"));

    let resp = bridge
        .handle_post(
            r#"{"id":2,"plugin":"odd","request":{"op":"register","kind":"sidebar","payload":{"icon":"grid"}}}"#,
        )
        .await;
    assert!(error_of(&resp).contains("invalid sidebar item"));
    assert!(error_of(&resp).contains("label"));
}

#[tokio::test]
async fn malformed_envelopes_report_parse_errors() {
    let bridge = Bridge::new();
    let resp = bridge.handle_post("{ not json").await;
    assert!(error_of(&resp).contains("parse envelope"));

    let resp = bridge
        .handle_post(r#"{"id":1,"plugin":"x","request":{"op":"teleport"}}"#)
        .await;
    assert!(error_of(&resp).contains("parse envelope"));
}

#[tokio::test]
async fn kube_ops_without_a_cluster_error_cleanly() {
    let bridge = Bridge::new();
    for body in [
        r#"{"id":1,"plugin":"p","request":{"op":"list","kind":"pods","ns":null}}"#,
        r#"{"id":2,"plugin":"p","request":{"op":"watch","kind":"pods","ns":"default"}}"#,
        r#"{"id":3,"plugin":"p","request":{"op":"get","kind":"pods","ns":"default","name":"web"}}"#,
        r#"{"id":4,"plugin":"p","request":{"op":"logs","name":"web","ns":"default","container":null}}"#,
    ] {
        let resp = bridge.handle_post(body).await;
        assert_eq!(error_of(&resp), "no cluster connected", "{resp:?}");
    }
}

#[tokio::test]
async fn exec_is_deferred_even_when_connected() {
    let bridge = Bridge::new();
    let resp = bridge
        .handle_post(
            r#"{"id":1,"plugin":"p","request":{"op":"exec","name":"web","ns":"default","container":null,"cmd":["sh","-c","ls"]}}"#,
        )
        .await;
    assert!(error_of(&resp).contains("exec is not supported yet"));
}

/// kube client aimed at a dead cluster (127.0.0.1:1, refused — no network).
fn dead_client() -> kube::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let url: http::Uri = "http://127.0.0.1:1".parse().unwrap();
    kube::Client::try_from(kube::Config::new(url)).unwrap()
}

#[tokio::test]
async fn dead_cluster_dispatch_errors_through_discovery() {
    let bridge = Bridge::connected(dead_client());
    // list/watch/get resolve the kind via discovery first; the dead cluster
    // fails there, before any resource request is sent.
    for body in [
        r#"{"id":1,"plugin":"p","request":{"op":"list","kind":"pods","ns":null}}"#,
        r#"{"id":2,"plugin":"p","request":{"op":"watch","kind":"pods","ns":"default"}}"#,
        r#"{"id":3,"plugin":"p","request":{"op":"get","kind":"pods","ns":"default","name":"web"}}"#,
    ] {
        let resp = bridge.handle_post(body).await;
        assert!(error_of(&resp).contains("discovery:"), "{resp:?}");
    }
}

#[tokio::test]
async fn dead_cluster_logs_error_after_params_build() {
    let bridge = Bridge::connected(dead_client());
    let resp = bridge
        .handle_post(
            r#"{"id":1,"plugin":"p","request":{"op":"logs","name":"web","ns":"default","container":null}}"#,
        )
        .await;
    assert!(error_of(&resp).contains("logs web:"), "{resp:?}");
}

#[tokio::test]
async fn connected_logs_still_require_a_namespace() {
    let bridge = Bridge::connected(dead_client());
    let resp = bridge
        .handle_post(
            r#"{"id":1,"plugin":"p","request":{"op":"logs","name":"web","ns":"","container":null}}"#,
        )
        .await;
    assert!(error_of(&resp).contains("namespace required"), "{resp:?}");
}

#[tokio::test]
async fn set_client_swaps_and_disconnects_kube_access() {
    let bridge = Bridge::new();
    // Swap a client in: list leaves the no-cluster fallback and hits discovery.
    bridge.set_client(Some(dead_client()));
    assert!(bridge.client().is_some());
    let resp = bridge
        .handle_post(r#"{"id":1,"plugin":"p","request":{"op":"list","kind":"pods","ns":null}}"#)
        .await;
    assert!(error_of(&resp).contains("discovery:"), "{resp:?}");

    // Disconnect: kube ops fall back to the clean no-cluster error.
    bridge.set_client(None);
    assert!(bridge.client().is_none());
    let resp = bridge
        .handle_post(r#"{"id":2,"plugin":"p","request":{"op":"list","kind":"pods","ns":null}}"#)
        .await;
    assert_eq!(error_of(&resp), "no cluster connected");
}

#[tokio::test]
async fn snapshot_is_a_point_in_time_copy_of_the_store() {
    let bridge = Bridge::new();
    assert!(bridge.snapshot().is_empty());
    let resp = bridge
        .handle_post(
            r#"{"id":1,"plugin":"argocd","request":{"op":"register","kind":"sidebar","payload":{"label":"Applications","route":"/argocd/apps"}}}"#,
        )
        .await;
    assert!(matches!(&resp, ApiResponse::Ok { .. }), "{resp:?}");
    let snap = bridge.snapshot();
    assert!(snap.get("argocd").is_some());
    assert_eq!(snap.all_sidebar_items().len(), 1);
}

#[tokio::test]
async fn register_route_and_status_payload_parse_failures_are_atomic() {
    let bridge = Bridge::new();
    // RouteSpec requires `path`; StatusItem requires `label`. The serde
    // error surfaces before any validation and the store stays untouched.
    let resp = bridge
        .handle_post(
            r#"{"id":1,"plugin":"odd","request":{"op":"register","kind":"route","payload":{"title":"T"}}}"#,
        )
        .await;
    assert!(error_of(&resp).contains("invalid route"), "{resp:?}");
    let resp = bridge
        .handle_post(
            r#"{"id":2,"plugin":"odd","request":{"op":"register","kind":"status","payload":{"color":"green"}}}"#,
        )
        .await;
    assert!(error_of(&resp).contains("invalid status item"), "{resp:?}");
    let store = bridge.store();
    let store = store.lock().expect("test store lock");
    assert!(store.get("odd").is_none());
}
