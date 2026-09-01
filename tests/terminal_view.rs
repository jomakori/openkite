//! Integration tests for the terminal view's pure-logic helpers.
//!
//! No Dioxus runtime, no kube client — these pin the state-machine and
//! selector helper shapes the `#[component]` bodies consume (per the
//! openkite-dev skill: components are smoke-only-testable locally).

use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

use openkite::views::terminal::{default_container, parse_pod_name, phase_label, TerminalPhase};

fn pod_named(name: &str) -> Pod {
    Pod {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn phase_label_disconnected() {
    assert_eq!(phase_label(&TerminalPhase::Disconnected), "Disconnected");
}

#[test]
fn phase_label_connecting() {
    assert_eq!(phase_label(&TerminalPhase::Connecting), "Connecting…");
}

#[test]
fn phase_label_connected() {
    assert_eq!(phase_label(&TerminalPhase::Connected), "Connected");
}

#[test]
fn phase_label_bridge_pending() {
    assert_eq!(
        phase_label(&TerminalPhase::BridgePending),
        "Bridge pending Phase 1"
    );
}

#[test]
fn phase_label_error() {
    // The message is rendered separately in the toolbar, not in the label.
    assert_eq!(phase_label(&TerminalPhase::Error("x".into())), "Error");
}

#[test]
fn parse_pod_name_returns_name_for_named_pod() {
    let pod = pod_named("web-1");
    assert_eq!(parse_pod_name(&Some(pod)), Some("web-1".to_string()));
}

#[test]
fn parse_pod_name_returns_none_for_unnamed_pod() {
    assert_eq!(parse_pod_name(&None), None);
    assert_eq!(parse_pod_name(&Some(Pod::default())), None);
}

#[test]
fn default_container_matches_pick_default_container_semantics() {
    assert_eq!(
        default_container(&["a".into(), "b".into(), "c".into()]),
        Some("a".to_string())
    );
    assert_eq!(
        default_container(&["".into(), "a".into()]),
        Some("a".to_string())
    );
    assert_eq!(default_container(&[]), None);
    assert_eq!(default_container(&["".into()]), None);
}
