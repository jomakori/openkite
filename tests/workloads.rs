//! Integration tests for the workload row mapping.

use k8s_openapi::api::apps::v1::{
    DaemonSet, DaemonSetSpec, DaemonSetStatus, Deployment, DeploymentSpec, DeploymentStatus,
    ReplicaSet, ReplicaSetSpec, ReplicaSetStatus, StatefulSet, StatefulSetSpec, StatefulSetStatus,
};
use k8s_openapi::api::batch::v1::{CronJob, CronJobSpec, CronJobStatus, Job, JobSpec, JobStatus};
use k8s_openapi::api::core::v1::{ContainerStatus, Pod, PodStatus};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference, Time};

use openkite::components::resource_table::{CellExtras, HealthDot};
use openkite::components::status_badge::StatusKind;
use openkite::workloads::{
    age_cell, controller_for_pod, cron_job_columns, cron_job_row, daemon_set_columns,
    daemon_set_row, deployment_columns, deployment_row, job_columns, job_row, pod_columns,
    pod_health_dots, pod_row, replica_set_columns, replica_set_row, stateful_set_columns,
    stateful_set_row, WorkloadKind,
};

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
            restart_count: if i == 0 { restarts } else { 0 },
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
fn pod_row_emits_nine_cells_with_mockup_layout() {
    let row = pod_row(&pod("web", "default", "Running", 2, 3, 4));
    assert_eq!(row.id, "default/web");
    assert_eq!(row.namespace.as_deref(), Some("default"));
    assert_eq!(row.cells.len(), 9);
    assert_eq!(row.cells[0].text, "web");
    // Health dots: two of three containers ready.
    assert_eq!(row.cells[1].text, "2/3");
    assert!(matches!(row.cells[1].extras, CellExtras::HealthDots(_)));
    // Ready, Restarts.
    assert_eq!(row.cells[2].text, "2/3");
    assert_eq!(row.cells[3].text, "4");
    // Bare pod: no owner, node, or QoS yet → dashes.
    assert_eq!(row.cells[4].text, "-");
    assert_eq!(row.cells[5].text, "-");
    assert_eq!(row.cells[6].text, "-");
    // Age second-to-last (no creation timestamp → dash), Status last.
    assert_eq!(row.cells[7].text, "-");
    assert_eq!(row.cells[8].status, Some(StatusKind::Running));
}

#[test]
fn pod_row_failed_phase_maps_to_failed_status() {
    let row = pod_row(&pod("crash", "default", "Failed", 0, 1, 0));
    assert_eq!(row.cells[8].status, Some(StatusKind::Failed));
}

#[test]
fn pod_health_dots_count_ok_versus_failed() {
    let mut p = pod("web", "default", "Running", 2, 2, 0);
    if let Some(status) = p.status.as_mut() {
        status.container_statuses = Some(vec![
            ContainerStatus {
                name: "c0".into(),
                ready: true,
                ..Default::default()
            },
            ContainerStatus {
                name: "c1".into(),
                ready: false,
                ..Default::default()
            },
        ]);
    }
    let cell = pod_health_dots(&p);
    assert_eq!(cell.text, "1/2");
    match cell.extras {
        CellExtras::HealthDots(dots) => {
            assert_eq!(dots.len(), 2);
            assert_eq!(dots[0], HealthDot::Ok);
            assert_eq!(dots[1], HealthDot::Err);
        }
        CellExtras::Plain => panic!("health dots must use the rich extras payload"),
    }
}

