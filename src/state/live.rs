//! Single owner of live, reflector-backed cluster state (OKT-96).
//!
//! ## Why this module exists
//!
//! Live state was previously created *inside the view layer*: `views/workloads.rs`
//! had a `workload_table!` macro that spawned a `drive_reflector` per kind in a
//! `use_effect`, with its own task-abort bookkeeping. `state::resources::ResourceState`
//! — a reusable wrapper for exactly that — had zero callers.
//!
//! Two consequences followed, and both are why this module is the owner now:
//!
//! 1. **The push channel could not work app-wide.** A reflector that only runs
//!    while a view is mounted cannot push anything from another screen.
//! 2. **Watching twice was one careless line away.** Adding a global pods
//!    reflector here while the view already spawned its own produced two
//!    concurrent watch streams on the same kind — doubled API-server load, two
//!    caches, no error anywhere. Neither the build nor the runtime log can see
//!    that; only reading both files reveals it.
//!
//! So: one reflector per kind, owned here, started once per connection, and
//! consumed by whoever needs it (the workloads tables *and* the push channel).
//!
//! ## Lifecycle
//!
//! [`start`] is idempotent per kind (`Option::get_or_insert_with`), so a
//! reconnect cannot stack a second stream on the same kind. It is deliberately
//! **not async**: `Api::<T>::all` needs no API discovery, so every signal exists
//! as soon as `start` returns. That removes a real race — views rendered before
//! an async start completed would otherwise observe `None` and never re-run.
//! [`generation`] still exists as reactive insurance for the ordering between
//! the shell's start effect and a child view's first render.

use std::fmt::Debug;
use std::hash::Hash;
use std::sync::{Arc, Mutex, OnceLock};

use dioxus::prelude::*;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{Node, Pod, Secret};
use kube::{Api, Client, Resource};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use super::resources::ResourceState;

/// Start one reflector, serialising each snapshot into the push channel
/// (OKT-91). Extracted so every kind costs a single line below rather than a
/// copy of the same closure.
fn start_kind<T>(client: Client, kind: &str) -> ResourceState<T>
where
    T: Resource + Clone + DeserializeOwned + Debug + Send + Sync + Serialize + 'static,
    T::DynamicType: Eq + Hash + Clone + Default,
{
    let api = Api::<T>::all(client);
    let kind = kind.to_string();
    ResourceState::watch_with(api, move |rows| {
        // Serialise on the reflector task so the cost stays off the render
        // path. `publish` is thread-safe and a no-op when nobody subscribed.
        let payload: Vec<Value> = rows
            .iter()
            .filter_map(|obj| serde_json::to_value(obj.as_ref()).ok())
            .collect();
        crate::push::publish(&kind, None, payload);
    })
}

/// Reflectors owned by the shell — one per watched kind.
///
/// Adding a kind is three lines: a field, a `get_or_insert_with` line in
/// [`LiveResources::start`], and a signal accessor. Keeping it explicit rather
/// than a map is deliberate: `ResourceState<T>` is generic and
/// `ResourceState<DynamicObject>` does **not** compile (`ApiResource` has no
/// `Default`, which `T::DynamicType: Default` requires), so a heterogeneous map
/// would need `dyn Any` and lose the concrete row types the views depend on.
#[derive(Default)]
pub struct LiveResources {
    pods: Option<ResourceState<Pod>>,
    nodes: Option<ResourceState<Node>>,
    deployments: Option<ResourceState<Deployment>>,
    stateful_sets: Option<ResourceState<StatefulSet>>,
    daemon_sets: Option<ResourceState<DaemonSet>>,
    replica_sets: Option<ResourceState<ReplicaSet>>,
    jobs: Option<ResourceState<Job>>,
    cron_jobs: Option<ResourceState<CronJob>>,
    secrets: Option<ResourceState<Secret>>,
}

impl LiveResources {
    fn count(&self) -> usize {
        [
            self.pods.is_some(),
            self.nodes.is_some(),
            self.deployments.is_some(),
            self.stateful_sets.is_some(),
            self.daemon_sets.is_some(),
            self.replica_sets.is_some(),
            self.jobs.is_some(),
            self.cron_jobs.is_some(),
            self.secrets.is_some(),
        ]
        .iter()
        .filter(|started| **started)
        .count()
    }

