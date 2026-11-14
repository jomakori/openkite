//! Acceptance tests for row projection: size, strip rules, and the metadata-only path.
use std::collections::BTreeMap;

use k8s_openapi::api::apps::v1::{
    Deployment, DeploymentCondition, DeploymentSpec, DeploymentStatus,
};
use k8s_openapi::api::core::v1::{
    Container, ContainerStatus, Event, Node, NodeCondition, NodeSpec, NodeStatus, ObjectReference,
    Pod, PodCondition, PodSpec, PodStatus, Secret,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{
    FieldsV1, ManagedFieldsEntry, ObjectMeta, Time,
};
use k8s_openapi::jiff::Timestamp;
use k8s_openapi::ByteString;
use openkite_engine::projection::{
    project_deployment, project_event, project_metadata_only, project_node, project_pod,
    project_secret, Row,
};

const LAST_APPLIED_KEY: &str = "kubectl.kubernetes.io/last-applied-configuration";

/// Metadata carrying a populated `managedFields` and a huge last-applied annotation,
/// so the strip is actually proven rather than assumed absent.
fn dirty_metadata(name: &str, namespace: Option<&str>) -> ObjectMeta {
    let mut annotations = BTreeMap::new();
    annotations.insert(
        LAST_APPLIED_KEY.to_owned(),
        format!("{{\"huge\":\"{}\"}}", "x".repeat(4000)),
    );
    annotations.insert("note".to_owned(), "kept-small".to_owned());

    let mut labels = BTreeMap::new();
    labels.insert("app".to_owned(), name.to_owned());

    ObjectMeta {
        name: Some(name.to_owned()),
        namespace: namespace.map(str::to_owned),
        uid: Some(format!("uid-{name}")),
        resource_version: Some("123".to_owned()),
        creation_timestamp: Some(Time(Timestamp::now())),
        labels: Some(labels),
        annotations: Some(annotations),
        managed_fields: Some(vec![ManagedFieldsEntry {
            api_version: Some("v1".to_owned()),
            fields_type: Some("FieldsV1".to_owned()),
            fields_v1: Some(FieldsV1(serde_json::json!({"f:metadata": {}}))),
            manager: Some("kubectl-client-side-apply".to_owned()),
            operation: Some("Update".to_owned()),
            subresource: None,
            time: Some(Time(Timestamp::now())),
        }]),
        ..Default::default()
    }
}

fn pod_fixture() -> Pod {
    Pod {
        metadata: dirty_metadata("web-0", Some("apps")),
        spec: Some(PodSpec {
            node_name: Some("node-a".to_owned()),
            containers: vec![Container {
                name: "web".to_owned(),
                image: Some("registry.example/web:1.0".to_owned()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        status: Some(PodStatus {
            phase: Some("Running".to_owned()),
            start_time: Some(Time(Timestamp::now())),
            conditions: Some(vec![PodCondition {
                type_: "Ready".to_owned(),
                status: "True".to_owned(),
                ..Default::default()
            }]),
            container_statuses: Some(vec![ContainerStatus {
                name: "web".to_owned(),
                image: "registry.example/web:1.0".to_owned(),
                image_id: String::new(),
                ready: true,
                restart_count: 2,
                started: Some(true),
                ..Default::default()
            }]),
            ..Default::default()
        }),
    }
}

fn deployment_fixture() -> Deployment {
    Deployment {
        metadata: dirty_metadata("web", Some("apps")),
        spec: DeploymentSpec {
            replicas: Some(3),
            ..Default::default()
        },
        status: Some(DeploymentStatus {
            ready_replicas: Some(2),
            available_replicas: Some(2),
            updated_replicas: Some(3),
            unavailable_replicas: Some(1),
            conditions: Some(vec![DeploymentCondition {
                type_: Some("Available".to_owned()),
                status: Some("True".to_owned()),
                ..Default::default()
            }]),
            ..Default::default()
        }),
    }
}

fn node_fixture() -> Node {
    let mut allocatable = BTreeMap::new();
    allocatable.insert(
        "cpu".to_owned(),
        k8s_openapi::apimachinery::pkg::api::resource::Quantity("4".to_owned()),
    );

    Node {
        metadata: dirty_metadata("node-a", None),
        spec: Some(NodeSpec {
            unschedulable: Some(false),
            ..Default::default()
        }),
        status: Some(NodeStatus {
            allocatable: Some(allocatable.clone()),
            capacity: Some(allocatable),
            conditions: Some(vec![NodeCondition {
                type_: "Ready".to_owned(),
                status: "True".to_owned(),
                ..Default::default()
            }]),
            ..Default::default()
        }),
    }
}

fn event_fixture() -> Event {
    Event {
        metadata: dirty_metadata("web-0.123", Some("apps")),
        involved_object: ObjectReference {
            kind: Some("Pod".to_owned()),
            name: Some("web-0".to_owned()),
            namespace: Some("apps".to_owned()),
            ..Default::default()
        },
        type_: Some("Warning".to_owned()),
        reason: Some("BackOff".to_owned()),
        count: Some(5),
        ..Default::default()
    }
}

fn secret_fixture() -> Secret {
    let mut data = BTreeMap::new();
    data.insert(
        "password".to_owned(),
        ByteString(b"super-secret-value".to_vec()),
    );
    let mut string_data = BTreeMap::new();
    string_data.insert("extra".to_owned(), "also-secret".to_owned());

    Secret {
        metadata: dirty_metadata("db-creds", Some("apps")),
        data: Some(data),
        string_data: Some(string_data),
        type_: Some("Opaque".to_owned()),
        immutable: None,
    }
}

fn all_rows() -> Vec<Row> {
    vec![
        project_pod(&pod_fixture()),
        project_deployment(&deployment_fixture()),
        project_node(&node_fixture()),
        project_event(&event_fixture()),
        project_secret(&secret_fixture()),
        project_metadata_only(&dirty_metadata("configmap-x", Some("apps")), "ConfigMap"),
    ]
}

#[test]
fn every_row_serializes_under_one_kilobyte() {
    for row in all_rows() {
        let bytes = serde_json::to_vec(&row).expect("row must serialize");
        assert!(
            bytes.len() < 1024,
            "row for {} serialized to {} bytes",
            row.meta.kind,
            bytes.len()
        );
    }
}

#[test]
fn managed_fields_never_reach_a_row() {
    for row in all_rows() {
        let json = serde_json::to_string(&row).expect("row must serialize");
        assert!(
            !json.contains("managedFields") && !json.contains("managed_fields"),
            "managedFields leaked into {} row: {json}",
            row.meta.kind
        );
    }
}

#[test]
fn last_applied_configuration_never_reaches_a_row() {
    for row in all_rows() {
        let json = serde_json::to_string(&row).expect("row must serialize");
        assert!(
            !json.contains(LAST_APPLIED_KEY) && !json.contains("huge"),
            "last-applied annotation leaked into {} row: {json}",
            row.meta.kind
        );
    }
}

#[test]
fn secret_payload_never_reaches_a_row() {
    let row = project_secret(&secret_fixture());
    let json = serde_json::to_string(&row).expect("row must serialize");
    assert!(!json.contains("\"data\""), "secret data key leaked: {json}");
    assert!(
        !json.contains("\"stringData\""),
        "secret stringData key leaked: {json}"
    );
    assert!(
        !json.contains("super-secret-value") && !json.contains("also-secret"),
        "secret payload leaked: {json}"
    );
    // base64 of the literal secret value must not appear either.
    assert!(
        !json.contains("c3VwZXItc2VjcmV0LXZhbHVl"),
        "base64-encoded secret payload leaked: {json}"
    );
}

#[test]
fn full_pod_spec_never_reaches_a_list_row() {
    let row = project_pod(&pod_fixture());
    let json = serde_json::to_string(&row).expect("row must serialize");
    assert!(!json.contains("\"spec\""), "full pod spec leaked: {json}");
    assert!(
        !json.contains("registry.example/web:1.0"),
        "container image string leaked into row: {json}"
    );
}

#[test]
fn metadata_only_path_carries_no_status_scalars() {
    let row = project_metadata_only(&dirty_metadata("configmap-x", Some("apps")), "ConfigMap");
    let json = serde_json::to_string(&row).expect("row must serialize");
    assert!(
        json.contains("\"Metadata\""),
        "expected the Metadata variant: {json}"
    );
    assert_eq!(row.meta.kind, "ConfigMap");
}