#[test]
fn pod_health_dots_empty_when_no_container_status() {
    let p = Pod {
        metadata: meta("pending", "default"),
        status: Some(PodStatus {
            phase: Some("Pending".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let cell = pod_health_dots(&p);
    assert_eq!(cell.text, "—");
    match cell.extras {
        CellExtras::HealthDots(dots) => assert!(dots.is_empty()),
        CellExtras::Plain => panic!("missing statuses must still carry the dots payload"),
    }
}

#[test]
fn controller_for_pod_extracts_owner_reference() {
    let mut p = pod("web", "default", "Running", 1, 1, 0);
    p.metadata.owner_references = Some(vec![OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "ReplicaSet".to_string(),
        name: "web-abc".to_string(),
        uid: "00000000-0000-0000-0000-000000000000".to_string(),
        ..Default::default()
    }]);
    assert_eq!(controller_for_pod(&p), "ReplicaSet/web-abc");
}

#[test]
fn controller_for_pod_with_no_owner_returns_dash() {
    let p = pod("standalone", "default", "Running", 1, 1, 0);
    assert_eq!(controller_for_pod(&p), "-");
}

#[test]
fn age_cell_returns_dash_for_none_timestamp() {
    let cell = age_cell(&None);
    assert_eq!(cell.text, "-");
    assert!(matches!(
        cell.sort,
        openkite::components::resource_table::SortKey::Number(_)
    ));
}

#[test]
fn age_cell_formats_seconds_minutes_hours_days() {
    let now = k8s_openapi::jiff::Timestamp::now();
    let cases = [
        (0u64, "0s"),
        (30, "30s"),
        (5 * 60, "5m"),
        (3 * 60 * 60, "3h"),
        (2 * 60 * 60 * 24, "2d"),
    ];
    for (offset, expected) in cases {
        let ts =
            k8s_openapi::jiff::Timestamp::from_second(now.as_second() - offset as i64).unwrap();
        let cell = age_cell(&Some(Time(ts)));
        assert_eq!(cell.text, expected, "offset {offset}s");
    }
}

#[test]
fn deployment_row_includes_up_to_date_and_controller() {
    let d = Deployment {
        metadata: meta("api", "default"),
        spec: Some(DeploymentSpec {
            replicas: Some(3),
            ..Default::default()
        }),
        status: Some(DeploymentStatus {
            ready_replicas: Some(3),
            updated_replicas: Some(3),
            available_replicas: Some(2),
            ..Default::default()
        }),
    };
    let row = deployment_row(&d);
    assert_eq!(row.cells.len(), 7);
    assert_eq!(row.cells[1].text, "3/3");
    assert_eq!(row.cells[2].text, "3");
    assert_eq!(row.cells[3].text, "2");
    assert_eq!(row.cells[4].text, "-");
    assert_eq!(row.cells[6].status, Some(StatusKind::Ready));
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
    };
    let row = deployment_row(&d);
    assert_eq!(row.cells[1].text, "3/3");
    assert_eq!(row.cells[6].status, Some(StatusKind::Ready));
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
    };
    let row = deployment_row(&d);
    assert_eq!(row.cells[1].text, "1/3");
    assert_eq!(row.cells[6].status, Some(StatusKind::Degraded));
}

#[test]
fn stateful_set_row_includes_up_to_date_and_age() {
    let s = StatefulSet {
        metadata: meta("db", "default"),
        spec: Some(StatefulSetSpec {
            replicas: Some(3),
            ..Default::default()
        }),
        status: Some(StatefulSetStatus {
            ready_replicas: Some(2),
            updated_replicas: Some(2),
            ..Default::default()
        }),
    };
    let row = stateful_set_row(&s);
    assert_eq!(row.cells.len(), 5);
    assert_eq!(row.cells[1].text, "2/3");
    assert_eq!(row.cells[2].text, "2");
    assert_eq!(row.cells[3].text, "-");
    assert_eq!(row.cells[4].status, Some(StatusKind::Degraded));
}

#[test]
fn daemon_set_row_includes_desired_current_available_age() {
    let ds = DaemonSet {
        metadata: meta("agent", "default"),
        spec: Some(DaemonSetSpec::default()),
        status: Some(DaemonSetStatus {
            desired_number_scheduled: 3,
            number_ready: 2,
            current_number_scheduled: 3,
            number_available: Some(1),
            ..Default::default()
        }),
    };
    let row = daemon_set_row(&ds);
    assert_eq!(row.cells.len(), 7);
    assert_eq!(row.cells[1].text, "2/3");
    assert_eq!(row.cells[2].text, "3");
    assert_eq!(row.cells[3].text, "3");
    assert_eq!(row.cells[4].text, "1");
    assert_eq!(row.cells[5].text, "-");
    assert_eq!(row.cells[6].status, Some(StatusKind::Degraded));
}

#[test]
fn replica_set_row_includes_desired_and_age() {
    let rs = ReplicaSet {
        metadata: meta("web-abc", "default"),
        spec: Some(ReplicaSetSpec {
            replicas: Some(3),
            ..Default::default()
        }),
        status: Some(ReplicaSetStatus {
            ready_replicas: Some(2),
            ..Default::default()
        }),
    };
    let row = replica_set_row(&rs);
    assert_eq!(row.cells.len(), 5);
    assert_eq!(row.cells[1].text, "2");
    assert_eq!(row.cells[2].text, "3");
    assert_eq!(row.cells[3].text, "-");
    assert_eq!(row.cells[4].status, Some(StatusKind::Degraded));
}

#[test]
fn job_row_includes_duration_and_age() {
    let now = k8s_openapi::jiff::Timestamp::now();
    let start = k8s_openapi::jiff::Timestamp::from_second(now.as_second() - 3700).unwrap();
    let end = k8s_openapi::jiff::Timestamp::from_second(now.as_second() - 60).unwrap();
    let job = Job {
        metadata: meta("migrate", "default"),
        spec: Some(JobSpec {
            completions: Some(1),
            ..Default::default()
        }),
        status: Some(JobStatus {
            succeeded: Some(1),
            start_time: Some(Time(start)),
            completion_time: Some(Time(end)),
            ..Default::default()
        }),
    };
    let row = job_row(&job);
    assert_eq!(row.cells.len(), 5);
    assert_eq!(row.cells[1].text, "1/1");
    assert_eq!(row.cells[2].text, "1h");
    assert_eq!(row.cells[3].text, "-");
    assert_eq!(row.cells[4].status, Some(StatusKind::Succeeded));
}

#[test]
fn cron_job_row_includes_last_schedule_and_age() {
    let now = k8s_openapi::jiff::Timestamp::now();
    let last = k8s_openapi::jiff::Timestamp::from_second(now.as_second() - 120).unwrap();
    let cj = CronJob {
        metadata: meta("nightly", "default"),
        spec: CronJobSpec {
            schedule: "*/5 * * * *".to_string(),
            suspend: Some(false),
            ..Default::default()
        },
        status: Some(CronJobStatus {
            last_schedule_time: Some(Time(last)),
            ..Default::default()
        }),
    };
    let row = cron_job_row(&cj);
    assert_eq!(row.cells.len(), 6);
    assert_eq!(row.cells[1].text, "*/5 * * * *");
    assert_eq!(row.cells[2].text, "No");
    assert_eq!(row.cells[3].text, "2m");
    assert_eq!(row.cells[4].text, "-");
    assert_eq!(row.cells[5].status, Some(StatusKind::Running));
}

#[test]
fn columns_match_row_layouts() {
    assert_eq!(pod_columns().len(), 9);
    assert_eq!(deployment_columns().len(), 7);
    assert_eq!(stateful_set_columns().len(), 5);
    assert_eq!(daemon_set_columns().len(), 7);
    assert_eq!(replica_set_columns().len(), 5);
    assert_eq!(job_columns().len(), 5);
    assert_eq!(cron_job_columns().len(), 6);
}

#[test]
fn workload_kind_labels_match_mockup() {
    assert_eq!(WorkloadKind::ALL.len(), 8);
    let labels = WorkloadKind::ALL.map(|kind| kind.label());
    assert_eq!(
        labels,
        [
            "Pods",
            "Nodes",
            "Deployments",
            "StatefulSets",
            "DaemonSets",
            "ReplicaSets",
            "Jobs",
            "CronJobs",
        ]
    );
}
