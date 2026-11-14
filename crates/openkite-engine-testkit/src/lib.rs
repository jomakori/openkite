//! In-process fake Kubernetes apiserver for engine integration tests.
mod discovery;
mod routes;
mod status;
mod store;

use std::collections::HashMap;
use std::sync::Arc;

use axum::http::Uri;
use kube::{Client, Config};
use serde::Serialize;
use serde_json::Value;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

pub use store::Gvk;

/// A change fed into the store, and the watch type it surfaces as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// An object that did not exist before.
    Added,
    /// An object already in the store.
    Modified,
    /// An object to remove.
    Deleted,
}

/// Describes one Kubernetes kind served by the fake apiserver.
#[derive(Clone, Debug)]
pub struct ResourceDef {
    pub group: String,
    pub version: String,
    pub kind: String,
    pub plural: String,
    pub namespaced: bool,
    pub verbs: Vec<String>,
    pub short_names: Vec<String>,
    pub categories: Vec<String>,
}

impl ResourceDef {
    /// Builds a resource definition with the common CRUD+watch verb set.
    pub fn new(group: &str, version: &str, kind: &str, plural: &str, namespaced: bool) -> Self {
        Self {
            group: group.to_string(),
            version: version.to_string(),
            kind: kind.to_string(),
            plural: plural.to_string(),
            namespaced,
            verbs: [
                "create",
                "delete",
                "deletecollection",
                "get",
                "list",
                "patch",
                "update",
                "watch",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            short_names: Vec::new(),
            categories: Vec::new(),
        }
    }

    /// The group/version/plural key this resource is stored under.
    pub fn gvk(&self) -> Gvk {
        Gvk {
            group: self.group.clone(),
            version: self.version.clone(),
            plural: self.plural.clone(),
        }
    }
}

/// Builder that seeds the fake apiserver with resource definitions and typed objects.
#[derive(Default)]
pub struct Scenario {
    pub(crate) resources: Vec<ResourceDef>,
    pub(crate) objects: HashMap<Gvk, Vec<Value>>,
}

impl Scenario {
    /// Starts an empty scenario.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a resource kind so discovery and list endpoints can serve it.
    pub fn with_resource(mut self, def: ResourceDef) -> Self {
        self.resources.push(def);
        self
    }

    /// Seeds one typed object under the given resource definition.
    pub fn add<T: Serialize>(mut self, def: &ResourceDef, object: T) -> Self {
        let value = serde_json::to_value(object).expect("scenario object must serialize");
        self.objects.entry(def.gvk()).or_default().push(value);
        self
    }

    /// Sets `metadata.namespace` on every seeded object of this kind that lacks one.
    pub fn with_namespace(mut self, def: &ResourceDef, namespace: &str) -> Self {
        if let Some(items) = self.objects.get_mut(&def.gvk()) {
            for item in items {
                let obj = item.as_object_mut().expect("object must be a JSON object");
                let meta = obj
                    .entry("metadata")
                    .or_insert_with(|| Value::Object(Default::default()));
                let meta = meta
                    .as_object_mut()
                    .expect("metadata must be a JSON object");
                meta.entry("namespace")
                    .or_insert_with(|| Value::String(namespace.to_string()));
            }
        }
        self
    }
}

/// A running fake apiserver bound to a local port.
pub struct FakeApiserver {
    base_url: String,
    store: Arc<store::Store>,
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<JoinHandle<()>>,
}

impl FakeApiserver {
    /// Starts the fake apiserver on `127.0.0.1:0`, seeded from `scenario`.
    pub async fn start(scenario: Scenario) -> Self {
        let store = Arc::new(store::Store::new(scenario));
        let app = routes::router(Arc::clone(&store));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local port");
        let addr = listener.local_addr().expect("local addr");
        let (tx, rx) = oneshot::channel();
        let join = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await
                .expect("fake apiserver task");
        });
        Self {
            base_url: format!("http://{addr}"),
            store,
            shutdown: Some(tx),
            join: Some(join),
        }
    }

    /// Applies a change and feeds it to every open watch on that resource.
    pub fn apply(&self, def: &ResourceDef, event: Event, object: Value) {
        self.store.apply(&def.gvk(), event, object);
    }

    /// The resourceVersion a resume would ask for: the newest one applied.
    pub fn resource_version(&self) -> String {
        self.store.resource_version()
    }

    /// The base URL the fake apiserver is listening on.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Builds a real `kube::Client` pointed at this fake apiserver.
    pub fn client(&self) -> Client {
        let uri: Uri = self.base_url.parse().expect("valid base url");
        let mut config = Config::new(uri);
        config.accept_invalid_certs = true;
        Client::try_from(config).expect("build kube client")
    }

    /// Stops the server and waits for its task to finish.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            let _ = join.await;
        }
    }
}
