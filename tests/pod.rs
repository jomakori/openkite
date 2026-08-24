//! Integration tests for pod detail mapping.

use k8s_openapi::api::core::v1::{
    Container, ContainerState, ContainerStateRunning, ContainerStateWaiting, ContainerStatus,
    Event, Pod, PodSpec, PodStatus,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};

use openkite::pod::{container_infos, pod_summary, sort_events};

fn running() -> ContainerState {
    ContainerState {
        running: Some(ContainerStateRunning::default()),
        ..Default::default()
    }
}

fn pod(
    containers: Vec<Container>,
    statuses: Vec<ContainerStatus>,
    phase: Option<&str>,
    node: Option<&str>,
) -> Pod {
    Pod {
        metadata: ObjectMeta::default(),
        spec: Some(PodSpec {
            containers,
            node_name: node.map(str::to_string),
            ..Default::default()
        }),
        status: Some(PodStatus {
            phase: phase.map(str::to_string),
            container_statuses: Some(statuses),
            ..Default::default()
        }),
    }
}

#[test]
fn container_infos_join_spec_and_status() {
    let p = pod(
        vec![
            Container {
                name: "app".into(),
                image: Some("nginx:1.25".into()),
                ..Default::default()
            },
            Container {
                name: "sidecar".into(),
                image: None,
                ..Default::default()
            },
        ],
        vec![
            ContainerStatus {
                name: "app".into(),
                image: "nginx:1.25".into(),
                ready: true,
                restart_count: 3,
                state: Some(running()),
                ..Default::default()
            },
            ContainerStatus {
                name: "sidecar".into(),
                image: "busybox".into(),
                ready: false,
                restart_count: 0,
                state: Some(ContainerState {
                    waiting: Some(ContainerStateWaiting {
                        reason: Some("CrashLoopBackOff".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ],
        Some("Running"),
        Some("node-1"),
    );

    let infos = container_infos(&p);
    assert_eq!(infos.len(), 2);

    assert_eq!(infos[0].name, "app");
    assert_eq!(infos[0].image, "nginx:1.25");
    assert!(infos[0].ready);
    assert_eq!(infos[0].restarts, 3);
    assert_eq!(infos[0].state, "Running");

    assert_eq!(infos[1].name, "sidecar");
    assert_eq!(infos[1].image, "");
    assert!(!infos[1].ready);
    assert_eq!(infos[1].state, "Waiting: CrashLoopBackOff");
}

#[test]
fn container_without_status_is_pending() {
    let p = pod(
        vec![Container {
            name: "app".into(),
            image: Some("nginx".into()),
            ..Default::default()
        }],
        vec![],
        Some("Pending"),
        None,
    );
    let infos = container_infos(&p);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].state, "Pending");
    assert_eq!(infos[0].restarts, 0);
    assert!(!infos[0].ready);
}

#[test]
fn pod_summary_extracts_overview_fields() {
    let p = pod(vec![], vec![], Some("Running"), Some("node-2"));
    let s = pod_summary(&p);
    assert_eq!(s.phase, "Running");
    assert_eq!(s.node, "node-2");
    assert_eq!(s.pod_ip, "");
    assert_eq!(s.qos, "");
    assert_eq!(s.reason, None);
}

#[test]
fn pod_without_status_is_unknown() {
    let p = pod(vec![], vec![], None, None);
    let s = pod_summary(&p);
    assert_eq!(s.phase, "Unknown");
    assert_eq!(s.node, "");
}

#[test]
fn sort_events_orders_newest_first_with_none_last() {
    let ts = |secs: i64| Time::from(jiff::Timestamp::from_second(secs).unwrap());
    let ev = |name: &str, ts: Option<Time>| Event {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            ..Default::default()
        },
        last_timestamp: ts,
        ..Default::default()
    };

    let mut events = vec![
        ev("old", Some(ts(100))),
        ev("none", None),
        ev("new", Some(ts(300))),
        ev("mid", Some(ts(200))),
    ];
    sort_events(&mut events);

    let names: Vec<String> = events
        .iter()
        .map(|e| e.metadata.name.clone().unwrap())
        .collect();
    assert_eq!(names, vec!["new", "mid", "old", "none"]);
}
