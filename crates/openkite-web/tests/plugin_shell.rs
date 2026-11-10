//! The host shell paints what a plugin registered (OKT-196).
//!
//! A plugin reaches the shell by one path, on both hosts: it POSTs a `register`
//! envelope to `/openkite`, the host merges it into the bridge's live
//! `RegistrationStore`, and the shell is built from that store. On the web host
//! the store is the one the bridge keeps — the same object the desktop mounts —
//! and it rides the snapshot, so the SSR document and the hydrating client are
//! built from one set of registrations.
//!
//! These tests drive the real router rather than the model, because the defect
//! they cover was exactly that the render never read the store: every call
//! succeeded, the store held the registration, and the painted sidebar was
//! empty. A model-level test would have passed over it.

mod support;

use std::sync::Arc;

use axum::http::StatusCode;
use openkite_host::bridge::Bridge;
use serde_json::{json, Value};
use support::{app, get_body, post_json};

fn temp_root() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

/// The envelope the browser's bridge JS posts for one registration.
fn register_envelope(id: u64, plugin: &str, kind: &str, payload: Value) -> Value {
    json!({
        "id": id,
        "plugin": plugin,
        "request": {"op": "register", "kind": kind, "payload": payload},
    })
}

/// A registered sidebar entry is in the nav of a full page render.
#[tokio::test]
async fn a_registered_plugin_nav_entry_renders_in_the_host_sidebar() {
    let bridge = Arc::new(Bridge::new());
    let app = app(bridge.clone(), temp_root().path());

    let (status, envelope) = post_json(
        app.clone(),
        "/openkite",
        register_envelope(
            1,
            "argocd",
            "sidebar",
            json!({"label": "Argo CD", "route": "/argocd"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "POST /openkite: {envelope}");
    assert_eq!(
        envelope["status"], "ok",
        "the bridge accepted the registration: {envelope}"
    );

    let (status, body) = get_body(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    // The plugin's section, built by the shell from the live store.
    assert!(
        body.contains("data-plugin=\"argocd\""),
        "the sidebar has no section for the registered plugin: {body}"
    );
    // Its entry is a link to the route it named — the nav the user clicks.
    assert!(
        body.contains("href=\"/argocd\""),
        "the registered nav entry is not a link: {body}"
    );
}

/// A plugin whose stylesheet the crate refuses keeps a row: disabled, with the
/// reason, instead of disappearing.
#[tokio::test]
async fn a_refused_stylesheet_renders_a_disabled_row_with_the_reason() {
    let bridge = Arc::new(Bridge::new());
    let app = app(bridge.clone(), temp_root().path());

    for envelope in [
        register_envelope(
            1,
            "flux",
            "sidebar",
            json!({"label": "Flux", "route": "/flux"}),
        ),
        // `@font-face` is global by construction: the crate cannot scope it to
        // the plugin's subtree, so the sheet is refused.
        register_envelope(
            2,
            "flux",
            "style",
            json!({"css": "@font-face { font-family: x; src: url(x.woff2); }"}),
        ),
    ] {
        let (status, answer) = post_json(app.clone(), "/openkite", envelope).await;
        assert_eq!(status, StatusCode::OK, "POST /openkite: {answer}");
        assert_eq!(answer["status"], "ok", "registration refused: {answer}");
    }

    let (status, body) = get_body(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    // The plugin is named by a row the user can see...
    assert!(
        body.contains("data-plugin=\"flux\""),
        "the refused plugin has no row at all: {body}"
    );
    // ...as a disabled one, carrying the reason...
    assert!(
        body.contains("data-disabled=\"1\""),
        "the refused plugin's row is not disabled: {body}"
    );
    assert!(
        body.contains("@font-face is global and cannot be scoped"),
        "the disabled row does not carry the reason: {body}"
    );
    // ...with nowhere to go, and no sheet painted for it.
    assert!(
        !body.contains("href=\"/flux\""),
        "the disabled row is still a link: {body}"
    );
    assert!(
        !body.contains("data-plugin-css=\"flux\""),
        "the refused sheet was painted anyway: {body}"
    );
}

/// The SSR document and the hydrating client are built from one snapshot, so
/// the registrations the server painted are the ones the client hydrates.
#[tokio::test]
async fn the_hydration_snapshot_carries_the_live_registrations() {
    let bridge = Arc::new(Bridge::new());
    let app = app(bridge.clone(), temp_root().path());

    let (status, answer) = post_json(
        app.clone(),
        "/openkite",
        register_envelope(
            1,
            "argocd",
            "sidebar",
            json!({"label": "Argo CD", "route": "/argocd"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "POST /openkite: {answer}");

    let (status, body) = get_body(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    let snapshot = embedded_snapshot(&body);
    let registrations = &snapshot["registrations"];
    assert!(
        registrations["by_plugin"]["argocd"]["sidebar"][0]["route"] == "/argocd",
        "the client would hydrate an empty shell: {registrations}"
    );
}

/// The snapshot JSON the page embeds for the hydrating client.
fn embedded_snapshot(body: &str) -> Value {
    const OPEN: &str = "<script id=\"openkite-snapshot\" type=\"application/json\">";
    let start = body
        .find(OPEN)
        .unwrap_or_else(|| panic!("no snapshot script in the page: {body}"))
        + OPEN.len();
    let rest = &body[start..];
    let end = rest
        .find("</script>")
        .unwrap_or_else(|| panic!("unterminated snapshot script"));
    serde_json::from_str(&rest[..end]).expect("embedded snapshot is JSON")
}
