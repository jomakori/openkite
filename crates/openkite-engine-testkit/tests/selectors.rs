//! Field and label selectors.

mod common;

use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, ListParams};

#[tokio::test]
async fn a_field_selector_narows_to_one_name() {
    let (server, _) = common::with_pods(10).await;
    let api: Api<Pod> = Api::all(server.client());

    let list = api
        .list(&ListParams::default().fields("metadata.name=pod-03"))
        .await
        .expect("field selected list");
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].metadata.name.as_deref(), Some("pod-03"));

    server.shutdown().await;
}

#[tokio::test]
async fn a_field_selector_on_a_missing_name_is_empty() {
    let (server, _) = common::with_pods(3).await;
    let api: Api<Pod> = Api::all(server.client());

    let list = api
        .list(&ListParams::default().fields("metadata.name=absent"))
        .await
        .expect("field selected list");
    assert!(list.items.is_empty());

    server.shutdown().await;
}

#[tokio::test]
async fn a_label_selector_matches_the_seeded_label() {
    let (server, _) = common::with_pods(6).await;
    let api: Api<Pod> = Api::all(server.client());

    let matching = api
        .list(&ListParams::default().labels("tier=web"))
        .await
        .expect("label selected list");
    assert_eq!(matching.items.len(), 6);

    let missing = api
        .list(&ListParams::default().labels("tier=db"))
        .await
        .expect("label selected list");
    assert!(missing.items.is_empty());

    server.shutdown().await;
}

#[tokio::test]
async fn a_namespace_field_selector_scopes_the_list() {
    let (server, _) = common::with_pods(2).await;
    let api: Api<Pod> = Api::all(server.client());

    let list = api
        .list(&ListParams::default().fields("metadata.namespace=apps"))
        .await
        .expect("namespace selected list");
    assert_eq!(list.items.len(), 2);

    let other = api
        .list(&ListParams::default().fields("metadata.namespace=other"))
        .await
        .expect("namespace selected list");
    assert!(other.items.is_empty());

    server.shutdown().await;
}
