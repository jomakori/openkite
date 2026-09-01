//! Workload views: browse and operate on all workload kinds (Pods → CronJobs).
//!
//! This module holds the pure mapping — column layouts, row mappers, and status
//! derivation — that the Workloads view feeds into the resource table. Keeping
//! it free of Dioxus makes it testable in isolation.

use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{OwnerReference, Time};

use crate::components::resource_table::{Cell, ColumnDef, HealthDot, ResourceRow};
use crate::components::status_badge::StatusKind;

/// The seven workload kinds the Workloads view lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkloadKind {
    Pods,
    Deployments,
    StatefulSets,
    DaemonSets,
    ReplicaSets,
    Jobs,
    CronJobs,
}

impl WorkloadKind {
    /// Every kind, in tab order.
    pub const ALL: [WorkloadKind; 7] = [
        WorkloadKind::Pods,
        WorkloadKind::Deployments,
        WorkloadKind::StatefulSets,
        WorkloadKind::DaemonSets,
        WorkloadKind::ReplicaSets,
        WorkloadKind::Jobs,
        WorkloadKind::CronJobs,
    ];

    /// Human-readable tab label.
    pub fn label(self) -> &'static str {
        match self {
            WorkloadKind::Pods => "Pods",
            WorkloadKind::Deployments => "Deployments",
            WorkloadKind::StatefulSets => "StatefulSets",
            WorkloadKind::DaemonSets => "DaemonSets",
            WorkloadKind::ReplicaSets => "ReplicaSets",
            WorkloadKind::Jobs => "Jobs",
            WorkloadKind::CronJobs => "CronJobs",
        }
    }

    /// Singular API kind string (matches the `kind` field of a manifest).
    /// Used by the CRUD modal for the starter/Edit doc.
    pub fn kind_str(self) -> &'static str {
        match self {
            WorkloadKind::Pods => "Pod",
            WorkloadKind::Deployments => "Deployment",
            WorkloadKind::StatefulSets => "StatefulSet",
            WorkloadKind::DaemonSets => "DaemonSet",
            WorkloadKind::ReplicaSets => "ReplicaSet",
            WorkloadKind::Jobs => "Job",
            WorkloadKind::CronJobs => "CronJob",
        }
    }

    /// `apiVersion` prefix for the CRUD modal's starter/Edit doc.
    pub fn api_version(self) -> &'static str {
        match self {
            WorkloadKind::Pods => "v1",
            WorkloadKind::Jobs | WorkloadKind::CronJobs => "batch/v1",
            _ => "apps/v1",
        }
    }
}

/// Column skeleton shared by every kind: Name leading, the kind-specific middle
/// columns, Age second-to-last, Status last — the order `kubectl get` uses.
fn kind_columns(middle: &[ColumnDef]) -> Vec<ColumnDef> {
    let mut columns = vec![ColumnDef {
        key: "name",
        label: "Name",
        width: None,
        sortable: true,
    }];
    columns.extend(middle.iter().cloned());
    columns.extend([
        ColumnDef {
            key: "age",
            label: "Age",
            width: Some(80),
            sortable: true,
        },
        ColumnDef {
            key: "status",
            label: "Status",
            width: Some(110),
            sortable: true,
        },
    ]);
    columns
}

/// "ready/total", numerically sorted by the ready count.
fn ready_cell(ready: i32, total: i32) -> Cell {
    Cell::number(format!("{ready}/{total}"), ready as f64)
}

/// Map ready-vs-desired replicas to a semantic status badge.
fn replicas_status(ready: i32, desired: i32) -> (String, StatusKind) {
    match (ready, desired) {
        (_, 0) => ("Pending".to_string(), StatusKind::Pending),
        (r, d) if r == d => ("Ready".to_string(), StatusKind::Ready),
        (0, _) => ("Pending".to_string(), StatusKind::Pending),
        _ => ("Degraded".to_string(), StatusKind::Degraded),
    }
}

/// Namespace-scoped object id (used as the row key and action target).
fn object_id(namespace: Option<&str>, name: &str) -> String {
    match namespace {
        Some(ns) => format!("{ns}/{name}"),
        None => name.to_string(),
    }
}

