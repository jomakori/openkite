//! The fixture behind the PR's captures: the page the host serves.
//!
//! Same shape as `dump_card_fixture` (`openkite-ui/tests/card_actions.rs`): an
//! ignored test that writes a real render to the path the environment names.

mod support;

use std::sync::Arc;

use openkite_host::bridge::Bridge;

/// Writes the page the host serves for `/workloads` — the SSR render with the
/// stylesheet and the hydration scripts in place — to `OPENKITE_WEB_FIXTURE_OUT`.
/// Ignored by default; it exists only to regenerate the PR's capture, against
/// whatever cluster `KUBECONFIG` points the host at.
#[tokio::test]
#[ignore = "writes the PR visual fixture; set OPENKITE_WEB_FIXTURE_OUT"]
async fn dump_workloads_fixture() {
    let Ok(out) = std::env::var("OPENKITE_WEB_FIXTURE_OUT") else {
        return;
    };
    let client = openkite_web::connect()
        .await
        .expect("connect to the cluster");
    let root = tempfile::tempdir().expect("tempdir");
    let app = support::app(Arc::new(Bridge::connected(client)), root.path());
    let (status, body) = support::get_body(app, "/workloads").await;
    assert_eq!(status, axum::http::StatusCode::OK, "the fixture route");
    std::fs::write(&out, body).expect("write the workloads fixture");
    println!("wrote {out}");
}
