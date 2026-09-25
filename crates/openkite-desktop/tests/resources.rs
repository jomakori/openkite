//! Integration tests for the reflector-backed resource state wiring.

use std::sync::{Arc, Mutex};

use k8s_openapi::api::core::v1::{ConfigMap, Service, ServicePort, ServiceSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::runtime::reflector::store;
use kube::runtime::watcher;
use openkite::state::resources::drive_reflector;
use serde_json::{json, Value};

fn config_map(name: &str) -> ConfigMap {
    ConfigMap {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[tokio::test]
async fn apply_events_update_store_and_republish_snapshot() {
    let (store, writer) = store::<ConfigMap>();
    let snapshot_lens = Arc::new(Mutex::new(Vec::<usize>::new()));

    let events = futures::stream::iter(vec![
        Ok(watcher::Event::Apply(config_map("a"))),
        Ok(watcher::Event::Apply(config_map("b"))),
    ]);

    let captured = snapshot_lens.clone();
    drive_reflector(writer, events, store.clone(), move |rows| {
        captured.lock().unwrap().push(rows.len());
    })
    .await;

    assert_eq!(
        store.state().len(),
        2,
        "both Apply events land in the store"
    );

    let lens = snapshot_lens.lock().unwrap();
    assert!(
        !lens.is_empty(),
        "snapshot callback must fire at least once"
    );
    assert_eq!(
        *lens.last().unwrap(),
        2,
        "final snapshot reflects both objects"
    );
}

#[tokio::test]
async fn delete_event_removes_from_store_and_republishes() {
    let (store, writer) = store::<ConfigMap>();
    let snapshot_lens = Arc::new(Mutex::new(Vec::<usize>::new()));

    let events = futures::stream::iter(vec![
        Ok(watcher::Event::Apply(config_map("a"))),
        Ok(watcher::Event::Apply(config_map("b"))),
        Ok(watcher::Event::Delete(config_map("a"))),
    ]);

    let captured = snapshot_lens.clone();
    drive_reflector(writer, events, store.clone(), move |rows| {
        captured.lock().unwrap().push(rows.len());
    })
    .await;

    assert_eq!(store.state().len(), 1, "deleted object leaves the store");
    let lens = snapshot_lens.lock().unwrap();
    assert_eq!(*lens.last().unwrap(), 1, "final snapshot drops deleted row");
}

#[tokio::test]
async fn empty_stream_never_fires_snapshot() {
    let (store, writer) = store::<ConfigMap>();
    let snapshot_lens = Arc::new(Mutex::new(0usize));
    let events = futures::stream::iter(Vec::<watcher::Result<watcher::Event<ConfigMap>>>::new());

    let captured = snapshot_lens.clone();
    drive_reflector(writer, events, store.clone(), move |_rows| {
        *captured.lock().unwrap() += 1;
    })
    .await;

    assert!(store.state().is_empty());
    assert_eq!(
        *snapshot_lens.lock().unwrap(),
        0,
        "no events → no callbacks"
    );
}

#[tokio::test]
async fn error_events_log_and_skip_snapshot_but_keep_stream_alive() {
    let (store, writer) = store::<ConfigMap>();
    let snapshot_lens = Arc::new(Mutex::new(Vec::<usize>::new()));

    // A transient watch error followed by a healthy Apply: the error must
    // not kill the loop (reflector reconnects upstream), and only the
    // Apply republishes a snapshot.
    let events = futures::stream::iter(vec![
        Err(watcher::Error::WatchError(Box::new(kube::core::Status {
            code: 410,
            reason: "Expired".to_string(),
            message: "watch reset".to_string(),
            ..Default::default()
        }))),
        Ok(watcher::Event::Apply(config_map("a"))),
    ]);

    let captured = snapshot_lens.clone();
    drive_reflector(writer, events, store.clone(), move |rows| {
        captured.lock().unwrap().push(rows.len());
    })
    .await;

    assert_eq!(store.state().len(), 1, "Apply after error still lands");
    let lens = snapshot_lens.lock().unwrap();
    assert_eq!(lens.as_slice(), &[1], "only the Apply fires a snapshot");
}

/// Service fixture: the row the reflector seeds and the push channel serialises.
fn service_fixture() -> Service {
    Service {
        metadata: ObjectMeta {
            name: Some("openkite-web".to_string()),
            namespace: Some("default".to_string()),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            cluster_ip: Some("10.96.0.42".to_string()),
            ports: Some(vec![ServicePort {
                name: Some("http".to_string()),
                port: 80,
                protocol: Some("TCP".to_string()),
                target_port: Some(IntOrString::Int(8080)),
                ..Default::default()
            }]),
            selector: Some(
                [("app".to_string(), "openkite-web".to_string())]
                    .into_iter()
                    .collect(),
            ),
            type_: Some("NodePort".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[tokio::test]
async fn service_events_seed_the_store_and_publish_the_fixture_manifest() {
    let (store, writer) = store::<Service>();
    let published = Arc::new(Mutex::new(Vec::<Vec<Value>>::new()));

    let events = futures::stream::iter(vec![Ok(watcher::Event::Apply(service_fixture()))]);

    let captured = published.clone();
    drive_reflector(writer, events, store.clone(), move |rows| {
        captured.lock().unwrap().push(
            rows.iter()
                .map(|obj| serde_json::to_value(obj.as_ref()).unwrap())
                .collect::<Vec<_>>(),
        );
    })
    .await;

    assert_eq!(
        store.state().len(),
        1,
        "the seeded service lands in the store"
    );
    let manifest = published
        .lock()
        .unwrap()
        .last()
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        manifest,
        vec![json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {"name": "openkite-web", "namespace": "default"},
            "spec": {
                "clusterIP": "10.96.0.42",
                "ports": [{"name": "http", "port": 80, "protocol": "TCP", "targetPort": 8080}],
                "selector": {"app": "openkite-web"},
                "type": "NodePort",
            },
        })],
        "the published row is the fixture manifest"
    );
}
