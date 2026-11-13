//! Discovery documents: the legacy `APIResourceList` and the aggregated `APIGroupDiscoveryList`.

use serde_json::{json, Map, Value};

use crate::store::Store;
use crate::ResourceDef;

/// The legacy resource list served at `/api/<version>` and `/apis/<group>/<version>`.
pub(crate) fn resource_list(store: &Store, group: &str, version: &str) -> Value {
    let resources: Vec<Value> = store
        .resources
        .iter()
        .filter(|def| def.group == group && def.version == version)
        .map(legacy_resource)
        .collect();

    json!({
        "kind": "APIResourceList",
        "apiVersion": "v1",
        "groupVersion": group_version(group, version),
        "resources": resources,
    })
}

fn legacy_resource(def: &ResourceDef) -> Value {
    let mut entry = Map::new();
    entry.insert("name".to_owned(), json!(def.plural));
    entry.insert("singularName".to_owned(), json!(singular(&def.plural)));
    entry.insert("namespaced".to_owned(), json!(def.namespaced));
    entry.insert("kind".to_owned(), json!(def.kind));
    entry.insert("verbs".to_owned(), json!(def.verbs));
    if !def.short_names.is_empty() {
        entry.insert("shortNames".to_owned(), json!(def.short_names));
    }
    if !def.categories.is_empty() {
        entry.insert("categories".to_owned(), json!(def.categories));
    }
    Value::Object(entry)
}

/// The aggregated discovery list for every non-core group, served at `/apis`.
pub(crate) fn aggregated(store: &Store) -> Value {
    let mut names: Vec<String> = Vec::new();
    for def in &store.resources {
        if def.group.is_empty() {
            continue;
        }
        if !names.contains(&def.group) {
            names.push(def.group.clone());
        }
    }

    let items: Vec<Value> = names
        .iter()
        .map(|name| {
            let mut versions: Vec<String> = Vec::new();
            for def in store.group_versions(name) {
                if !versions.contains(&def.version) {
                    versions.push(def.version.clone());
                }
            }
            let versions: Vec<Value> = versions
                .iter()
                .map(|version| aggregated_version(store, name, version))
                .collect();
            json!({
                "metadata": {"name": name},
                "versions": versions,
            })
        })
        .collect();

    json!({
        "kind": "APIGroupDiscoveryList",
        "apiVersion": "apidiscovery.k8s.io/v2",
        "metadata": {},
        "items": items,
    })
}

/// The aggregated list for the core group, served at `/api`.
pub(crate) fn aggregated_core(store: &Store) -> Value {
    let mut versions: Vec<String> = Vec::new();
    for def in store.group_versions("") {
        if !versions.contains(&def.version) {
            versions.push(def.version.clone());
        }
    }
    let versions: Vec<Value> = versions
        .iter()
        .map(|version| aggregated_version(store, "", version))
        .collect();

    json!({
        "kind": "APIGroupDiscoveryList",
        "apiVersion": "apidiscovery.k8s.io/v2",
        "metadata": {},
        "items": [{"metadata": {"name": ""}, "versions": versions}],
    })
}

fn aggregated_version(store: &Store, group: &str, version: &str) -> Value {
    let resources: Vec<Value> = store
        .group_versions(group)
        .into_iter()
        .filter(|def| def.version == version)
        .map(aggregated_resource)
        .collect();
    json!({"version": version, "resources": resources})
}

fn aggregated_resource(def: &ResourceDef) -> Value {
    let scope = if def.namespaced {
        "Namespaced"
    } else {
        "Cluster"
    };
    json!({
        "resource": def.plural,
        "responseKind": {
            "group": def.group,
            "version": def.version,
            "kind": def.kind,
        },
        "scope": scope,
        "singularResource": singular(&def.plural),
        "verbs": def.verbs,
        "shortNames": def.short_names,
        "categories": def.categories,
    })
}

/// The `APIGroupList` served at `/apis` for clients that do not speak aggregated discovery.
pub(crate) fn group_list(store: &Store) -> Value {
    let mut names: Vec<String> = Vec::new();
    for def in &store.resources {
        if !def.group.is_empty() && !names.contains(&def.group) {
            names.push(def.group.clone());
        }
    }

    let groups: Vec<Value> = names
        .iter()
        .map(|name| {
            let mut versions: Vec<String> = Vec::new();
            for def in store.group_versions(name) {
                if !versions.contains(&def.version) {
                    versions.push(def.version.clone());
                }
            }
            let first = versions.first().cloned().unwrap_or_default();
            json!({
                "name": name,
                "versions": versions
                    .iter()
                    .map(|version| json!({"groupVersion": group_version(name, version), "version": version}))
                    .collect::<Vec<Value>>(),
                "preferredVersion": {"groupVersion": group_version(name, &first), "version": first},
            })
        })
        .collect();

    json!({"kind": "APIGroupList", "apiVersion": "v1", "groups": groups})
}

/// The `APIVersions` served at `/api` for clients that do not speak aggregated discovery.
pub(crate) fn api_versions(store: &Store) -> Value {
    let mut versions: Vec<String> = Vec::new();
    for def in store.group_versions("") {
        if !versions.contains(&def.version) {
            versions.push(def.version.clone());
        }
    }
    json!({
        "kind": "APIVersions",
        "apiVersion": "v1",
        "versions": versions,
        "serverAddressByClientCIDRs": [],
    })
}

fn group_version(group: &str, version: &str) -> String {
    if group.is_empty() {
        version.to_owned()
    } else {
        format!("{group}/{version}")
    }
}

/// Naive singular form, good enough for the kinds a test scenario seeds.
fn singular(plural: &str) -> String {
    if let Some(stem) = plural.strip_suffix("ies") {
        return format!("{stem}y");
    }
    if let Some(stem) = plural.strip_suffix("ses") {
        return plural[..stem.len() + 2].to_owned();
    }
    match plural.strip_suffix('s') {
        Some(stem) if !stem.is_empty() => stem.to_owned(),
        _ => plural.to_owned(),
    }
}
