//! List responses: paging through `limit` and `continue`.

mod common;

use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, ListParams};

/// Reads every pod across all namespaces, following continue tokens.
async fn drain(client: kube::Client, page: u32) -> Vec<String> {
    let api: Api<Pod> = Api::all(client);
    let mut params = ListParams::default().limit(page);
    let mut names = Vec::new();
    loop {
        let list = api.list(&params).await.expect("list");
        for pod in list.items {
            names.push(pod.metadata.name.expect("pod name"));
        }
        match list.metadata.continue_ {
            Some(token) if !token.is_empty() => {
                params = ListParams::default().limit(page).continue_token(&token);
            }
            _ => break,
        }
    }
    names
}

#[tokio::test]
async fn a_single_page_returns_everything() {
    let (server, _) = common::with_pods(5).await;
    let names = drain(server.client(), 100).await;

    assert_eq!(names.len(), 5);
    assert!(names.contains(&"pod-00".to_owned()));
    assert!(names.contains(&"pod-04".to_owned()));
    server.shutdown().await;
}

#[tokio::test]
async fn limit_and_continue_walk_every_object_once() {
    let (server, _) = common::with_pods(25).await;
    let mut names = drain(server.client(), 10).await;
    names.sort();

    let expected: Vec<String> = (0..25).map(|index| format!("pod-{index:02}")).collect();
    assert_eq!(names, expected);
    server.shutdown().await;
}

#[tokio::test]
async fn the_last_page_carries_no_continue_token() {
    let (server, _) = common::with_pods(3).await;
    let client = server.client();
    let api: Api<Pod> = Api::all(client);

    let first = api
        .list(&ListParams::default().limit(3))
        .await
        .expect("list");
    assert_eq!(first.items.len(), 3);
    assert!(first
        .metadata
        .continue_
        .as_deref()
        .is_none_or(str::is_empty));

    server.shutdown().await;
}

#[tokio::test]
async fn a_namespaced_list_only_sees_its_namespace() {
    let (server, _) = common::with_pods(4).await;
    let client = server.client();
    let api: Api<Pod> = Api::namespaced(client, "apps");

    let list = api
        .list(&ListParams::default())
        .await
        .expect("namespaced list");
    assert_eq!(list.items.len(), 4);

    let other: Api<Pod> = Api::namespaced(server.client(), "elsewhere");
    let empty = other
        .list(&ListParams::default())
        .await
        .expect("empty namespaced list");
    assert!(empty.items.is_empty());

    server.shutdown().await;
}
