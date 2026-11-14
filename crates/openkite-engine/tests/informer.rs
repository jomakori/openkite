//! Acceptance tests for the informer supervisor, against the fake apiserver.
use std::time::Duration;

use kube::core::GroupVersionKind;
use openkite_engine::informer::{InformerEvent, InformerSupervisor};
use openkite_engine::interest::{Interest, InterestClass, Scope};
use openkite_engine_testkit::{Event, FakeApiserver, ResourceDef, Scenario};
use serde_json::{json, Value};

fn pods_def() -> ResourceDef {
    ResourceDef::new("", "v1", "Pod", "pods", true)
}

fn pod(name: &str) -> Value {
    json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {"name": name, "namespace": "apps"},
    })
}

fn pod_interest(class: InterestClass) -> Interest {
    Interest {
        gvk: GroupVersionKind {
            group: String::new(),
            version: "v1".to_owned(),
            kind: "Pod".to_owned(),
        },
        scope: Scope::Namespace("apps".to_owned()),
        selector: None,
        class,
    }
}

async fn start() -> (FakeApiserver, ResourceDef) {
    let pods = pods_def();
    let scenario = Scenario::new().with_resource(pods.clone());
    (FakeApiserver::start(scenario).await, pods)
}

#[tokio::test]
async fn two_views_share_one_watcher() {
    let (server, _pods) = start().await;
    let supervisor = InformerSupervisor::new(server.client());

    let first = supervisor
        .subscribe(pod_interest(InterestClass::Visible))
        .await;
    let second = supervisor
        .subscribe(pod_interest(InterestClass::Visible))
        .await;
    assert_eq!(
        supervisor.informer_count().await,
        1,
        "two subscribers share one informer"
    );

    drop(first);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        supervisor.informer_count().await,
        1,
        "one remaining subscriber keeps it alive"
    );

    drop(second);
    server.shutdown().await;
}

#[tokio::test]
async fn last_drop_parks_after_grace() {
    let (server, _pods) = start().await;
    let supervisor = InformerSupervisor::new(server.client());

    let sub = supervisor
        .subscribe(pod_interest(InterestClass::Visible))
        .await;
    assert_eq!(supervisor.informer_count().await, 1);

    drop(sub);
    // Visible grace is 5s; wait with margin for the park timer and handle removal.
    tokio::time::sleep(Duration::from_secs(6)).await;

    assert_eq!(
        supervisor.informer_count().await,
        0,
        "the informer parks and its handle is removed"
    );
    server.shutdown().await;
}

#[tokio::test]
async fn stale_watch_relists_once_and_resets() {
    let (server, pods) = start().await;
    let supervisor = InformerSupervisor::new(server.client());

    let mut sub = supervisor
        .subscribe(pod_interest(InterestClass::Pinned))
        .await;
    // Let the informer's initial list land and the watch connect before the log is evicted under it.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // The watcher is now watching from an early resourceVersion; evict it from the log so the
    // next resume (triggered by the 1s watch timeout) comes back as a 410.
    for index in 0..300 {
        server.apply(&pods, Event::Added, pod(&format!("eviction-{index:03}")));
    }

    let mut resets = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(100), sub.next()).await {
            Ok(Some(InformerEvent::Reset)) => resets += 1,
            Ok(Some(_)) => {}
            _ => {}
        }
    }
    assert_eq!(
        resets, 1,
        "exactly one Reset must arrive from the single relist"
    );

    server.apply(&pods, Event::Added, pod("post-reset"));
    let mut saw_live = false;
    let live_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < live_deadline {
        if let Ok(Some(InformerEvent::Upsert(_))) =
            tokio::time::timeout(Duration::from_millis(100), sub.next()).await
        {
            saw_live = true;
            break;
        }
    }
    assert!(saw_live, "live events must flow again after the reset");

    server.shutdown().await;
}
