//! Integration tests for the reflectors behind the Config & Storage section:
//! ConfigMaps, PersistentVolumeClaims and Ingresses.
//!
//! Every kind drives the real `drive_reflector` wiring over a mock watch
//! stream, and asserts the published snapshot is the seeded manifest field for
//! field — the fixture builder and the reflector writer have to agree on the
//! document, since the row mapping reads `apiVersion`/`kind`/`metadata` by name.

use std::fmt::Debug;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use k8s_openapi::api::core::v1::{
    ConfigMap, PersistentVolumeClaim, PersistentVolumeClaimSpec, PersistentVolumeClaimStatus,
};
use k8s_openapi::api::networking::v1::{Ingress, IngressRule, IngressSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::runtime::reflector::store;
use kube::runtime::watcher;
use kube::Resource;
use openkite::state::resources::drive_reflector;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

/// Drive `seeds` through the reflector as if the API server had sent them, and
/// return the last snapshot the views would receive.
async fn published<T>(seeds: Vec<T>) -> Vec<Value>
where
    T: Resource + Clone + DeserializeOwned + Debug + Send + Sync + Serialize + 'static,
    T::DynamicType: Eq + Hash + Clone + Default,
{
    let (store, writer) = store::<T>();
    let events: Vec<watcher::Result<watcher::Event<T>>> = seeds
        .into_iter()
        .map(|seed| Ok(watcher::Event::Apply(seed)))
        .collect();

    let captured = Arc::new(Mutex::new(Vec::<Value>::new()));
    let sink = captured.clone();
    drive_reflector(
        writer,
        futures::stream::iter(events),
        store.clone(),
        move |rows| {
            *sink.lock().unwrap() = rows
                .iter()
                .filter_map(|row| serde_json::to_value(row.as_ref()).ok())
                .collect();
        },
    )
    .await;

    last_snapshot(&captured)
}

/// Last snapshot the reflector callback wrote into `cell`.
fn last_snapshot(cell: &Mutex<Vec<Value>>) -> Vec<Value> {
    cell.lock().unwrap().clone()
}

fn meta(name: &str, namespace: &str) -> ObjectMeta {
    ObjectMeta {
        name: Some(name.to_string()),
        namespace: Some(namespace.to_string()),
        ..Default::default()
    }
}

fn config_map(name: &str, namespace: &str) -> ConfigMap {
    ConfigMap {
        metadata: meta(name, namespace),
        data: Some([("log.level".to_string(), "info".to_string())].into()),
        ..Default::default()
    }
}

fn persistent_volume_claim(name: &str, namespace: &str) -> PersistentVolumeClaim {
    PersistentVolumeClaim {
        metadata: meta(name, namespace),
        spec: Some(PersistentVolumeClaimSpec {
            storage_class_name: Some("local-path".to_string()),
            volume_name: Some(format!("pvc-{name}")),
            ..Default::default()
        }),
        status: Some(PersistentVolumeClaimStatus {
            phase: Some("Bound".to_string()),
            ..Default::default()
        }),
    }
}

fn ingress(name: &str, namespace: &str) -> Ingress {
    Ingress {
        metadata: meta(name, namespace),
        spec: Some(IngressSpec {
            ingress_class_name: Some("istio".to_string()),
            rules: Some(vec![IngressRule {
                host: Some(format!("{name}.maklab.net")),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        status: None,
    }
}

#[tokio::test]
async fn config_map_watch_publishes_the_seeded_manifest() {
    let rows = published(vec![config_map("openkite-config", "default")]).await;

    assert_eq!(
        rows,
        vec![json!({
            "apiVersion": "v1",
            "kind": "ConfigMap",
            "metadata": { "name": "openkite-config", "namespace": "default" },
            "data": { "log.level": "info" },
        })],
        "the reflector publishes the manifest the fixture seeds"
    );
}

#[tokio::test]
async fn persistent_volume_claim_watch_publishes_the_seeded_manifest() {
    let rows = published(vec![persistent_volume_claim("postgres-data", "default")]).await;

    assert_eq!(
        rows,
        vec![json!({
            "apiVersion": "v1",
            "kind": "PersistentVolumeClaim",
            "metadata": { "name": "postgres-data", "namespace": "default" },
            "spec": {
                "storageClassName": "local-path",
                "volumeName": "pvc-postgres-data",
            },
            "status": { "phase": "Bound" },
        })],
        "the reflector publishes the manifest the fixture seeds"
    );
}

#[tokio::test]
async fn ingress_watch_publishes_the_seeded_manifest() {
    let rows = published(vec![ingress("openkite-api", "default")]).await;

    assert_eq!(
        rows,
        vec![json!({
            "apiVersion": "networking.k8s.io/v1",
            "kind": "Ingress",
            "metadata": { "name": "openkite-api", "namespace": "default" },
            "spec": {
                "ingressClassName": "istio",
                "rules": [{ "host": "openkite-api.maklab.net" }],
            },
        })],
        "the reflector publishes the manifest the fixture seeds"
    );
}

#[tokio::test]
async fn deleting_a_claim_republishes_the_remaining_claims_only() {
    let (store, writer) = store::<PersistentVolumeClaim>();
    let events = futures::stream::iter(vec![
        Ok(watcher::Event::Apply(persistent_volume_claim(
            "postgres-data",
            "default",
        ))),
        Ok(watcher::Event::Apply(persistent_volume_claim(
            "openkite-api-data",
            "default",
        ))),
        Ok(watcher::Event::Delete(persistent_volume_claim(
            "postgres-data",
            "default",
        ))),
    ]);

    let names = Arc::new(Mutex::new(Vec::<String>::new()));
    let captured = names.clone();
    drive_reflector(writer, events, store.clone(), move |rows| {
        *captured.lock().unwrap() = rows
            .iter()
            .filter_map(|row| row.metadata.name.clone())
            .collect();
    })
    .await;

    assert_eq!(
        *names.lock().unwrap(),
        vec!["openkite-api-data".to_string()],
        "the final snapshot drops the deleted claim"
    );
}
