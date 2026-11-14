//! Informer supervisor: watches resources and broadcasts events to subscribers.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use kube::api::Api;
use kube::core::{ApiResource, DynamicObject};
use kube::runtime::watcher::{self, Event};
use kube::runtime::WatchStreamExt;
use kube::Client;
use serde_json::Value;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;

use crate::interest::{InformerState, Interest, InterestRegistry, Scope};

/// How long a watch connection runs before the server closes it, forcing a resume attempt.
const WATCH_TIMEOUT_SECS: u32 = 1;

/// Refcount-key for an informer: the same triple `Interest::key` uses.
type Key = (kube::core::GroupVersionKind, Scope, Option<String>);

#[derive(Debug, Clone)]
pub enum InformerEvent {
    Reset,
    Upsert(Value),
    Delete(String),
}

struct InformerHandle {
    tx: broadcast::Sender<InformerEvent>,
    task: JoinHandle<()>,
    generation: Arc<AtomicU64>,
}

struct Inner {
    client: Client,
    interests: Mutex<InterestRegistry>,
    handles: Mutex<HashMap<Key, InformerHandle>>,
}

impl Inner {
    fn new(client: Client) -> Self {
        Self {
            client,
            interests: Mutex::new(InterestRegistry::new()),
            handles: Mutex::new(HashMap::new()),
        }
    }

    async fn subscribe(inner: Arc<Inner>, interest: Interest) -> Subscription {
        {
            let mut registry = inner.interests.lock().await;
            registry.acquire(&interest);
            registry.set_state(&interest, InformerState::Running);
        }

        let key = interest.key();
        let needs_start = !inner.handles.lock().await.contains_key(&key);
        if needs_start {
            start_informer(&inner, interest.clone()).await;
        }

        let rx = {
            let handles = inner.handles.lock().await;
            let handle = handles.get(&key).expect("handle just ensured to exist");
            // Bump the generation so any park timer scheduled before this subscribe is cancelled.
            handle.generation.fetch_add(1, Ordering::SeqCst);
            handle.tx.subscribe()
        };

        Subscription {
            interest,
            inner,
            rx,
        }
    }

    /// Drops the refcount; schedules a park (handle removal) after the interest's class grace.
    async fn release(inner: &Arc<Inner>, interest: &Interest) {
        let grace = {
            let mut registry = inner.interests.lock().await;
            registry.release(interest)
        };
        let Some(grace) = grace else {
            return;
        };

        let key = interest.key();
        let my_generation = {
            let handles = inner.handles.lock().await;
            match handles.get(&key) {
                Some(handle) => handle.generation.load(Ordering::SeqCst),
                None => return,
            }
        };

        let inner = Arc::clone(inner);
        let interest = interest.clone();
        tokio::spawn(async move {
            tokio::time::sleep(grace).await;

            let removed = {
                let mut handles = inner.handles.lock().await;
                let current = handles
                    .get(&key)
                    .map(|handle| handle.generation.load(Ordering::SeqCst));
                if current != Some(my_generation) {
                    None
                } else {
                    handles.remove(&key)
                }
            };
            let Some(handle) = removed else {
                return;
            };
            handle.task.abort();

            let mut registry = inner.interests.lock().await;
            registry.set_state(&interest, InformerState::Parked);
        });
    }
}

/// Spawns the watch loop for a freshly-acquired interest and records its handle.
async fn start_informer(inner: &Arc<Inner>, interest: Interest) {
    let (tx, _rx) = broadcast::channel(256);
    let generation = Arc::new(AtomicU64::new(0));

    let client = inner.client.clone();
    let tx_task = tx.clone();
    let interest_task = interest.clone();
    let task = tokio::spawn(run_watch(client, interest_task, tx_task));

    let mut handles = inner.handles.lock().await;
    handles.insert(
        interest.key(),
        InformerHandle {
            tx,
            task,
            generation,
        },
    );
}

/// Runs the watch stream for one interest: broadcasts upserts/deletes, and a single
/// `Reset` when a 410 forces a re-list. Backoff between watch errors is handled by
/// `default_backoff`, so a flaky connection never busy-loops.
async fn run_watch(client: Client, interest: Interest, tx: broadcast::Sender<InformerEvent>) {
    let resource = ApiResource::from_gvk(&interest.gvk);
    let api: Api<DynamicObject> = match &interest.scope {
        Scope::Cluster => Api::all_with(client, &resource),
        Scope::Namespace(namespace) => Api::namespaced_with(client, namespace, &resource),
    };
    let config = watcher::Config {
        label_selector: interest.selector.clone(),
        timeout: Some(WATCH_TIMEOUT_SECS),
        ..Default::default()
    };

    let mut stream = Box::pin(watcher::watcher(api, config).default_backoff());
    let mut pending_reset = false;
    while let Some(event) = stream.next().await {
        match event {
            Ok(Event::Init) | Ok(Event::InitApply(_)) => {}
            Ok(Event::InitDone) => {
                if pending_reset {
                    pending_reset = false;
                    let _ = tx.send(InformerEvent::Reset);
                }
            }
            Ok(Event::Apply(obj)) => {
                let value = serde_json::to_value(&obj).unwrap_or(Value::Null);
                let _ = tx.send(InformerEvent::Upsert(value));
            }
            Ok(Event::Delete(obj)) => {
                let name = obj.metadata.name.clone().unwrap_or_default();
                let _ = tx.send(InformerEvent::Delete(name));
            }
            Err(watcher::Error::WatchError(status)) if status.code == 410 => {
                pending_reset = true;
            }
            Err(_) => {}
        }
    }
}

pub struct InformerSupervisor {
    inner: Arc<Inner>,
}

impl InformerSupervisor {
    /// A supervisor that watches through `client`; no informers are running until `subscribe`.
    pub fn new(client: Client) -> Self {
        Self {
            inner: Arc::new(Inner::new(client)),
        }
    }

    /// Refcount+1 on `interest`, starting its informer if this is the first subscriber.
    pub async fn subscribe(&self, interest: Interest) -> Subscription {
        Inner::subscribe(Arc::clone(&self.inner), interest).await
    }

    /// How many distinct informers are currently tracked (running or parking).
    pub async fn informer_count(&self) -> usize {
        self.inner.handles.lock().await.len()
    }

    pub async fn state(&self, interest: &Interest) -> Option<InformerState> {
        self.inner.interests.lock().await.state(interest)
    }
}

pub struct Subscription {
    interest: Interest,
    inner: Arc<Inner>,
    rx: broadcast::Receiver<InformerEvent>,
}

impl Subscription {
    pub async fn next(&mut self) -> Option<InformerEvent> {
        self.rx.recv().await.ok()
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let inner = Arc::clone(&self.inner);
        let interest = self.interest.clone();
        tokio::spawn(async move {
            Inner::release(&inner, &interest).await;
        });
    }
}