    /// Start a reflector per watched kind. Returns how many were newly started.
    ///
    /// Idempotent per kind: an already-watched kind keeps its existing stream,
    /// so reconnecting does not stack a second watch on it.
    pub fn start(&mut self, client: Client) -> usize {
        let before = self.count();
        self.pods
            .get_or_insert_with(|| start_kind::<Pod>(client.clone(), "pods"));
        self.nodes
            .get_or_insert_with(|| start_kind::<Node>(client.clone(), "nodes"));
        self.deployments
            .get_or_insert_with(|| start_kind::<Deployment>(client.clone(), "deployments"));
        self.stateful_sets
            .get_or_insert_with(|| start_kind::<StatefulSet>(client.clone(), "statefulsets"));
        self.daemon_sets
            .get_or_insert_with(|| start_kind::<DaemonSet>(client.clone(), "daemonsets"));
        self.replica_sets
            .get_or_insert_with(|| start_kind::<ReplicaSet>(client.clone(), "replicasets"));
        self.jobs
            .get_or_insert_with(|| start_kind::<Job>(client.clone(), "jobs"));
        self.cron_jobs
            .get_or_insert_with(|| start_kind::<CronJob>(client.clone(), "cronjobs"));
        self.secrets
            .get_or_insert_with(|| start_kind::<Secret>(client.clone(), "secrets"));

        let started = self.count() - before;
        if started > 0 {
            tracing::info!(started, total = self.count(), "live: reflectors running");
            // Bump the reactive generation so views that rendered before the
            // reflectors existed re-run and pick up their signal.
            *LIVE_GEN.write() += 1;
        }
        started
    }

    /// Tear every reflector down (disconnect / context switch).
    pub fn stop(&mut self) {
        // Explicit per-kind stop rather than a loop over a map: `ResourceState<T>`
        // is generic and each kind is a distinct type.
        macro_rules! stop_all {
            ($($field:ident),* $(,)?) => {
                $( if let Some(mut state) = self.$field.take() { state.stop(); } )*
            };
        }
        stop_all!(
            pods,
            nodes,
            deployments,
            stateful_sets,
            daemon_sets,
            replica_sets,
            jobs,
            cron_jobs,
            secrets
        );
    }

    /// Signal of pod rows, or `None` while pods are not being watched.
    pub fn pods_signal(&self) -> Option<Signal<Vec<Arc<Pod>>, SyncStorage>> {
        self.pods.as_ref().map(|s| s.signal())
    }

    pub fn nodes_signal(&self) -> Option<Signal<Vec<Arc<Node>>, SyncStorage>> {
        self.nodes.as_ref().map(|s| s.signal())
    }

    pub fn deployments_signal(&self) -> Option<Signal<Vec<Arc<Deployment>>, SyncStorage>> {
        self.deployments.as_ref().map(|s| s.signal())
    }

    pub fn stateful_sets_signal(&self) -> Option<Signal<Vec<Arc<StatefulSet>>, SyncStorage>> {
        self.stateful_sets.as_ref().map(|s| s.signal())
    }

    pub fn daemon_sets_signal(&self) -> Option<Signal<Vec<Arc<DaemonSet>>, SyncStorage>> {
        self.daemon_sets.as_ref().map(|s| s.signal())
    }

    pub fn replica_sets_signal(&self) -> Option<Signal<Vec<Arc<ReplicaSet>>, SyncStorage>> {
        self.replica_sets.as_ref().map(|s| s.signal())
    }

    pub fn jobs_signal(&self) -> Option<Signal<Vec<Arc<Job>>, SyncStorage>> {
        self.jobs.as_ref().map(|s| s.signal())
    }

    pub fn cron_jobs_signal(&self) -> Option<Signal<Vec<Arc<CronJob>>, SyncStorage>> {
        self.cron_jobs.as_ref().map(|s| s.signal())
    }

    pub fn secrets_signal(&self) -> Option<Signal<Vec<Arc<Secret>>, SyncStorage>> {
        self.secrets.as_ref().map(|s| s.signal())
    }

    /// Serialised rows for `kind`, or `None` when it is not watched.
    ///
    /// Used by the bridge's `Watch` op so a watched kind is served from the
    /// reflector instead of a fresh list.
    pub fn snapshot_json(&self, kind: &str) -> Option<Vec<Value>> {
        fn to_rows<T: Serialize>(rows: Vec<Arc<T>>) -> Vec<Value> {
            rows.iter()
                .filter_map(|obj| serde_json::to_value(obj.as_ref()).ok())
                .collect()
        }
        match kind.to_ascii_lowercase().as_str() {
            "pods" | "pod" => self.pods.as_ref().map(|s| to_rows(s.state())),
            "nodes" | "node" => self.nodes.as_ref().map(|s| to_rows(s.state())),
            "deployments" | "deployment" => self.deployments.as_ref().map(|s| to_rows(s.state())),
            "statefulsets" | "statefulset" => {
                self.stateful_sets.as_ref().map(|s| to_rows(s.state()))
            }
            "daemonsets" | "daemonset" => self.daemon_sets.as_ref().map(|s| to_rows(s.state())),
            "replicasets" | "replicaset" => self.replica_sets.as_ref().map(|s| to_rows(s.state())),
            "jobs" | "job" => self.jobs.as_ref().map(|s| to_rows(s.state())),
            "cronjobs" | "cronjob" => self.cron_jobs.as_ref().map(|s| to_rows(s.state())),
            "secrets" | "secret" => self.secrets.as_ref().map(|s| to_rows(s.state())),
            _ => None,
        }
    }
}

