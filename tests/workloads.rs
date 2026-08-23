//! Integration tests for the workload row mapping.

use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec, DeploymentStatus};
use k8s_openapi::api::core::v1::{ContainerStatus, Pod, PodStatus};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

use openkite::components::status_badge::StatusKind;
use openkite::workloads::{deployment_row, pod_row, WorkloadKind};

fn meta(name: &str, namespace: &str) -> ObjectMeta {
    ObjectMeta {
        name: Some(name.to_string()),
        namespace: Some(namespace.to_string()),
        ..Default::default()
    }
}

fn pod(name: &str, namespace: &str, phase: &str, ready: usize, total: usize, restarts: i32) -> Pod {
    let containers = (0..total)
        .map(|i| ContainerStatus {
            name: format!("c{i}"),
            ready: i < ready,
            restart_count: restarts,
            ..Default::default()
        })
        .collect();
    Pod {
        metadata: meta(name, namespace),
        status: Some(PodStatus {
            phase: Some(phase.to_string()),
            container_statuses: Some(containers),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn pod_row_maps_name_status_ready_and_restarts() {
    let row = pod_row(&pod("web", "default", "Running", 2, 3, 4));
    assert_eq!(row.id, "default/web");
    assert_eq!(row.namespace.as_deref(), Some("default"));
    assert_eq!(row.cells.len(), 4);
    assert_eq!(row.cells[0].text, "web");
    assert_eq!(row.cells[1].status, Some(StatusKind::Running));
    assert_eq!(row.cells[2].text, "2/3");
    assert_eq!(row.cells[3].text, "4");
}

#[test]
fn pod_row_failed_phase_maps_to_failed_status() {
    let row = pod_row(&pod("crash", "default", "Failed", 0, 1, 0));
    assert_eq!(row.cells[1].status, Some(StatusKind::Failed));
}

#[test]
fn deployment_row_ready_replicas_maps_to_ready_status() {
    let d = Deployment {
        metadata: meta("api", "default"),
        spec: Some(DeploymentSpec {
            replicas: Some(3),
            ..Default::default()
        }),
        status: Some(DeploymentStatus {
            ready_replicas: Some(3),
            ..Default::default()
        }),
        ..Default::default()
    };
    let row = deployment_row(&d);
    assert_eq!(row.cells[1].status, Some(StatusKind::Ready));
    assert_eq!(row.cells[2].text, "3/3");
}

#[test]
fn deployment_row_partial_replicas_maps_to_degraded_status() {
    let d = Deployment {
        metadata: meta("api", "default"),
        spec: Some(DeploymentSpec {
            replicas: Some(3),
            ..Default::default()
        }),
        status: Some(DeploymentStatus {
            ready_replicas: Some(1),
            ..Default::default()
        }),
        ..Default::default()
    };
    let row = deployment_row(&d);
    assert_eq!(row.cells[1].status, Some(StatusKind::Degraded));
    assert_eq!(row.cells[2].text, "1/3");
}

#[test]
fn workload_kind_lists_seven_kinds_with_labels() {
    assert_eq!(WorkloadKind::ALL.len(), 7);
    assert_eq!(WorkloadKind::Pods.label(), "Pods");
    assert_eq!(WorkloadKind::CronJobs.label(), "CronJobs");
}
