//! Integration tests for the bridge runtime: envelope parsing,
//! registration merging, and the headless error paths. kube dispatch needs
//! a live apiserver; the wire contract here pins what the shell mounts.

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
