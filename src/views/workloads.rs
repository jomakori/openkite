//! The Workloads view: a kind selector plus a live table per workload kind.

use dioxus::prelude::*;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::Pod;
use kube::api::Api;
use kube::runtime::reflector::store;
use kube::runtime::{watcher, WatchStreamExt};

use crate::components::resource_table::{ResourceRow, ResourceTable, RowActions};
use crate::runtime;
use crate::state::resources::drive_reflector;
use crate::workloads::{
    cron_job_columns, cron_job_row, daemon_set_columns, daemon_set_row, deployment_columns,
    deployment_row, job_columns, job_row, pod_columns, pod_row, replica_set_columns,
    replica_set_row, stateful_set_columns, stateful_set_row, WorkloadKind,
};

/// Start a live reflector for one workload kind and render it as a table.
macro_rules! workload_table {
    ($name:ident, $ty:path, $columns:path, $mapper:path) => {
        #[component]
        fn $name(row_actions: RowActions) -> Element {
            let rows = use_signal_sync(Vec::<ResourceRow>::new);
            // Slot for the running reflector task: aborted on re-run so a
            // switched client (ctrl-tab switcher) never leaves a stale
            // watcher racing writes into the same rows signal.
            let mut reflector_task =
                use_hook(|| CopyValue::new(None::<tokio::task::JoinHandle<()>>));

            use_effect(move || {
                if let Some(task) = reflector_task.write().take() {
                    task.abort();
                }
                let Some(client) = crate::runtime::client() else {
                    return;
                };
                let api = Api::<$ty>::all(client);
                let (store, writer) = store::<$ty>();
                let stream = watcher(api, watcher::Config::default()).default_backoff();
                let mut rows_for_task = rows;
                let task = tokio::spawn(drive_reflector(writer, stream, store, move |snapshot| {
                    let mapped: Vec<ResourceRow> =
                        snapshot.iter().map(|item| $mapper(item.as_ref())).collect();
                    rows_for_task.set(mapped);
                }));
                *reflector_task.write() = Some(task);
            });

            rsx! {
                ResourceTable {
                    columns: $columns(),
                    rows: rows.read().clone(),
                    row_actions: Some(row_actions),
                }
            }
        }
    };
}

workload_table!(PodsTable, Pod, pod_columns, pod_row);
workload_table!(
    DeploymentsTable,
    Deployment,
    deployment_columns,
    deployment_row
);
workload_table!(
    StatefulSetsTable,
    StatefulSet,
    stateful_set_columns,
    stateful_set_row
);
workload_table!(
    DaemonSetsTable,
    DaemonSet,
    daemon_set_columns,
    daemon_set_row
);
workload_table!(
    ReplicaSetsTable,
    ReplicaSet,
    replica_set_columns,
    replica_set_row
);
workload_table!(JobsTable, Job, job_columns, job_row);
workload_table!(CronJobsTable, CronJob, cron_job_columns, cron_job_row);

/// Split a row id (the `object_id` format: `ns/name` or bare `name`) back
/// into namespace + name for the CRUD modal targets.
fn split_row_id(id: &str) -> (Option<String>, String) {
    match id.split_once('/') {
        Some((ns, name)) => (Some(ns.to_string()), name.to_string()),
        None => (None, id.to_string()),
    }
}

/// The Workloads view: a kind selector plus the live table for the selection.
#[component]
pub fn WorkloadView() -> Element {
    let mut kind = use_signal(|| WorkloadKind::Pods);

    // Per-row CRUD actions. The handlers open the matching modal via the
    // runtime CRUD_TARGET signal; the kube apply itself is Phase-1
    // placeholder (see crate::crud::apply_mutation).
    let row_actions = {
        let active_kind = kind();
        let kind_str = active_kind.kind_str().to_string();
        let api_version = active_kind.api_version().to_string();
        RowActions {
            on_edit: {
                let kind_str = kind_str.clone();
                let api_version = api_version.clone();
                Some(EventHandler::new(move |id: String| {
                    let (ns, name) = split_row_id(&id);
                    // Phase-1 placeholder doc: the editor pre-loads a minimal
                    // manifest stub; a real cluster fetch replaces this when
                    // the bridge mutation ops land.
                    let doc = serde_json::json!({
                        "apiVersion": api_version,
                        "kind": kind_str.clone(),
                        "metadata": {
                            "name": name,
                            "namespace": ns,
                        },
                    });
                    runtime::open_editor_for(kind_str.clone(), doc);
                }))
            },
            on_delete: {
                let kind_str = kind_str.clone();
                Some(EventHandler::new(move |id: String| {
                    let (ns, name) = split_row_id(&id);
                    runtime::open_delete_for(kind_str.clone(), ns, name);
                }))
            },
            on_scale: {
                let kind_str = kind_str.clone();
                Some(EventHandler::new(move |id: String| {
                    let (ns, name) = split_row_id(&id);
                    runtime::open_scale_for(kind_str.clone(), ns, name, 1);
                }))
            },
        }
    };

    rsx! {
        div { class: "workloads",
            div { class: "kind-tabs",
                for k in WorkloadKind::ALL {
                    button {
                        class: if kind() == k { "kind-tab active" } else { "kind-tab" },
                        onclick: move |_| kind.set(k),
                        "{k.label()}"
                    }
                }
                button {
                    class: "btn btn-primary new-resource",
                    onclick: move |_| {
                        let k = kind();
                        runtime::open_new_for(k.kind_str().to_string());
                    },
                    "+ New"
                }
            }
            match kind() {
                WorkloadKind::Pods => rsx! { PodsTable { row_actions: row_actions.clone() } },
                WorkloadKind::Deployments => rsx! { DeploymentsTable { row_actions: row_actions.clone() } },
                WorkloadKind::StatefulSets => rsx! { StatefulSetsTable { row_actions: row_actions.clone() } },
                WorkloadKind::DaemonSets => rsx! { DaemonSetsTable { row_actions: row_actions.clone() } },
                WorkloadKind::ReplicaSets => rsx! { ReplicaSetsTable { row_actions: row_actions.clone() } },
                WorkloadKind::Jobs => rsx! { JobsTable { row_actions: row_actions.clone() } },
                WorkloadKind::CronJobs => rsx! { CronJobsTable { row_actions: row_actions.clone() } },
            }
        }
    }
}
