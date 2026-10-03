//! The Pod inventory surface — the reference's `#view-workloads` table.
//!
//! One source, both hosts: the crate owns the columns, the row shape and the
//! pager; a host maps its pod objects onto [`PodRow`] and hands them over.
//! Namespace scoping and filtering come from the shared
//! [`ResourceTable`] and the console's one namespace selection.

use dioxus::prelude::*;

use crate::components::resource_table::{
    Cell, ColumnDef, HealthDot, ResourceRow, ResourceTable, TableStatus,
};
use crate::components::status_badge::StatusKind;

/// Rows the inventory shows per page.
pub const PODS_PER_PAGE: usize = 10;

/// The empty-state line the inventory renders when nothing matches.
pub const EMPTY_MESSAGE: &str = "No pods in this view.";

/// One pod's display fields. A host maps its own pod object onto this shape.
#[derive(Debug, Clone, PartialEq)]
pub struct PodRow {
    pub name: String,
    pub namespace: String,
    pub health: Vec<HealthDot>,
    pub restarts: i32,
    pub controller: String,
    pub node: String,
    pub qos: String,
    pub age: String,
    /// Lifecycle phase or reason, e.g. `Running`, `CrashLoopBackOff`.
    pub phase: String,
}

impl PodRow {
    /// The display row this pod paints.
    pub fn to_resource_row(&self) -> ResourceRow {
        let restarts = self.restarts.max(0);
        ResourceRow {
            id: format!("{}/{}", self.namespace, self.name),
            namespace: Some(self.namespace.clone()),
            cells: vec![
                Cell::text(self.name.clone()).with_class("resource-name"),
                Cell::text(self.namespace.clone()).with_class("namespace"),
                Cell::health_dots(self.health.clone()),
                Cell::number(restarts.to_string(), restarts as f64).with_class(if restarts > 0 {
                    "restarts warn"
                } else {
                    "restarts"
                }),
                Cell::text(self.controller.clone()).with_class("controller"),
                Cell::text(self.node.clone()).with_class("node"),
                Cell::text(self.qos.clone()).with_class("qos"),
                Cell::text(self.age.clone()).with_class("qos"),
                Cell::status(&self.phase, pod_status_kind(&self.phase)),
            ],
        }
    }
}

/// The inventory's columns, in the reference's order.
pub fn pod_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            key: "name",
            label: "Name",
            width: Some(240),
            sortable: true,
        },
        ColumnDef {
            key: "namespace",
            label: "Namespace",
            width: Some(140),
            sortable: true,
        },
        ColumnDef {
            key: "health",
            label: "Health",
            width: Some(90),
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
            width: Some(150),
            sortable: true,
        },
        ColumnDef {
            key: "qos",
            label: "QoS",
            width: Some(110),
            sortable: true,
        },
        ColumnDef {
            key: "age",
            label: "Age",
            width: Some(80),
            sortable: true,
        },
        ColumnDef {
            key: "status",
            label: "Status",
            width: Some(130),
            sortable: true,
        },
    ]
}

/// The semantic status a phase or reason maps to.
pub fn pod_status_kind(phase: &str) -> StatusKind {
    match phase {
        "Running" => StatusKind::Running,
        "Succeeded" | "Completed" => StatusKind::Succeeded,
        "Failed" | "Error" => StatusKind::Failed,
        "Pending" | "ContainerCreating" | "PodInitializing" | "Terminating" => StatusKind::Pending,
        reason if reason.starts_with("CrashLoop") => StatusKind::CrashLoop,
        _ => StatusKind::Unknown,
    }
}

/// The Pod inventory surface.
#[component]
pub fn PodInventory(rows: Vec<PodRow>, #[props(default)] status: TableStatus) -> Element {
    let resource_rows: Vec<ResourceRow> = rows.iter().map(PodRow::to_resource_row).collect();
    rsx! {
        ResourceTable {
            columns: pod_columns(),
            rows: resource_rows,
            status,
            empty_message: Some(EMPTY_MESSAGE.to_string()),
            per_page: Some(PODS_PER_PAGE),
            page_label: Some("pods".to_string()),
        }
    }
}
