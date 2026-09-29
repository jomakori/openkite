//! Headless mounts of the standalone log viewer (OKT-136 surface migration).
//!
//! Mirrors `support::mount_html` from the existing surface: build a
//! throwaway `VirtualDom`, seed `SELECTED_POD` inside the vdom runtime,
//! rebuild in place, and snapshot the rendered HTML. The container, follow
//! and paused-hint helpers stay as pure-logic helpers in `openkite_api::pod`;
//! the tests for those live alongside them so a future wasm build only has
//! to type-check one definition site.

mod support;

use dioxus::prelude::*;
use openkite_api::pod::{ContainerInfo, PodObject, PodSummary};
use openkite_ui::components::logs::LogsView;

fn log_pod() -> PodObject {
    PodObject {
        name: "web-1".into(),
        namespace: Some("default".into()),
        summary: PodSummary::default(),
        containers: vec![
            ContainerInfo {
                name: "web".into(),
                image: "nginx".into(),
                ready: true,
                restarts: 0,
                state: "Running".into(),
            },
            ContainerInfo {
                name: "sidecar".into(),
                image: "envoy".into(),
                ready: false,
                restarts: 3,
                state: "Waiting".into(),
            },
        ],
        labels: Default::default(),
        annotations: Default::default(),
        yaml: String::new(),
    }
}

fn empty_app() -> Element {
    rsx! { LogsView {} }
}

fn app_with_anchor() -> Element {
    rsx! {
        LogsView {}
        div { "shell-anchor" }
    }
}

#[test]
fn no_selected_pod_renders_pod_prompt() {
    let html = support::mount_html(empty_app, || {});
    assert!(html.contains("log-panel"), "got: {html}");
    assert!(
        html.contains("Select a pod to view its logs"),
        "got: {html}"
    );
}

#[test]
fn selected_pod_renders_picker_follow_clear_and_inspector_hand_off() {
    let html = support::mount_html(empty_app, || {
        *openkite_ui::runtime::SELECTED_POD.write() = Some(log_pod());
    });
    for want in [">web<", ">sidecar<", "Follow", "Clear", "Open in inspector"] {
        assert!(html.contains(want), "missing {want:?}: {html}");
    }
}

#[test]
fn container_picker_default_is_first_non_empty() {
    let html = support::mount_html(empty_app, || {
        *openkite_ui::runtime::SELECTED_POD.write() = Some(log_pod());
    });
    assert!(
        html.contains("value=\"web\""),
        "select value should default to the first container, got: {html}"
    );
}

#[test]
fn seeded_renders_alongside_sibling_anchor() {
    let html = support::mount_html(app_with_anchor, || {
        *openkite_ui::runtime::SELECTED_POD.write() = Some(log_pod());
    });
    assert!(html.contains("shell-anchor"), "got: {html}");
    assert!(html.contains("log-panel"), "got: {html}");
    assert!(html.contains(">web<"), "got: {html}");
}
