//! Watch: ordering, resume, expiry, bookmarks and selector filtering.

mod common;

use futures_util::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, WatchParams};
use kube::core::WatchEvent;
use openkite_engine_testkit::{Event, ResourceDef};
use serde_json::{json, Value};

const EVENT_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

fn pod(name: &str) -> Value {
    json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {"name": name, "namespace": "apps"},
    })
}

async fn next_event<S>(stream: &mut S) -> WatchEvent<Pod>
where
    S: futures_util::Stream<Item = Result<WatchEvent<Pod>, kube::Error>> + Unpin,
{
    tokio::time::timeout(EVENT_WAIT, stream.next())
        .await
        .expect("an event within the deadline")
        .expect("a stream item")
        .expect("a decode result")
}

fn kind_of(event: &WatchEvent<Pod>) -> &'static str {
    match event {
        WatchEvent::Added(_) => "ADDED",
        WatchEvent::Modified(_) => "MODIFIED",
        WatchEvent::Deleted(_) => "DELETED",
        WatchEvent::Bookmark(_) => "BOOKMARK",
        WatchEvent::Error(_) => "ERROR",
    }
}

#[tokio::test]
async fn events_arrive_in_order() {
    let (server, pods): (_, ResourceDef) = common::with_pods(0).await;
    let api: Api<Pod> = Api::all(server.client());
    let mut stream = Box::pin(
        api.watch(&WatchParams::default(), "0")
            .await
            .expect("watch"),
    );

    server.apply(&pods, Event::Added, pod("pod-a"));
    server.apply(&pods, Event::Modified, pod("pod-a"));
    server.apply(&pods, Event::Deleted, pod("pod-a"));

    let mut seen = Vec::new();
    for _ in 0..3 {
        let event = next_event(&mut stream).await;
        seen.push(kind_of(&event));
    }
    assert_eq!(seen, ["ADDED", "MODIFIED", "DELETED"]);

    server.shutdown().await;
}

#[tokio::test]
async fn resume_from_resource_version_replays_only_newer_events() {
    let (server, pods) = common::with_pods(0).await;
    server.apply(&pods, Event::Added, pod("pod-a"));
    let resume = server.resource_version();
    server.apply(&pods, Event::Added, pod("pod-b"));

    let api: Api<Pod> = Api::all(server.client());
    let mut stream = Box::pin(
        api.watch(&WatchParams::default(), &resume)
            .await
            .expect("watch"),
    );

    let event = next_event(&mut stream).await;
    match event {
        WatchEvent::Added(pod) => assert_eq!(pod.metadata.name.as_deref(), Some("pod-b")),
        other => panic!("expected the replayed pod-b, got {}", kind_of(&other)),
    }
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(300), stream.next())
            .await
            .is_err(),
        "only the newer event is replayed"
    );

    server.shutdown().await;
}

#[tokio::test]
async fn a_stale_resource_version_is_expired() {
    let (server, pods) = common::with_pods(0).await;
    for index in 0..300 {
        server.apply(&pods, Event::Added, pod(&format!("pod-{index:03}")));
    }

    let api: Api<Pod> = Api::all(server.client());
    let error = match api.watch(&WatchParams::default(), "1").await {
        Ok(_) => panic!("a resourceVersion beyond the log is refused"),
        Err(error) => error,
    };
    match error {
        kube::Error::Api(status) => {
            assert_eq!(status.code, 410);
            assert_eq!(status.reason, "Expired");
        }
        other => panic!("expected an Expired status, got {other}"),
    }

    server.shutdown().await;
}

#[tokio::test]
async fn a_bookmark_arrives_and_the_timeout_closes_the_stream() {
    let (server, _) = common::with_pods(0).await;
    let api: Api<Pod> = Api::all(server.client());
    let mut stream = Box::pin(
        api.watch(&WatchParams::default().timeout(1), "0")
            .await
            .expect("watch"),
    );

    let event = next_event(&mut stream).await;
    assert_eq!(kind_of(&event), "BOOKMARK");

    let ended = tokio::time::timeout(EVENT_WAIT, stream.next()).await;
    assert!(
        matches!(ended, Ok(None)),
        "the stream closes after the timeout"
    );

    server.shutdown().await;
}

#[tokio::test]
async fn a_field_selector_filters_the_watch() {
    let (server, pods) = common::with_pods(0).await;
    let api: Api<Pod> = Api::all(server.client());
    let mut stream = Box::pin(
        api.watch(&WatchParams::default().fields("metadata.name=pod-01"), "0")
            .await
            .expect("watch"),
    );

    server.apply(&pods, Event::Added, pod("pod-02"));
    server.apply(&pods, Event::Added, pod("pod-01"));

    let event = next_event(&mut stream).await;
    match event {
        WatchEvent::Added(pod) => assert_eq!(pod.metadata.name.as_deref(), Some("pod-01")),
        other => panic!("expected pod-01, got {}", kind_of(&other)),
    }

    server.shutdown().await;
}
