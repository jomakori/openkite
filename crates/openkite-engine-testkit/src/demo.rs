//! A seeded cluster used by both the fixture-backed web shell and the tests that assert
//! on it: the same objects a reviewer sees in a preview are the ones the checks read.

use k8s_openapi::jiff::Timestamp;
use serde_json::{json, Value};

use crate::{ResourceDef, Scenario};

/// Namespace every seeded workload lives in.
pub const NAMESPACE: &str = "demo";

/// The kinds the console reads, one definition each.
struct Kinds {
    namespaces: ResourceDef,
    nodes: ResourceDef,
    pods: ResourceDef,
    events: ResourceDef,
    deployments: ResourceDef,
    applications: ResourceDef,
}

impl Kinds {
    fn new() -> Self {
        Self {
            namespaces: ResourceDef::new("", "v1", "Namespace", "namespaces", false),
            nodes: ResourceDef::new("", "v1", "Node", "nodes", false),
            pods: ResourceDef::new("", "v1", "Pod", "pods", true),
            events: ResourceDef::new("", "v1", "Event", "events", true),
            deployments: ResourceDef::new("apps", "v1", "Deployment", "deployments", true),
            applications: ResourceDef::new(
                "argoproj.io",
                "v1alpha1",
                "Application",
                "applications",
                true,
            ),
        }
    }
}