impl Drop for LiveResources {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Bumped whenever a reflector set starts, so views can re-run after the shell
/// installs them. A no-op signal until the Dioxus runtime exists.
pub static LIVE_GEN: GlobalSignal<u64> = Signal::global(|| 0);

/// Process-wide handle. A `Mutex` rather than a signal: the lifecycle is driven
/// by connect/disconnect, not by rendering.
static LIVE: OnceLock<Mutex<LiveResources>> = OnceLock::new();

/// The shared live-state handle.
pub fn live() -> &'static Mutex<LiveResources> {
    LIVE.get_or_init(|| Mutex::new(LiveResources::default()))
}

/// Take the lock, recovering from poisoning: a panic elsewhere must not leave
/// the app permanently unable to watch its own cluster.
fn guard() -> std::sync::MutexGuard<'static, LiveResources> {
    match live().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Start reflectors for a freshly connected client (call on connect).
pub fn start(client: Client) -> usize {
    guard().start(client)
}

/// Stop every reflector (disconnect / before switching context).
pub fn stop() {
    guard().stop();
}

/// Reactive generation counter — read it in an effect to re-run once the
/// reflectors are installed.
pub fn generation() -> u64 {
    *LIVE_GEN.read()
}

/// Whether any reflector is currently running.
pub fn is_watching() -> bool {
    guard().count() > 0
}

/// Signal of pod rows, or `None` while pods are not being watched.
pub fn pods_signal() -> Option<Signal<Vec<Arc<Pod>>, SyncStorage>> {
    guard().pods_signal()
}

pub fn nodes_signal() -> Option<Signal<Vec<Arc<Node>>, SyncStorage>> {
    guard().nodes_signal()
}

pub fn deployments_signal() -> Option<Signal<Vec<Arc<Deployment>>, SyncStorage>> {
    guard().deployments_signal()
}

pub fn stateful_sets_signal() -> Option<Signal<Vec<Arc<StatefulSet>>, SyncStorage>> {
    guard().stateful_sets_signal()
}

pub fn daemon_sets_signal() -> Option<Signal<Vec<Arc<DaemonSet>>, SyncStorage>> {
    guard().daemon_sets_signal()
}

pub fn replica_sets_signal() -> Option<Signal<Vec<Arc<ReplicaSet>>, SyncStorage>> {
    guard().replica_sets_signal()
}

pub fn jobs_signal() -> Option<Signal<Vec<Arc<Job>>, SyncStorage>> {
    guard().jobs_signal()
}

pub fn cron_jobs_signal() -> Option<Signal<Vec<Arc<CronJob>>, SyncStorage>> {
    guard().cron_jobs_signal()
}

pub fn secrets_signal() -> Option<Signal<Vec<Arc<Secret>>, SyncStorage>> {
    guard().secrets_signal()
}

/// Serialised rows for `kind` when it is watched live, else `None` so the
/// caller falls back to a one-shot list.
pub fn snapshot_json(kind: &str) -> Option<Vec<Value>> {
    guard().snapshot_json(kind)
}

/// Keep only objects in `ns`; all namespaces when `ns` is `None` or empty.
///
/// Objects with no `metadata.namespace` are cluster-scoped (e.g. nodes) and are
/// kept regardless — they are not "in a different namespace", they are in none.
pub fn filter_ns(rows: Vec<Value>, ns: Option<&str>) -> Vec<Value> {
    match ns {
        Some(ns) if !ns.is_empty() => rows
            .into_iter()
            .filter(
                |obj| match obj.pointer("/metadata/namespace").and_then(|v| v.as_str()) {
                    Some(found) => found == ns,
                    None => true,
                },
            )
            .collect(),
        _ => rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_manager_watches_nothing() {
        let resources = LiveResources::default();
        assert_eq!(resources.count(), 0);
        assert!(resources.pods_signal().is_none());
        assert!(resources.snapshot_json("pods").is_none());
    }

    #[test]
    fn stop_on_an_empty_manager_is_a_no_op() {
        let mut resources = LiveResources::default();
        resources.stop();
        assert_eq!(resources.count(), 0);
    }

    #[test]
    fn snapshot_of_an_unwatched_kind_is_none_not_empty() {
        // `None` (not `Some(vec![])`) is what tells the bridge to fall back to a
        // real list, so an unwatched kind must never report as watched.
        let resources = LiveResources::default();
        for kind in ["pods", "deployments", "secrets", "nonsense"] {
            assert!(resources.snapshot_json(kind).is_none(), "{kind}");
        }
    }

    #[test]
    fn filter_ns_keeps_cluster_scoped_objects_and_matches_namespaced_ones() {
        let rows = vec![
            serde_json::json!({"metadata": {"namespace": "default", "name": "a"}}),
            serde_json::json!({"metadata": {"namespace": "kube-system", "name": "b"}}),
            serde_json::json!({"metadata": {"name": "node-1"}}),
        ];
        let filtered = filter_ns(rows.clone(), Some("default"));
        assert_eq!(filtered.len(), 2, "default pod + cluster-scoped node");
        assert_eq!(
            filter_ns(rows.clone(), None).len(),
            3,
            "no ns filter keeps all"
        );
        assert_eq!(filter_ns(rows, Some("")).len(), 3, "empty ns means all");
    }
}
