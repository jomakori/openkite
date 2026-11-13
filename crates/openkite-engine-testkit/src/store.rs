//! In-memory object store: seeded objects and a deterministic resourceVersion.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde_json::{json, Map, Value};

use crate::{ResourceDef, Scenario};

/// Group/version/plural key identifying a served resource.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Gvk {
    pub group: String,
    pub version: String,
    pub plural: String,
}

/// Holds the seeded surface of a fake apiserver.
pub(crate) struct Store {
    pub(crate) resources: Vec<ResourceDef>,
    objects: Mutex<HashMap<Gvk, Vec<Value>>>,
    resource_version: AtomicU64,
}

impl Store {
    /// Takes ownership of a scenario and stamps every seed with the first resourceVersion.
    pub(crate) fn new(scenario: Scenario) -> Self {
        let store = Self {
            resources: scenario.resources,
            objects: Mutex::new(scenario.objects),
            resource_version: AtomicU64::new(1),
        };
        store.stamp_seeds();
        store
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
