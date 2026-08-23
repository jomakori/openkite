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
