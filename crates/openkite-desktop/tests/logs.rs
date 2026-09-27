//! Integration tests for the log streaming module.

use openkite::logs::{FollowState, LineBuffer, LogOptions, MAX_LINES};

#[test]
fn line_buffer_appends_and_reads_back() {
    let mut buf = LineBuffer::default();
    buf.push("first");
    buf.push("second");
    assert_eq!(buf.len(), 2);
    assert_eq!(buf.lines(), &["first".to_string(), "second".to_string()]);
}

#[test]
fn line_buffer_caps_and_drops_oldest() {
    let mut buf = LineBuffer::default();
    for i in 0..(MAX_LINES + 5) {
        buf.push(format!("line {i}"));
    }
    assert_eq!(buf.len(), MAX_LINES);
    assert_eq!(buf.lines()[0], "line 5");
    assert_eq!(
        buf.lines()[MAX_LINES - 1],
        format!("line {}", MAX_LINES + 4)
    );
}

#[test]
fn line_buffer_clear_empties() {
    let mut buf = LineBuffer::default();
    buf.push("a");
    buf.push("b");
    buf.clear();
    assert!(buf.is_empty());
}

#[test]
fn follow_state_pause_resume() {
    let mut state = FollowState::Following;
    assert!(state.is_following());
    state.pause();
    assert_eq!(state, FollowState::Paused);
    assert!(!state.is_following());
    state.resume();
    assert!(state.is_following());
}

#[test]
fn log_options_map_to_kube_params() {
    let opts = LogOptions {
        container: Some("sidecar".to_string()),
        follow: true,
        tail_lines: Some(100),
        timestamps: true,
    };
    let params = opts.to_params();
    assert_eq!(params.container.as_deref(), Some("sidecar"));
    assert!(params.follow);
    assert_eq!(params.tail_lines, Some(100));
    assert!(params.timestamps);
}

// ─────────────────────────────────────────────────────────────
// Logs view (headless mount).
// ─────────────────────────────────────────────────────────────

use dioxus::prelude::*;
use k8s_openapi::api::core::v1::{Container, Pod, PodSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use openkite::views::logs::LogsView;

fn log_pod() -> Pod {
    Pod {
        metadata: ObjectMeta {
            name: Some("web-1".into()),
            namespace: Some("default".into()),
            ..Default::default()
        },
        spec: Some(PodSpec {
            containers: vec![
                Container {
                    name: "web".into(),
                    image: Some("nginx".into()),
                    ..Default::default()
                },
                Container {
                    name: "sidecar".into(),
                    image: Some("envoy".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn mount_logs(seed: impl FnOnce()) -> String {
    let mut vdom = VirtualDom::new(LogsView);
    vdom.in_runtime(seed);
    vdom.rebuild_in_place();
    dioxus_ssr::Renderer::new().render(&vdom)
}

#[test]
fn logs_view_empty_state_without_selected_pod() {
    let html = mount_logs(|| {});
    assert!(
        html.contains("Select a pod to view its logs"),
        "got: {html}"
    );
}

#[test]
fn logs_view_with_pod_renders_picker_and_toolbar() {
    let pod = log_pod();
    let html = mount_logs(move || {
        *openkite::runtime::SELECTED_POD.write() = Some(pod);
    });
    assert!(html.contains(">web<"), "got: {html}");
    assert!(html.contains(">sidecar<"), "got: {html}");
    assert!(html.contains("Follow"), "got: {html}");
    assert!(html.contains("Clear"), "got: {html}");
    assert!(html.contains("Open in inspector"), "got: {html}");
    assert!(
        html.contains("Select a container to view logs."),
        "got: {html}"
    );
}