/// A small cluster: three namespaces, two nodes, four deployments, six pods across the
/// phases the console colours differently, two events, one ArgoCD application.
pub fn scenario() -> Scenario {
    let kinds = Kinds::new();
    let mut scenario = Scenario::new()
        .with_resource(kinds.namespaces.clone())
        .with_resource(kinds.nodes.clone())
        .with_resource(kinds.pods.clone())
        .with_resource(kinds.events.clone())
        .with_resource(kinds.deployments.clone())
        .with_resource(kinds.applications.clone());

    for name in ["demo", "kube-system", "argocd"] {
        scenario = scenario.add(
            &kinds.namespaces,
            json!({
                "apiVersion": "v1",
                "kind": "Namespace",
                "metadata": {"name": name, "creationTimestamp": "2026-01-04T09:00:00Z"},
                "status": {"phase": "Active"},
            }),
        );
    }

    for (name, cpu, memory, ready) in [
        ("demo-a", "4", "16Gi", true),
        ("demo-b", "8", "32Gi", false),
    ] {
        scenario = scenario.add(
            &kinds.nodes,
            json!({
                "apiVersion": "v1",
                "kind": "Node",
                "metadata": {"name": name, "creationTimestamp": "2026-01-04T09:02:00Z"},
                "status": {
                    "capacity": {"cpu": cpu, "memory": memory, "pods": "110"},
                    "allocatable": {"cpu": cpu, "memory": memory, "pods": "110"},
                    "conditions": [{"type": "Ready", "status": if ready { "True" } else { "False" }}],
                    "nodeInfo": {"kubeletVersion": "v1.32.1", "osImage": "Ubuntu 24.04.2 LTS"},
                },
            }),
        );
    }

    for (name, workload) in [
        ("api", "checkout"),
        ("worker", "checkout"),
        ("frontend", "storefront"),
        ("cache", "storefront"),
    ] {
        let replicas = if workload == "checkout" { 2 } else { 1 };
        let ready = if name == "cache" { 0 } else { replicas };
        scenario = scenario.add(
            &kinds.deployments,
            json!({
                "apiVersion": "apps/v1",
                "kind": "Deployment",
                "metadata": {
                    "name": name,
                    "namespace": NAMESPACE,
                    "creationTimestamp": "2026-01-04T09:05:00Z",
                },
                "spec": {"replicas": replicas, "selector": {"matchLabels": {"app": name}}},
                "status": {"replicas": replicas, "readyReplicas": ready, "availableReplicas": ready},
            }),
        );
    }

    for (index, (name, phase, restarts, ready)) in [
        ("api-7d9f-abcde", "Running", 0, true),
        ("api-7d9f-fghij", "Running", 2, true),
        ("worker-5b6c-klmno", "Running", 0, true),
        ("frontend-8c4d-pqrst", "Pending", 0, false),
        ("cache-2f1a-uvwxy", "Running", 7, false),
        ("migration-9a8b-zabcd", "Failed", 0, false),
    ]
    .into_iter()
    .enumerate()
    {
        let node = if index % 2 == 0 { "demo-a" } else { "demo-b" };
        let workload = name.split('-').next().unwrap_or(name);
        let state = match phase {
            "Running" => json!({"running": {"startedAt": "2026-01-04T09:10:05Z"}}),
            "Pending" => json!({"waiting": {"reason": "ContainerCreating"}}),
            _ => json!({"terminated": {"reason": "Error", "exitCode": 1}}),
        };
        scenario = scenario.add(
            &kinds.pods,
            json!({
                "apiVersion": "v1",
                "kind": "Pod",
                "metadata": {
                    "name": name,
                    "namespace": NAMESPACE,
                    "uid": pod_uid(index),
                    "creationTimestamp": "2026-01-04T09:10:00Z",
                    "labels": {"app": workload},
                },
                "spec": {
                    "nodeName": node,
                    "containers": [{
                        "name": "app",
                        "image": format!("ghcr.io/jomakori/openkite:{workload}"),
                        "resources": {
                            "requests": {"cpu": "250m", "memory": "256Mi"},
                            "limits": {"memory": "512Mi"},
                        },
                    }],
                },
                "status": {
                    "phase": phase,
                    "startTime": "2026-01-04T09:10:05Z",
                    "containerStatuses": [{
                        "name": "app",
                        "ready": ready,
                        "restartCount": restarts,
                        "image": format!("ghcr.io/jomakori/openkite:{workload}"),
                        "state": state,
                    }],
                },
            }),
        );
    }

    // Events from the last few minutes. The kubelet re-emits one back-off per retry, so three
    // raw Events for one pod must reach the list as a single row carrying their counts; the
    // scheduling failure stays separate because its cause is different.
    for (index, minutes) in [(0usize, 4), (1, 2), (2, 0)] {
        scenario = scenario.add(
            &kinds.events,
            json!({
                "apiVersion": "v1",
                "kind": "Event",
                "metadata": {
                    "name": format!("cache-2f1a-uvwxy.{index:04}"),
                    "namespace": NAMESPACE,
                    "uid": format!("ev-backoff-{index}"),
                },
                "type": "Warning",
                "reason": "BackOff",
                "message": "Back-off restarting failed container app in pod cache-2f1a-uvwxy_demo",
                "count": index as i32 + 1,
                "firstTimestamp": minutes_ago(9),
                "lastTimestamp": minutes_ago(minutes),
                "involvedObject": {
                    "apiVersion": "v1",
                    "kind": "Pod",
                    "namespace": NAMESPACE,
                    "name": "cache-2f1a-uvwxy",
                    "uid": pod_uid(4),
                },
            }),
        );
    }

    scenario = scenario.add(
        &kinds.events,
        json!({
            "apiVersion": "v1",
            "kind": "Event",
            "metadata": {
                "name": "frontend-8c4d-pqrst.17f1",
                "namespace": NAMESPACE,
                "uid": "ev-scheduling-0",
            },
            "type": "Normal",
            "reason": "FailedScheduling",
            "message": "0/2 nodes are available: insufficient cpu",
            "count": 1,
            "firstTimestamp": minutes_ago(3),
            "lastTimestamp": minutes_ago(3),
            "involvedObject": {
                "apiVersion": "v1",
                "kind": "Pod",
                "namespace": NAMESPACE,
                "name": "frontend-8c4d-pqrst",
                "uid": pod_uid(3),
            },
        }),
    );

    scenario.add(
        &kinds.applications,
        application("checkout", "Synced", "Healthy"),
    )
}

/// Deterministic pod uid, so events can point at a pod the fixture also seeds.
fn pod_uid(index: usize) -> String {
    format!("00000000-0000-4000-8000-{index:012}")
}

/// RFC3339 timestamp `minutes` before the moment the fixture boots, so a seeded event is
/// always inside the aggregation window.
fn minutes_ago(minutes: u64) -> String {
    let then = Timestamp::now() - std::time::Duration::from_secs(minutes * 60);
    then.to_string()
}

/// One ArgoCD application as the console's Argo panel reads it.
fn application(name: &str, sync: &str, health: &str) -> Value {
    json!({
        "apiVersion": "argoproj.io/v1alpha1",
        "kind": "Application",
        "metadata": {"name": format!("demo-{name}"), "namespace": "argocd"},
        "spec": {
            "source": {
                "repoURL": "https://github.com/jomakori/openkite",
                "targetRevision": "main",
                "path": format!("deploy/{name}"),
            },
            "destination": {"namespace": NAMESPACE},
        },
        "status": {
            "sync": {"status": sync},
            "health": {"status": health},
        },
    })
}
