//! The Workloads view: a kind selector plus a live table per workload kind.

use dioxus::prelude::*;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::Pod;
use kube::api::Api;
use kube::runtime::reflector::store;
use kube::runtime::{watcher, WatchStreamExt};

use crate::components::resource_table::{ResourceRow, ResourceTable};
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
        fn $name() -> Element {
            let rows = use_signal_sync(Vec::<ResourceRow>::new);
            // Slot for the running reflector task: aborted on re-run so a
            // switched client (ctrl-tab switcher) never leaves a stale
            // watcher racing writes into the same rows signal.
            let mut reflector_task = use_hook(|| CopyValue::new(None::<tokio::task::JoinHandle<()>>));

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

/// The Workloads view: a kind selector plus the live table for the selection.
#[component]
pub fn WorkloadView() -> Element {
    let mut kind = use_signal(|| WorkloadKind::Pods);

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
            }
            match kind() {
                WorkloadKind::Pods => rsx! { PodsTable {} },
                WorkloadKind::Deployments => rsx! { DeploymentsTable {} },
                WorkloadKind::StatefulSets => rsx! { StatefulSetsTable {} },
                WorkloadKind::DaemonSets => rsx! { DaemonSetsTable {} },
                WorkloadKind::ReplicaSets => rsx! { ReplicaSetsTable {} },
                WorkloadKind::Jobs => rsx! { JobsTable {} },
                WorkloadKind::CronJobs => rsx! { CronJobsTable {} },
            }
        }
    }
}
