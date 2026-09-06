//! Integration tests for the pod-detail data layer (`src/pod.rs`).
//!
//! `src/views/pod_detail.rs` is a thin Dioxus shell over these mappers, so
//! the coverage lives here: container state labels (all branches), the full
//! `PodSummary` field surface, and pod-level edge cases (missing spec).

use k8s_openapi::api::core::v1::{
    Container, ContainerState, ContainerStateRunning, ContainerStateTerminated,
    ContainerStateWaiting, ContainerStatus, Pod, PodSpec, PodStatus,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

use openkite::pod::{container_infos, pod_summary};

fn running() -> ContainerState {
    ContainerState {
        running: Some(ContainerStateRunning::default()),
        ..Default::default()
    }
}

fn waiting(reason: Option<&str>) -> ContainerState {
    ContainerState {
        waiting: Some(ContainerStateWaiting {
            reason: reason.map(str::to_string),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn terminated(reason: Option<&str>, exit_code: i32) -> ContainerState {
    ContainerState {
        terminated: Some(ContainerStateTerminated {
            reason: reason.map(str::to_string),
            exit_code,
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn pod_with_container_state(state: Option<ContainerState>) -> Pod {
    Pod {
        metadata: ObjectMeta::default(),
        spec: Some(PodSpec {
            containers: vec![Container {
                name: "app".into(),
                image: Some("nginx:1.25".into()),
                ..Default::default()
            }],
            node_name: Some("node-1".into()),
            ..Default::default()
        }),
        status: Some(PodStatus {
            phase: Some("Running".into()),
            container_statuses: Some(vec![ContainerStatus {
                name: "app".into(),
                image: "nginx:1.25".into(),
                ready: true,
                restart_count: 1,
                state,
                ..Default::default()
            }]),
            ..Default::default()
        }),
    }
}

#[test]
fn running_state_label() {
    let infos = container_infos(&pod_with_container_state(Some(running())));
    assert_eq!(infos[0].state, "Running");
    assert!(infos[0].ready);
}

#[test]
fn waiting_with_reason_label() {
    let infos = container_infos(&pod_with_container_state(Some(waiting(Some(
        "CrashLoopBackOff",
    )))));
    assert_eq!(infos[0].state, "Waiting: CrashLoopBackOff");
    assert!(!infos[0].ready);
}

#[test]
fn waiting_without_reason_label() {
    let infos = container_infos(&pod_with_container_state(Some(waiting(None))));
    assert_eq!(infos[0].state, "Waiting");
}

#[test]
fn terminated_with_reason_label() {
    let infos = container_infos(&pod_with_container_state(Some(terminated(
        Some("OOMKilled"),
        137,
    ))));
    assert_eq!(infos[0].state, "Terminated: OOMKilled");
    assert!(!infos[0].ready);
}

#[test]
fn terminated_without_reason_uses_exit_code() {
    let infos = container_infos(&pod_with_container_state(Some(terminated(None, 1))));
    assert_eq!(infos[0].state, "Terminated (exit 1)");
}

#[test]
fn empty_state_object_is_unknown() {
    let infos = container_infos(&pod_with_container_state(Some(ContainerState::default())));
    assert_eq!(infos[0].state, "Unknown");
}

#[test]
fn pod_summary_carries_ip_qos_reason_and_message() {
    let mut p = pod_with_container_state(Some(running()));
    let status = p.status.as_mut().unwrap();
    status.pod_ip = Some("10.0.0.7".into());
    status.qos_class = Some("Guaranteed".into());
    status.reason = Some("Evicted".into());
    status.message = Some("The node was low on resource: memory".into());

    let s = pod_summary(&p);
    assert_eq!(s.phase, "Running");
    assert_eq!(s.node, "node-1");
    assert_eq!(s.pod_ip, "10.0.0.7");
    assert_eq!(s.qos, "Guaranteed");
    assert_eq!(s.reason.as_deref(), Some("Evicted"));
    assert_eq!(
        s.message.as_deref(),
        Some("The node was low on resource: memory")
    );
}

#[test]
fn pod_summary_handles_missing_spec_and_status() {
    let p = Pod {
        metadata: ObjectMeta::default(),
        spec: None,
        status: None,
    };
    let s = pod_summary(&p);
    assert_eq!(s.phase, "Unknown");
    assert_eq!(s.node, "");
    assert_eq!(s.pod_ip, "");
    assert_eq!(s.qos, "");
    assert_eq!(s.reason, None);
    assert_eq!(s.message, None);

    // Missing spec + missing statuses → one Pending row from no containers.
    let infos = container_infos(&p);
    assert!(infos.is_empty());
}

#[test]
fn container_without_status_defaults_to_pending_row() {
    let p = Pod {
        metadata: ObjectMeta::default(),
        spec: Some(PodSpec {
            containers: vec![Container {
                name: "sidecar".into(),
                image: None,
                ..Default::default()
            }],
            ..Default::default()
        }),
        status: None,
    };
    let infos = container_infos(&p);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].name, "sidecar");
    assert_eq!(infos[0].image, "");
    assert_eq!(infos[0].state, "Pending");
    assert_eq!(infos[0].restarts, 0);
    assert!(!infos[0].ready);
}
