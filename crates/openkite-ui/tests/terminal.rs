//! Headless mounts of the standalone terminal surface.
//!
//! The xterm.js instance itself belongs to the host (the mount JS in
//! `components::terminal` attaches to the `data-term-host` div after the
//! bundle evals), so no SSR pass can render a live terminal. What these tests
//! pin is the chrome around that mount point — the toolbar, the phase label
//! and the container picker — plus the gate a host without a terminal
//! capability renders instead of a toolbar with no host behind it.

mod support;

use dioxus::prelude::*;
use openkite_api::capability::Capabilities;
use openkite_api::pod::{ContainerInfo, PodObject, PodSummary};
use openkite_ui::components::terminal::TerminalView;

// The runtime slots are process-global; hold this guard in every test that
// mutates them so parallel test threads cannot gate on each other's state.
static RUNTIME_SLOTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn gate_lock() -> std::sync::MutexGuard<'static, ()> {
    RUNTIME_SLOTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn term_pod() -> PodObject {
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

fn terminal() -> Element {
    rsx! { TerminalView {} }
}

fn mount_with(capabilities: Capabilities, seed: impl FnOnce()) -> String {
    let _gate = gate_lock();
    let html = support::mount_html(terminal, move || {
        openkite_ui::runtime::set_published_capabilities(Some(capabilities));
        seed();
    });
    openkite_ui::runtime::set_published_capabilities(None);
    *openkite_ui::runtime::SELECTED_POD.write() = None;
    html
}

#[test]
fn host_without_the_capability_renders_the_unsupported_chrome() {
    let html = mount_with(Capabilities::server_side(), || {});
    assert!(html.contains("Terminal not available"), "got: {html}");
    assert!(
        html.contains("does not advertise the terminal surface"),
        "got: {html}"
    );
    assert!(
        !html.contains("data-term-host"),
        "no mount point without the capability: {html}"
    );
}

#[test]
fn missing_capability_descriptor_gates_the_surface_too() {
    let _gate = gate_lock();
    openkite_ui::runtime::set_published_capabilities(None);
    let html = support::mount_html(terminal, || {});
    *openkite_ui::runtime::SELECTED_POD.write() = None;
    assert!(html.contains("Terminal not available"), "got: {html}");
    assert!(!html.contains("data-term-host"), "got: {html}");
}

#[test]
fn no_selected_pod_renders_the_prompt_and_the_mount_point() {
    let html = mount_with(Capabilities::in_process(), || {});
    for want in [
        "Pick a pod to start a terminal session",
        "(none — open from inspector)",
        "Disconnected",
        "term-status",
        "Reconnect",
        "Disconnect",
    ] {
        assert!(html.contains(want), "missing {want:?}: {html}");
    }
    assert!(
        html.contains("data-term-host=\"xterm-bundle-v1-"),
        "the sidecar's mount point must carry the bundle's cache-buster id: {html}"
    );
}

#[test]
fn selected_pod_renders_the_picker_and_drops_the_empty_state() {
    let html = mount_with(Capabilities::in_process(), || {
        *openkite_ui::runtime::SELECTED_POD.write() = Some(term_pod());
    });
    for want in ["pod: web-1", ">web<", ">sidecar<", "value=\"web\""] {
        assert!(html.contains(want), "missing {want:?}: {html}");
    }
    assert!(
        !html.contains("Pick a pod to start a terminal session"),
        "got: {html}"
    );
}
