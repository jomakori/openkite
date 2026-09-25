//! Integration tests for the component layer: headless SSR renders of the
//! hook-free primitives plus the pure logic helpers the `#[component]`
//! bodies call.
//!
//! SSR renders pin the emitted HTML/CSS contract (`dioxus_ssr::render_element`).
//! Components that own signals or effects (tables, editors, overlays) are not
//! SSR-rendered here — their stateful logic lives in dedicated test files
//! (`tests/resource_table.rs`, `tests/crud.rs`).

use dioxus::prelude::rsx;
use dioxus_ssr::render_element;
use openkite::components::code_editor::{code_editor_path, compute_diagnostics, YamlDiagnostics};
use openkite::components::status_badge::{StatusBadge, StatusKind, StatusPill};

#[test]
fn status_badge_renders_class_and_label() {
    let html = render_element(rsx! { StatusBadge { status: StatusKind::Running } });
    assert!(html.contains("status-badge status-ok"), "got: {html}");
    assert!(html.contains(">Running<"), "got: {html}");
}

#[test]
fn status_badge_error_status_renders_error_class() {
    let html = render_element(rsx! { StatusBadge { status: StatusKind::CrashLoop } });
    assert!(html.contains("status-badge status-err"), "got: {html}");
    assert!(html.contains(">CrashLoop<"), "got: {html}");
}

#[test]
fn status_badge_unknown_renders_muted_class() {
    let html = render_element(rsx! { StatusBadge { status: StatusKind::Unknown } });
    assert!(html.contains("status-badge status-muted"), "got: {html}");
}

#[test]
fn status_badge_succeeded_is_ok_class() {
    let html = render_element(rsx! { StatusBadge { status: StatusKind::Succeeded } });
    assert!(html.contains("status-badge status-ok"), "got: {html}");
    assert!(html.contains(">Succeeded<"), "got: {html}");
}

#[test]
fn every_status_kind_renders_without_panicking() {
    for kind in [
        StatusKind::Running,
        StatusKind::Ready,
        StatusKind::Pending,
        StatusKind::Succeeded,
        StatusKind::Failed,
        StatusKind::CrashLoop,
        StatusKind::Unknown,
        StatusKind::OutOfSync,
        StatusKind::Degraded,
        StatusKind::Suspended,
    ] {
        let html = render_element(rsx! { StatusBadge { status: kind } });
        assert!(!html.is_empty());
    }
}

#[test]
fn code_editor_compute_diagnostics_clean_document() {
    assert!(compute_diagnostics("apiVersion: v1\nkind: ConfigMap\n").is_empty());
    assert!(compute_diagnostics("").is_empty());
}

#[test]
fn code_editor_compute_diagnostics_reports_invalid_yaml() {
    let diags = compute_diagnostics("kind: [unclosed");
    assert_eq!(diags.len(), 1);
    assert!(!diags[0].message.is_empty());
    // Diagnostics carry a 1-based position.
    assert!(diags[0].line >= 1);
}

#[test]
fn code_editor_path_is_stable_cache_buster() {
    assert_eq!(code_editor_path(), "cm-bundle-v1");
}

#[test]
fn status_pill_renders_design_system_variant() {
    let danger = render_element(rsx! { StatusPill { status: StatusKind::Degraded } });
    assert!(danger.contains("pill danger"), "got: {danger}");
    assert!(danger.contains(">Degraded<"), "got: {danger}");
    let ok = render_element(rsx! { StatusPill { status: StatusKind::Ready } });
    assert!(ok.contains("pill success"), "got: {ok}");
    let warn = render_element(rsx! { StatusPill { status: StatusKind::OutOfSync } });
    assert!(warn.contains("pill warn"), "got: {warn}");
}

#[test]
fn yaml_diagnostics_renders_one_row_per_diagnostic() {
    let html = render_element(rsx! { YamlDiagnostics {
        diagnostics: vec![(3, 5, "bad yaml".to_string())]
    } });
    assert!(html.contains("diagnostic-line"), "got: {html}");
    assert!(html.contains("3:5"), "got: {html}");
    assert!(html.contains("bad yaml"), "got: {html}");
}
