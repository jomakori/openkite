//! The Workloads view: a kind selector plus a live table per workload kind.

use dioxus::prelude::*;

use crate::components::resource_table::{ResourceRow, ResourceTable, RowActions};
use crate::runtime;
use crate::workloads::{
    cron_job_columns, cron_job_row, daemon_set_columns, daemon_set_row, deployment_columns,
    deployment_row, job_columns, job_row, node_columns, node_row, pod_columns, pod_row,
    replica_set_columns, replica_set_row, secret_columns, secret_row, stateful_set_columns,
    stateful_set_row, WorkloadKind,
};

/// Render a workload kind from the shell's shared reflector (OKT-96).
///
/// The reflector itself lives in `state::live` — **one watch per kind for the
/// whole app** — so this component only maps the live snapshot into rows. It
/// used to spawn its own `drive_reflector` here, which is how a second global
/// reflector for the same kind became possible; there is now exactly one owner.
///
/// `$live` is the matching signal accessor, e.g.
/// `crate::state::live::pods_signal`. `on_row_click` is optional; when supplied
/// the table exposes per-row click events so a slide-over can open.
macro_rules! workload_table {
    ($name:ident, $columns:path, $mapper:path, $live:path) => {
        #[component]
        fn $name(
            row_actions: RowActions,
            #[props(default)] on_row_click: Option<EventHandler<ResourceRow>>,
        ) -> Element {
            let mut rows = use_signal_sync(Vec::<ResourceRow>::new);

            // Reading `generation()` keeps this reactive. The shell installs the
            // reflectors from an effect, which can run after this component's
            // first render; the counter bumps when they appear, so this re-runs
            // and picks up its signal instead of staying empty forever.
            use_effect(move || {
                let _generation = crate::state::live::generation();
                let Some(signal) = $live() else {
                    return;
                };
                let mapped: Vec<ResourceRow> = signal
                    .read()
                    .iter()
                    .map(|item| $mapper(item.as_ref()))
                    .collect();
                // OKT-96 evidence: this effect has re-run because the live
                // signal changed, and the mapped rows are about to be handed to
                // `ResourceTable`. Logging the count makes the reflector→view
                // hop observable from outside the process, which is the
                // difference between "state updated" and "the UI reacted".
                let count = mapped.len();
                rows.set(mapped);
                tracing::info!(view = stringify!($name), rows = count, "live: view rows");
            });

            rsx! {
                ResourceTable {
                    columns: $columns(),
                    rows: rows.read().clone(),
                    row_actions: Some(row_actions),
                    on_row_click,
                }
            }
        }
    };
}

workload_table!(
    PodsTable,
    pod_columns,
    pod_row,
    crate::state::live::pods_signal
);
workload_table!(
    NodesTable,
    node_columns,
    node_row,
    crate::state::live::nodes_signal
);
workload_table!(
    DeploymentsTable,
    deployment_columns,
    deployment_row,
    crate::state::live::deployments_signal
);
workload_table!(
    StatefulSetsTable,
    stateful_set_columns,
    stateful_set_row,
    crate::state::live::stateful_sets_signal
);
workload_table!(
    DaemonSetsTable,
    daemon_set_columns,
    daemon_set_row,
    crate::state::live::daemon_sets_signal
);
workload_table!(
    ReplicaSetsTable,
    replica_set_columns,
    replica_set_row,
    crate::state::live::replica_sets_signal
);
workload_table!(
    JobsTable,
    job_columns,
    job_row,
    crate::state::live::jobs_signal
);
workload_table!(
    CronJobsTable,
    cron_job_columns,
    cron_job_row,
    crate::state::live::cron_jobs_signal
);
workload_table!(
    SecretsTable,
    secret_columns,
    secret_row,
    crate::state::live::secrets_signal
);

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
                WorkloadKind::Nodes => rsx! { NodesTable { row_actions: row_actions.clone() } },
                WorkloadKind::Deployments => rsx! { DeploymentsTable { row_actions: row_actions.clone() } },
                WorkloadKind::StatefulSets => rsx! { StatefulSetsTable { row_actions: row_actions.clone() } },
                WorkloadKind::DaemonSets => rsx! { DaemonSetsTable { row_actions: row_actions.clone() } },
                WorkloadKind::ReplicaSets => rsx! { ReplicaSetsTable { row_actions: row_actions.clone() } },
                WorkloadKind::Jobs => rsx! { JobsTable { row_actions: row_actions.clone() } },
                WorkloadKind::CronJobs => rsx! { CronJobsTable { row_actions: row_actions.clone() } },
                WorkloadKind::Secrets => rsx! {
                    SecretsTable {
                        row_actions: row_actions.clone(),
                        on_row_click: {
                            EventHandler::new(move |row: crate::components::resource_table::ResourceRow| {
                                // The reflector surfaces only the row projection;
                                // the slide-over opens on a stub carrying the
                                // row id. A real cluster fetch replaces this when
                                // the bridge fetch ops land (Phase 1).
                                let (ns, name) = split_row_id(&row.id);
                                let mut secret = k8s_openapi::api::core::v1::Secret::default();
                                secret.metadata.name = Some(name);
                                secret.metadata.namespace = ns;
                                *crate::runtime::SELECTED_SECRET.write() = Some(secret);
                            })
                        },
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_row_id_splits_namespaced_id() {
        assert_eq!(
            split_row_id("default/web"),
            (Some("default".to_string()), "web".to_string())
        );
    }

    #[test]
    fn split_row_id_keeps_bare_id_and_extra_slashes() {
        assert_eq!(split_row_id("node-1"), (None, "node-1".to_string()));
        assert_eq!(
            split_row_id("ns/a/b"),
            (Some("ns".to_string()), "a/b".to_string())
        );
    }
}
