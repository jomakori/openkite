//! In-memory object store: seeded objects and a deterministic resourceVersion.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde_json::{json, Map, Value};
use tokio::sync::broadcast;

use crate::{Event, ResourceDef, Scenario};

/// How many events the store keeps for `resourceVersion` resume.
const LOG_CAPACITY: usize = 256;

/// Group/version/plural key identifying a served resource.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Gvk {
    pub group: String,
    pub version: String,
    pub plural: String,
}

/// One logged change, replayed to a watch that resumed from an older resourceVersion.
#[derive(Clone, Debug)]
pub(crate) struct WatchRecord {
    pub(crate) gvk: Gvk,
    pub(crate) event: Event,
    pub(crate) resource_version: u64,
    pub(crate) object: Value,
}

/// Holds the seeded surface of a fake apiserver.
pub(crate) struct Store {
    pub(crate) resources: Vec<ResourceDef>,
    objects: Mutex<HashMap<Gvk, Vec<Value>>>,
    resource_version: AtomicU64,
    log: Mutex<VecDeque<WatchRecord>>,
    dropped_through: AtomicU64,
    events: broadcast::Sender<WatchRecord>,
}

impl Store {
    /// Takes ownership of a scenario and stamps every seed with the first resourceVersion.
    pub(crate) fn new(scenario: Scenario) -> Self {
        let (events, _) = broadcast::channel(LOG_CAPACITY);
        let store = Self {
            resources: scenario.resources,
            objects: Mutex::new(scenario.objects),
            resource_version: AtomicU64::new(1),
            log: Mutex::new(VecDeque::new()),
            dropped_through: AtomicU64::new(0),
            events,
        };
        store.stamp_seeds();
        store
    }

    /// Applies a change, giving it the next resourceVersion and feeding every watcher.
    pub(crate) fn apply(&self, gvk: &Gvk, event: Event, mut object: Value) -> Value {
        let resource_version = self.resource_version.fetch_add(1, Ordering::SeqCst) + 1;
        set_resource_version(&mut object, &resource_version.to_string());

        let mut objects = self.objects.lock().expect("store lock");
        let items = objects.entry(gvk.clone()).or_default();
        let name = field(&object, "name");
        match event {
            Event::Deleted => {
                if let Some(name) = name {
                    items.retain(|item| field(item, "name").as_deref() != Some(name.as_str()));
                }
            }
            _ => match items
                .iter_mut()
                .find(|item| field(item, "name") == name && name.is_some())
            {
                Some(existing) => *existing = object.clone(),
                None => items.push(object.clone()),
            },
        }
        drop(objects);

        let record = WatchRecord {
            gvk: gvk.clone(),
            event,
            resource_version,
            object: object.clone(),
        };
        let mut log = self.log.lock().expect("event log lock");
        log.push_back(record.clone());
        while log.len() > LOG_CAPACITY {
            if let Some(dropped) = log.pop_front() {
                self.dropped_through
                    .fetch_max(dropped.resource_version, Ordering::SeqCst);
            }
        }
        drop(log);

        let _ = self.events.send(record);
        object
    }

    /// Subscribes to live events; subscribe before replaying so nothing falls in the gap.
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<WatchRecord> {
        self.events.subscribe()
    }

    /// Logged events newer than `from`, oldest first.
    pub(crate) fn replay(&self, gvk: &Gvk, from: u64) -> Vec<WatchRecord> {
        let log = self.log.lock().expect("event log lock");
        log.iter()
            .filter(|record| &record.gvk == gvk && record.resource_version > from)
            .cloned()
            .collect()
    }

    /// True when the log no longer reaches back to `resource_version`.
    pub(crate) fn is_stale(&self, resource_version: u64) -> bool {
        resource_version != 0 && resource_version < self.dropped_through.load(Ordering::SeqCst)
    }

    fn stamp_seeds(&self) {
        let mut objects = self.objects.lock().expect("store lock");
        for items in objects.values_mut() {
            for item in items.iter_mut() {
                set_resource_version(item, "1");
            }
        }
    }

    /// The current resourceVersion, shared by every list response.
    pub(crate) fn resource_version(&self) -> String {
        self.resource_version.load(Ordering::SeqCst).to_string()
    }

    /// The served definition for a group/version/plural triple.
    pub(crate) fn resource(
        &self,
        group: &str,
        version: &str,
        plural: &str,
    ) -> Option<&ResourceDef> {
        self.resources
            .iter()
            .find(|def| def.group == group && def.version == version && def.plural == plural)
    }

    /// Every definition served for a group, ordered as they were seeded.
    pub(crate) fn group_versions(&self, group: &str) -> Vec<&ResourceDef> {
        self.resources
            .iter()
            .filter(|def| def.group == group)
            .collect()
    }

    /// A consistent clone of the objects stored under one resource.
    pub(crate) fn snapshot(&self, def: &ResourceDef) -> Vec<Value> {
        let objects = self.objects.lock().expect("store lock");
        objects.get(&def.gvk()).cloned().unwrap_or_default()
    }
}

/// Writes `metadata.resourceVersion`, creating `metadata` when the object has none.
fn set_resource_version(object: &mut Value, resource_version: &str) {
    let Some(object) = object.as_object_mut() else {
        return;
    };
    let metadata = object
        .entry("metadata")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(metadata) = metadata.as_object_mut() {
        metadata.insert(
            "resourceVersion".to_owned(),
            Value::String(resource_version.to_owned()),
        );
    }
}

/// Reads a string field from `metadata`.
fn field(item: &Value, key: &str) -> Option<String> {
    item.pointer(&format!("/metadata/{key}"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Builds the `metadata` block returned by list responses.
pub(crate) fn list_meta(resource_version: &str, continue_token: Option<&str>) -> Value {
    let mut meta = Map::new();
    meta.insert(
        "resourceVersion".to_owned(),
        Value::String(resource_version.to_owned()),
    );
    if let Some(token) = continue_token {
        meta.insert("continue".to_owned(), Value::String(token.to_owned()));
    }
    json!(meta)
}
