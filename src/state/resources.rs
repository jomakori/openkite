//! Live resource state — kube reflector store wrapped in a Dioxus signal (OKT-9).
//!
//! A background task runs the kube watcher (auto-reconnecting with backoff) and
//! pushes a fresh snapshot into a sync Dioxus signal on every event, so views
//! re-render without manual refresh. [`ResourceState::stop`] tears the watcher
//! down on cluster disconnect; the caller rebuilds one on context switch.

#![allow(dead_code)] // consumed by OKT-10 workload/config views

use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

use dioxus::prelude::*;
use futures::StreamExt;
use kube::api::Api;
use kube::runtime::reflector::{self, store, Store};
use kube::runtime::{watcher, WatchStreamExt};
use kube::{Client, Resource};
use serde::de::DeserializeOwned;

/// Live, reflector-backed state for a single resource kind `T`.
pub struct ResourceState<T>
where
    T: Resource + Clone + DeserializeOwned + Debug + Send + Sync + 'static,
    T::DynamicType: Eq + Hash + Clone + Default,
{
    store: Store<T>,
    snapshot: Signal<Vec<Arc<T>>, SyncStorage>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl<T> ResourceState<T>
where
    T: Resource + Clone + DeserializeOwned + Debug + Send + Sync + 'static,
    T::DynamicType: Eq + Hash + Clone + Default,
{
    /// Start watching `T` across all namespaces (namespaced kinds) or the
    /// whole cluster (cluster-scoped kinds).
    pub fn start(client: Client) -> Self {
        Self::watch(Api::all(client))
    }

    /// Start watching an explicit [`Api`] (lets callers attach list/field
    /// selectors before handing it over).
    pub fn watch(api: Api<T>) -> Self {
        let (store, writer) = store::<T>();
        // kube 4.x has no `resync_period`; consistency comes from the watch
        // timeout + `default_backoff` re-list on error.
        let config = watcher::Config::default();
        let stream = watcher(api, config).default_backoff();

        let snapshot: Signal<Vec<Arc<T>>, SyncStorage> = Signal::new_maybe_sync(Vec::new());
        let mut snapshot_task = snapshot;
        let store_task = store.clone();

        let task = tokio::spawn(async move {
            let stream = reflector::reflector(writer, stream);
            let mut stream = Box::pin(stream);
            while let Some(event) = stream.next().await {
                match event {
                    Ok(_) => snapshot_task.set(store_task.state()),
                    Err(err) => tracing::warn!(error = ?err, "reflector watch error"),
                }
            }
        });

        Self {
            store,
            snapshot,
            task: Some(task),
        }
    }

    /// Current snapshot of all watched objects (read-only).
    pub fn state(&self) -> Vec<Arc<T>> {
        self.store.state()
    }

    /// The reactive signal views read to re-render on change.
    pub fn signal(&self) -> Signal<Vec<Arc<T>>, SyncStorage> {
        self.snapshot
    }

    /// Stop the watcher (cluster disconnect / context switch).
    pub fn stop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl<T> Drop for ResourceState<T>
where
    T: Resource + Clone + DeserializeOwned + Debug + Send + Sync + 'static,
    T::DynamicType: Eq + Hash + Clone + Default,
{
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::ConfigMap;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn config_map(name: &str) -> ConfigMap {
        ConfigMap {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn store_starts_empty() {
        let (store, _writer) = store::<ConfigMap>();
        assert!(store.state().is_empty());
    }

    #[tokio::test]
    async fn reflector_applies_apply_events() {
        let (store, writer) = store::<ConfigMap>();
        let events = futures::stream::iter(vec![
            Ok(watcher::Event::Apply(config_map("a"))),
            Ok(watcher::Event::Apply(config_map("b"))),
        ]);
        let stream = reflector::reflector(writer, events);
        let mut stream = Box::pin(stream);
        while let Some(event) = stream.next().await {
            assert!(event.is_ok());
        }
        assert_eq!(store.state().len(), 2);
    }
}