/// Format a duration in seconds as the smallest non-zero unit: `5s`, `12m`, `3h`, `2d`.
/// Pure helper, exposed for tests and used by `age_cell` and the Job duration column.
fn humanize_duration(seconds: i64) -> String {
    let abs = seconds.unsigned_abs();
    if abs < 60 {
        format!("{seconds}s")
    } else if abs < 60 * 60 {
        format!("{}m", seconds / 60)
    } else if abs < 60 * 60 * 24 {
        format!("{}h", seconds / 3600)
    } else {
        format!("{}d", seconds / 86400)
    }
}

/// Build a relative-age cell ("how long ago") from a timestamp — the Age column
/// of every kind and the CronJob last-schedule column. Missing timestamps render
/// as `"-"` and sort after every real age via an infinite numeric key.
pub fn age_cell(ts: &Option<Time>) -> Cell {
    let Some(ts) = ts else {
        return Cell::number("-", f64::INFINITY);
    };
    let now = k8s_openapi::jiff::Timestamp::now();
    let delta = now.as_second() - ts.0.as_second();
    Cell::number(humanize_duration(delta), delta as f64)
}

/// One health dot per container, green when ready, red when the container
/// is in a non-ready state. Pending pods (no container statuses) produce an
/// empty dot list that renders as an em dash.
pub fn pod_health_dots(pod: &Pod) -> Cell {
    let dots: Vec<HealthDot> = pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
        .map(|list| {
            list.iter()
                .map(|c| {
                    if c.ready {
                        HealthDot::Ok
                    } else {
                        HealthDot::Err
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Cell::health_dots(dots)
}

/// Controller owner as `Kind/name`, or `"-"` when there are no owner
/// references. The API server lists the controlling owner first.
fn format_controller(refs: &[OwnerReference]) -> String {
    match refs.first() {
        Some(owner) => format!("{}/{}", owner.kind, owner.name),
        None => "-".to_string(),
    }
}

/// Pod's controlling workload (`Deployment/checkout-api`-style), or `"-"`.
pub fn controller_for_pod(pod: &Pod) -> String {
    format_controller(pod.metadata.owner_references.as_deref().unwrap_or(&[]))
}

/// Deployment's owner — usually none, hence `-` for top-level deployments.
pub fn controller_for_deployment(d: &Deployment) -> String {
    format_controller(d.metadata.owner_references.as_deref().unwrap_or(&[]))
}

/// Scheduled node, or `"-"` for pending pods without a node assignment.
fn node_cell(pod: &Pod) -> Cell {
    let node = pod
        .spec
        .as_ref()
        .and_then(|spec| spec.node_name.clone())
        .unwrap_or_else(|| "-".to_string());
    Cell::text(node)
}

/// QoS class (`Guaranteed`, `Burstable`, `BestEffort`), or `"-"` when status
/// has not reported it yet. CSS upper-cases the display.
fn qos_cell(pod: &Pod) -> Cell {
    let qos = pod
        .status
        .as_ref()
        .and_then(|s| s.qos_class.clone())
        .unwrap_or_else(|| "-".to_string());
    Cell::text(qos)
}

// --- Pods ---

/// Pod columns: name, health, ready, restarts, controller, node, QoS, age, status.
pub fn pod_columns() -> Vec<ColumnDef> {
    kind_columns(&[
        ColumnDef {
            key: "health",
            label: "Health",
            width: Some(70),
            sortable: true,
        },
        ColumnDef {
            key: "ready",
            label: "Ready",
            width: Some(80),
            sortable: true,
        },
        ColumnDef {
            key: "restarts",
            label: "Restarts",
            width: Some(90),
            sortable: true,
        },
        ColumnDef {
            key: "controller",
            label: "Controller",
            width: Some(220),
            sortable: true,
        },
        ColumnDef {
            key: "node",
            label: "Node",
            width: Some(140),
            sortable: true,
        },
        ColumnDef {
            key: "qos",
            label: "QoS",
            width: Some(110),
            sortable: true,
        },
    ])
}

/// Map a pod to a display row.
pub fn pod_row(pod: &Pod) -> ResourceRow {
    let name = pod.metadata.name.clone().unwrap_or_default();
    let namespace = pod.metadata.namespace.clone();
    let (label, kind) = pod_status(pod);
    let (ready, total) = pod_ready(pod);
    let restarts = pod_restarts(pod);
    ResourceRow {
        id: object_id(namespace.as_deref(), &name),
        namespace,
        cells: vec![
            Cell::text(name),
            pod_health_dots(pod),
            ready_cell(ready, total),
            Cell::number(restarts.to_string(), restarts as f64),
            Cell::text(controller_for_pod(pod)),
            node_cell(pod),
            qos_cell(pod),
            age_cell(&pod.metadata.creation_timestamp),
            Cell::status(&label, kind),
        ],
    }
}

/// Pod lifecycle phase → semantic status.
fn pod_status(pod: &Pod) -> (String, StatusKind) {
    let phase = pod
        .status
        .as_ref()
        .and_then(|s| s.phase.as_deref())
        .unwrap_or("Unknown");
    let kind = match phase {
        "Running" => StatusKind::Running,
        "Pending" => StatusKind::Pending,
        "Succeeded" => StatusKind::Succeeded,
        "Failed" => StatusKind::Failed,
        _ => StatusKind::Unknown,
    };
    (phase.to_string(), kind)
}

/// (ready, total) container counts for the pod's ready column.
fn pod_ready(pod: &Pod) -> (i32, i32) {
    match pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
    {
        Some(list) => {
            let ready = list.iter().filter(|c| c.ready).count() as i32;
            (ready, list.len() as i32)
        }
        None => (0, 0),
    }
}

/// Total restart count across all containers.
fn pod_restarts(pod: &Pod) -> i32 {
    pod.status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
        .map(|list| list.iter().map(|c| c.restart_count).sum())
        .unwrap_or(0)
}

// --- Deployments ---

/// Deployment columns: name, ready, up-to-date, available, controller, age, status.
pub fn deployment_columns() -> Vec<ColumnDef> {
    kind_columns(&[
        ColumnDef {
            key: "ready",
            label: "Ready",
            width: Some(90),
            sortable: true,
        },
        ColumnDef {
            key: "updated",
            label: "Up-to-date",
            width: Some(100),
            sortable: true,
        },
        ColumnDef {
            key: "available",
            label: "Available",
            width: Some(100),
            sortable: true,
        },
        ColumnDef {
            key: "controller",
            label: "Controller",
            width: Some(220),
            sortable: true,
        },
    ])
}

/// Map a deployment to a display row.
pub fn deployment_row(d: &Deployment) -> ResourceRow {
    let name = d.metadata.name.clone().unwrap_or_default();
    let namespace = d.metadata.namespace.clone();
    let desired = d.spec.as_ref().and_then(|s| s.replicas).unwrap_or(0);
    let ready = d
        .status
        .as_ref()
        .and_then(|s| s.ready_replicas)
        .unwrap_or(0);
    let updated = d
        .status
        .as_ref()
        .and_then(|s| s.updated_replicas)
        .unwrap_or(0);
    let available = d
        .status
        .as_ref()
        .and_then(|s| s.available_replicas)
        .unwrap_or(0);
    let (label, kind) = replicas_status(ready, desired);
    ResourceRow {
        id: object_id(namespace.as_deref(), &name),
        namespace,
        cells: vec![
            Cell::text(name),
            ready_cell(ready, desired),
            Cell::number(updated.to_string(), updated as f64),
            Cell::number(available.to_string(), available as f64),
            Cell::text(controller_for_deployment(d)),
            age_cell(&d.metadata.creation_timestamp),
            Cell::status(&label, kind),
        ],
    }
}

// --- StatefulSets ---

/// StatefulSet columns: name, ready, up-to-date, age, status.
pub fn stateful_set_columns() -> Vec<ColumnDef> {
    kind_columns(&[
        ColumnDef {
            key: "ready",
            label: "Ready",
            width: Some(90),
            sortable: true,
        },
        ColumnDef {
            key: "updated",
            label: "Up-to-date",
            width: Some(100),
            sortable: true,
        },
    ])
}

/// Map a stateful set to a display row.
pub fn stateful_set_row(s: &StatefulSet) -> ResourceRow {
    let name = s.metadata.name.clone().unwrap_or_default();
    let namespace = s.metadata.namespace.clone();
    let desired = s.spec.as_ref().and_then(|sp| sp.replicas).unwrap_or(0);
    let ready = s
        .status
        .as_ref()
        .and_then(|st| st.ready_replicas)
        .unwrap_or(0);
    let updated = s
        .status
        .as_ref()
        .and_then(|st| st.updated_replicas)
        .unwrap_or(0);
    let (label, kind) = replicas_status(ready, desired);
    ResourceRow {
        id: object_id(namespace.as_deref(), &name),
        namespace,
        cells: vec![
            Cell::text(name),
            ready_cell(ready, desired),
            Cell::number(updated.to_string(), updated as f64),
            age_cell(&s.metadata.creation_timestamp),
            Cell::status(&label, kind),
        ],
    }
}

// --- DaemonSets ---

/// DaemonSet columns: name, ready, desired, current, available, age, status.
pub fn daemon_set_columns() -> Vec<ColumnDef> {
    kind_columns(&[
        ColumnDef {
            key: "ready",
            label: "Ready",
            width: Some(90),
            sortable: true,
        },
        ColumnDef {
            key: "desired",
            label: "Desired",
            width: Some(90),
            sortable: true,
        },
        ColumnDef {
            key: "current",
            label: "Current",
            width: Some(90),
            sortable: true,
        },
        ColumnDef {
            key: "available",
            label: "Available",
            width: Some(100),
            sortable: true,
        },
    ])
}

/// Map a daemon set to a display row.
pub fn daemon_set_row(ds: &DaemonSet) -> ResourceRow {
    let name = ds.metadata.name.clone().unwrap_or_default();
    let namespace = ds.metadata.namespace.clone();
    let desired = ds
        .status
        .as_ref()
        .map(|s| s.desired_number_scheduled)
        .unwrap_or(0);
    let ready = ds.status.as_ref().map(|s| s.number_ready).unwrap_or(0);
    let current = ds
        .status
        .as_ref()
        .map(|s| s.current_number_scheduled)
        .unwrap_or(0);
    let available = ds
        .status
        .as_ref()
        .and_then(|s| s.number_available)
        .unwrap_or(0);
    let (label, kind) = replicas_status(ready, desired);
    ResourceRow {
        id: object_id(namespace.as_deref(), &name),
        namespace,
        cells: vec![
            Cell::text(name),
            ready_cell(ready, desired),
            Cell::number(desired.to_string(), desired as f64),
            Cell::number(current.to_string(), current as f64),
            Cell::number(available.to_string(), available as f64),
            age_cell(&ds.metadata.creation_timestamp),
            Cell::status(&label, kind),
        ],
    }
}

// --- ReplicaSets ---

/// ReplicaSet columns: name, ready, desired, age, status.
pub fn replica_set_columns() -> Vec<ColumnDef> {
    kind_columns(&[
        ColumnDef {
            key: "ready",
            label: "Ready",
            width: Some(90),
            sortable: true,
        },
        ColumnDef {
            key: "desired",
            label: "Desired",
            width: Some(90),
            sortable: true,
        },
    ])
}

/// Map a replica set to a display row.
pub fn replica_set_row(rs: &ReplicaSet) -> ResourceRow {
    let name = rs.metadata.name.clone().unwrap_or_default();
    let namespace = rs.metadata.namespace.clone();
    let desired = rs.spec.as_ref().and_then(|s| s.replicas).unwrap_or(0);
    let ready = rs
        .status
        .as_ref()
        .and_then(|s| s.ready_replicas)
        .unwrap_or(0);
    let (label, kind) = replicas_status(ready, desired);
    ResourceRow {
        id: object_id(namespace.as_deref(), &name),
        namespace,
        cells: vec![
            Cell::text(name),
            Cell::number(ready.to_string(), ready as f64),
            Cell::number(desired.to_string(), desired as f64),
            age_cell(&rs.metadata.creation_timestamp),
            Cell::status(&label, kind),
        ],
    }
}

// --- Jobs ---

/// Job columns: name, completions, duration, age, status.
pub fn job_columns() -> Vec<ColumnDef> {
    kind_columns(&[
        ColumnDef {
            key: "completions",
            label: "Completions",
            width: Some(110),
            sortable: true,
        },
        ColumnDef {
            key: "duration",
            label: "Duration",
            width: Some(100),
            sortable: true,
        },
    ])
}

/// Elapsed wall time from job start to completion (or now, while the job is
/// still running). Missing start time renders as `"-"`.
fn job_duration_cell(job: &Job) -> Cell {
    let Some(start) = job.status.as_ref().and_then(|s| s.start_time.as_ref()) else {
        return Cell::text("-");
    };
    let now = k8s_openapi::jiff::Timestamp::now();
    let end = job
        .status
        .as_ref()
        .and_then(|s| s.completion_time.as_ref())
        .map(|t| t.0)
        .unwrap_or(now);
    let seconds = end.as_second() - start.0.as_second();
    Cell::number(humanize_duration(seconds), seconds as f64)
}

/// Map a job to a display row.
pub fn job_row(job: &Job) -> ResourceRow {
    let name = job.metadata.name.clone().unwrap_or_default();
    let namespace = job.metadata.namespace.clone();
    let succeeded = job.status.as_ref().and_then(|s| s.succeeded).unwrap_or(0);
    let failed = job.status.as_ref().and_then(|s| s.failed).unwrap_or(0);
    let active = job.status.as_ref().and_then(|s| s.active).unwrap_or(0);
    let completions = job.spec.as_ref().and_then(|s| s.completions).unwrap_or(1);
    let (label, kind) = if failed > 0 {
        ("Failed".to_string(), StatusKind::Failed)
    } else if succeeded > 0 {
        ("Succeeded".to_string(), StatusKind::Succeeded)
    } else if active > 0 {
        ("Running".to_string(), StatusKind::Running)
    } else {
        ("Pending".to_string(), StatusKind::Pending)
    };
    ResourceRow {
        id: object_id(namespace.as_deref(), &name),
        namespace,
        cells: vec![
            Cell::text(name),
            Cell::number(format!("{succeeded}/{completions}"), succeeded as f64),
            job_duration_cell(job),
            age_cell(&job.metadata.creation_timestamp),
            Cell::status(&label, kind),
        ],
    }
}

// --- CronJobs ---

/// CronJob columns: name, schedule, suspend, last schedule, age, status.
pub fn cron_job_columns() -> Vec<ColumnDef> {
    kind_columns(&[
        ColumnDef {
            key: "schedule",
            label: "Schedule",
            width: Some(140),
            sortable: true,
        },
        ColumnDef {
            key: "suspend",
            label: "Suspend",
            width: Some(90),
            sortable: true,
        },
        ColumnDef {
            key: "last_schedule",
            label: "Last schedule",
            width: Some(120),
            sortable: true,
        },
    ])
}

/// Map a cron job to a display row.
pub fn cron_job_row(cj: &CronJob) -> ResourceRow {
    let name = cj.metadata.name.clone().unwrap_or_default();
    let namespace = cj.metadata.namespace.clone();
    let schedule = cj.spec.schedule.clone();
    let suspended = cj.spec.suspend.unwrap_or(false);
    let last_schedule = age_cell(
        &cj.status
            .as_ref()
            .and_then(|s| s.last_schedule_time.clone()),
    );
    let (label, kind) = if suspended {
        ("Suspended".to_string(), StatusKind::Suspended)
    } else {
        ("Active".to_string(), StatusKind::Running)
    };
    ResourceRow {
        id: object_id(namespace.as_deref(), &name),
        namespace,
        cells: vec![
            Cell::text(name),
            Cell::text(schedule),
            Cell::text(if suspended { "Yes" } else { "No" }),
            last_schedule,
            age_cell(&cj.metadata.creation_timestamp),
            Cell::status(&label, kind),
        ],
    }
}
