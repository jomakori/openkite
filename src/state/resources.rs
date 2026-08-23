//! Live resource state — kube reflector store wrapped in a Dioxus signal.
//!
//! A background task runs the kube watcher (auto-reconnecting with backoff) and
//! republishes a fresh snapshot into a sync signal after every event, so views
//! re-render without manual refresh. [`ResourceState::stop`] tears the watcher
//! down; the caller rebuilds one on context switch.

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
///
/// The watcher task applies each event to the backing [`Store`] and pushes the
/// resulting snapshot into a sync signal, so any view reading [`signal`] re-
/// renders as objects are added, modified, or deleted.
///
/// [`signal`]: ResourceState::signal
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
    /// Start watching `T` across all namespaces (namespaced kinds) or the whole
    /// cluster (cluster-scoped kinds).
    pub fn start(client: Client) -> Self {
        Self::watch(Api::all(client))
    }

    /// Start watching an explicit [`Api`], letting callers attach list/field
    /// selectors before handing it over.
    pub fn watch(api: Api<T>) -> Self {
        let (store, writer) = store::<T>();
        let stream = watcher(api, watcher::Config::default()).default_backoff();

        let snapshot: Signal<Vec<Arc<T>>, SyncStorage> = Signal::new_maybe_sync(Vec::new());
        let mut snapshot_task = snapshot;
        let store_task = store.clone();

        let task = tokio::spawn(drive_reflector(writer, stream, store_task, move |rows| {
            snapshot_task.set(rows)
        }));

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

/// Drive a reflector to completion: apply each stream event to `store` and
/// invoke `on_snapshot` with the fresh snapshot after every event.
///
/// Extracted as a free function so the wiring can be exercised with a mock
/// stream; [`ResourceState`] wraps it with a signal-backed callback. The stream
/// is expected to already carry backoff (see [`WatchStreamExt::default_backoff`]).
pub async fn drive_reflector<T, W, F>(
    writer: store::Writer<T>,
    stream: W,
    store: Store<T>,
    on_snapshot: F,
) where
    T: Resource + Clone + DeserializeOwned + Debug + Send + Sync + 'static,
    T::DynamicType: Eq + Hash + Clone + Default,
    W: Stream<Item = watcher::Result<watcher::Event<T>>>,
    F: Fn(Vec<Arc<T>>),
{
    let stream = reflector::reflector(writer, stream);
    let mut stream = Box::pin(stream);
    while let Some(event) = stream.next().await {
        match event {
            Ok(_) => on_snapshot(store.state()),
            Err(err) => tracing::warn!(error = ?err, "reflector watch error"),
        }
    }
}
