//! Integration tests for the reflector-backed resource state wiring.

use std::sync::{Arc, Mutex};

use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::runtime::reflector::store;
use kube::runtime::watcher;
use openkite::state::resources::drive_reflector;

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
        Err(kube::Error::Api(kube::core::ErrorResponse {
            status: "Failure".into(),
            message: "watch reset".into(),
            reason: "Expired".into(),
            code: 410,
        })),
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
