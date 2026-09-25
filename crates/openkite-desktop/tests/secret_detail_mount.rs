//! Headless mounts of the Secret detail slide-over (coverage roadmap B5).
//! tests/secret_detail.rs pins the decode/label helpers; these run the
//! `SecretDetail` render body for the closed (no selection) and open
//! (masked key/value rows) states. Effects that call `document::eval` are
//! registered but never run by `rebuild_in_place`, so the mount stays
//! headless-safe.

mod support;

use std::collections::BTreeMap;

use dioxus::prelude::*;
use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::ByteString;

use openkite::components::secret_detail::SecretDetail;
use openkite::runtime::SELECTED_SECRET;

fn closed_detail() -> Element {
    rsx! {
        SecretDetail {}
        div { "shell" }
    }
}

fn open_detail() -> Element {
    rsx! { SecretDetail {} }
}

fn make_secret() -> Secret {
    let mut data = BTreeMap::<String, ByteString>::new();
    data.insert("password".to_string(), ByteString(b"hunter2".to_vec()));
    data.insert("token".to_string(), ByteString(b"tok123".to_vec()));
    Secret {
        metadata: ObjectMeta {
            name: Some("app".into()),
            namespace: Some("default".into()),
            ..Default::default()
        },
        data: Some(data),
        ..Default::default()
    }
}

#[test]
fn no_selection_renders_nothing() {
    let html = support::mount_html(closed_detail, || {});
    assert!(html.contains("shell"), "got: {html}");
    assert!(!html.contains("inspector"), "got: {html}");
}

#[test]
fn selected_secret_renders_masked_key_value_rows() {
    let html = support::mount_html(open_detail, || {
        *SELECTED_SECRET.write() = Some(make_secret());
    });
    assert!(html.contains("inspector open"), "got: {html}");
    assert!(html.contains("app"), "got: {html}");
    assert!(html.contains("Secret"), "got: {html}");
    assert!(html.contains("namespace: default"), "got: {html}");
    assert!(html.contains("Opaque"), "got: {html}");
    // One masked row per data key + the per-row Reveal/Hide/Copy buttons.
    assert!(html.contains("password"), "got: {html}");
    assert!(html.contains("token"), "got: {html}");
    assert!(html.contains("********"), "got: {html}");
    assert!(html.contains("Reveal all values"), "got: {html}");
    assert!(html.contains("reveal-btn"), "got: {html}");
    assert!(html.contains("hide-btn"), "got: {html}");
    assert!(html.contains("copy-btn"), "got: {html}");
    assert!(html.contains("value-masked"), "got: {html}");
}
