//! Workload views: browse and operate on all workload kinds (Pods → CronJobs).
//!
//! This module holds the pure mapping — column layouts, row mappers, and status
//! derivation — that the Workloads view feeds into the resource table. Keeping
//! it free of Dioxus makes it testable in isolation.

use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::Pod;

use crate::components::resource_table::{Cell, ColumnDef, ResourceRow};
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
}

/// A name + status column pair, the common prefix for every kind.
fn name_status_columns(extra: &[ColumnDef]) -> Vec<ColumnDef> {
    let mut columns = vec![
        ColumnDef {
            key: "name",
            label: "Name",
            width: None,
            sortable: true,
        },
        ColumnDef {
            key: "status",
            label: "Status",
            width: Some(110),
            sortable: true,
        },
    ];
    columns.extend(extra.iter().cloned());
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

// --- Pods ---

/// Pod columns: name, status, ready, restarts.
pub fn pod_columns() -> Vec<ColumnDef> {
    name_status_columns(&[
        ColumnDef {
            key: "ready",
            label: "Ready",
            width: Some(90),
            sortable: true,
        },
        ColumnDef {
            key: "restarts",
            label: "Restarts",
            width: Some(90),
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
            Cell::status(&label, kind),
            ready_cell(ready, total),
            Cell::number(restarts.to_string(), restarts as f64),
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

/// Deployment columns: name, status, ready, available.
pub fn deployment_columns() -> Vec<ColumnDef> {
    name_status_columns(&[
        ColumnDef {
            key: "ready",
            label: "Ready",
            width: Some(90),
            sortable: true,
        },
        ColumnDef {
            key: "available",
            label: "Available",
            width: Some(90),
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
            Cell::status(&label, kind),
            ready_cell(ready, desired),
            Cell::number(available.to_string(), available as f64),
        ],
    }
}

// --- StatefulSets ---

/// StatefulSet columns: name, status, ready.
pub fn stateful_set_columns() -> Vec<ColumnDef> {
    name_status_columns(&[ColumnDef {
        key: "ready",
        label: "Ready",
        width: Some(90),
        sortable: true,
    }])
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
    let (label, kind) = replicas_status(ready, desired);
    ResourceRow {
        id: object_id(namespace.as_deref(), &name),
        namespace,
        cells: vec![
            Cell::text(name),
            Cell::status(&label, kind),
            ready_cell(ready, desired),
        ],
    }
}

// --- DaemonSets ---

/// DaemonSet columns: name, status, ready, available.
pub fn daemon_set_columns() -> Vec<ColumnDef> {
    name_status_columns(&[
        ColumnDef {
            key: "ready",
            label: "Ready",
            width: Some(90),
            sortable: true,
        },
        ColumnDef {
            key: "available",
            label: "Available",
            width: Some(90),
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
            Cell::status(&label, kind),
            ready_cell(ready, desired),
            Cell::number(available.to_string(), available as f64),
        ],
    }
}

// --- ReplicaSets ---

/// ReplicaSet columns: name, status, ready.
pub fn replica_set_columns() -> Vec<ColumnDef> {
    name_status_columns(&[ColumnDef {
        key: "ready",
        label: "Ready",
        width: Some(90),
        sortable: true,
    }])
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
            Cell::status(&label, kind),
            ready_cell(ready, desired),
        ],
    }
}

// --- Jobs ---

/// Job columns: name, status, completions.
pub fn job_columns() -> Vec<ColumnDef> {
    name_status_columns(&[ColumnDef {
        key: "completions",
        label: "Completions",
        width: Some(110),
        sortable: true,
    }])
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
            Cell::status(&label, kind),
            Cell::number(format!("{succeeded}/{completions}"), succeeded as f64),
        ],
    }
}

// --- CronJobs ---

/// CronJob columns: name, status, schedule, suspend.
pub fn cron_job_columns() -> Vec<ColumnDef> {
    name_status_columns(&[
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
    ])
}

/// Map a cron job to a display row.
pub fn cron_job_row(cj: &CronJob) -> ResourceRow {
    let name = cj.metadata.name.clone().unwrap_or_default();
    let namespace = cj.metadata.namespace.clone();
    let schedule = cj
        .spec
        .as_ref()
        .map(|s| s.schedule.clone())
        .unwrap_or_default();
    let suspended = cj.spec.as_ref().and_then(|s| s.suspend).unwrap_or(false);
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
            Cell::status(&label, kind),
            Cell::text(schedule),
            Cell::text(if suspended { "Yes" } else { "No" }),
        ],
    }
}
