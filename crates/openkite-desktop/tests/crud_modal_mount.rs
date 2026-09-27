//! Headless mounts of the CRUD overlay + every modal (coverage roadmap B5).
//! Seeding CRUD_TARGET through the real `open_*` helpers and mounting
//! `CrudOverlay` exercises the dispatch arms and each modal's render body
//! (starter docs, validation, disabled-until-typed gates) — no cluster, no
//! apply path.

mod support;

use dioxus::prelude::*;
use openkite::components::crud_modal::CrudOverlay;
use openkite::runtime::{open_delete_for, open_editor_for, open_new_for, open_scale_for};
use serde_json::{json, Value};

fn crud_overlay() -> Element {
    rsx! { CrudOverlay {} }
}

fn crud_overlay_with_host() -> Element {
    rsx! {
        CrudOverlay {}
        div { "host" }
    }
}

fn edit_doc() -> Value {
    json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {
            "name": "web",
            "namespace": "default",
            "resourceVersion": "4242"
        }
    })
}

#[test]
fn closed_overlay_renders_nothing() {
    let html = support::mount_html(crud_overlay_with_host, || {});
    assert!(html.contains("host"), "got: {html}");
    assert!(!html.contains("modal"), "got: {html}");
}

#[test]
fn new_target_dispatches_new_editor() {
    let html = support::mount_html(crud_overlay, || open_new_for("Pod".into()));
    assert!(html.contains("New resource"), "got: {html}");
    assert!(html.contains("Pod"), "got: {html}");
    assert!(html.contains("Create"), "got: {html}");
}

#[test]
fn edit_target_dispatches_edit_editor() {
    let html = support::mount_html(crud_overlay, || {
        open_editor_for("Deployment".into(), edit_doc())
    });
    assert!(html.contains("Edit resource"), "got: {html}");
    assert!(html.contains("Deployment"), "got: {html}");
    assert!(html.contains("Apply"), "got: {html}");
}

#[test]
fn delete_target_dispatches_confirm_delete() {
    // Namespace-less triple exercises the `Pod · name` eyebrow arm.
    let html = support::mount_html(crud_overlay, || {
        open_delete_for("Pod".into(), None, "standalone".into())
    });
    assert!(html.contains("Delete resource"), "got: {html}");
    assert!(html.contains("Pod · standalone"), "got: {html}");
    assert!(html.contains("Deletion is irreversible."), "got: {html}");
    // Typed-name gate: empty input keeps Delete disabled but rendered.
    assert!(html.contains("to confirm."), "got: {html}");
    assert!(html.contains("btn btn-danger"), "got: {html}");
}

#[test]
fn scale_target_dispatches_confirm_scale() {
    let html = support::mount_html(crud_overlay, || {
        open_scale_for("Deployment".into(), Some("default".into()), "web".into(), 3)
    });
    assert!(html.contains("Scale workload"), "got: {html}");
    assert!(html.contains("Deployment · default/web"), "got: {html}");
    assert!(html.contains("Current: 3"), "got: {html}");
    assert!(html.contains("btn btn-primary"), "got: {html}");
}
