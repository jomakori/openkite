//! Row projections: compact, privacy-safe summaries of objects for list views.
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashSet};
use std::hash::{Hash, Hasher};

use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{Event, Node, Pod, Secret};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use serde::{Deserialize, Serialize};

/// Fields every row carries regardless of kind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RowMeta {
    pub uid: String,
    pub name: String,
    pub namespace: Option<String>,
    pub kind: String,
    pub created: Option<String>,
    pub labels_hash: u64,
    pub resource_version: Option<String>,
}

/// A condition's type and status, stripped of messages and timestamps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConditionRow {
    pub type_: String,
    pub status: String,
}

/// Pod-specific status scalars.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PodStatusRow {
    pub phase: Option<String>,
    pub restarts: i32,
    pub ready_containers: u32,
    pub total_containers: u32,
    pub node: Option<String>,
    pub started_at: Option<String>,
    pub image_count: u32,
}

/// Deployment-specific status scalars.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeploymentStatusRow {
    pub replicas_desired: Option<i32>,
    pub replicas_ready: Option<i32>,
    pub replicas_available: Option<i32>,
    pub replicas_updated: Option<i32>,
    pub replicas_unavailable: Option<i32>,
    pub conditions: Vec<ConditionRow>,
}

/// Node-specific status scalars.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeStatusRow {
    pub ready: Option<bool>,
    pub unschedulable: bool,
    pub allocatable_present: bool,
    pub capacity_present: bool,
    pub address_count: u32,
    pub condition_count: u32,
}

/// Event-specific status scalars.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventStatusRow {
    pub type_: Option<String>,
    pub reason: Option<String>,
    pub count: Option<i32>,
    pub involved_kind: Option<String>,
    pub involved_name: Option<String>,
    pub involved_namespace: Option<String>,
}

/// Secret-specific status scalars: never the key values themselves.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SecretStatusRow {
    pub type_: Option<String>,
    pub key_count: u32,
}

/// Per-kind status scalars; unlisted kinds and the Probe path carry metadata only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RowStatus {
    Pod(PodStatusRow),
    Deployment(DeploymentStatusRow),
    Node(NodeStatusRow),
    Event(EventStatusRow),
    Secret(SecretStatusRow),
    Metadata,
}

/// A projected list row: metadata plus kind-specific status scalars.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Row {
    pub meta: RowMeta,
    pub status: RowStatus,
}

/// Order-independent digest of a label set, so rows never carry raw label values.
fn hash_labels(labels: &Option<BTreeMap<String, String>>) -> u64 {
    let mut hasher = DefaultHasher::new();
    if let Some(map) = labels {
        for (key, value) in map {
            key.hash(&mut hasher);
            value.hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Builds the kind-independent fields shared by every row.
fn row_meta(meta: &ObjectMeta, kind: &str) -> RowMeta {
    RowMeta {
        uid: meta.uid.clone().unwrap_or_default(),
        name: meta.name.clone().unwrap_or_default(),
        namespace: meta.namespace.clone(),
        kind: kind.to_owned(),
        created: meta
            .creation_timestamp
            .as_ref()
            .map(|time| time.0.to_string()),
        labels_hash: hash_labels(&meta.labels),
        resource_version: meta.resource_version.clone(),
    }
}

/// Projects a Pod into its row: phase, restarts, container readiness, node, image count.
pub fn project_pod(pod: &Pod) -> Row {
    let status = pod.status.as_ref();
    let container_statuses = status.and_then(|status| status.container_statuses.as_ref());
    let restarts = container_statuses
        .map(|statuses| {
            statuses
                .iter()
                .map(|status| status.restart_count)
                .sum::<i32>()
        })
        .unwrap_or(0);
    let ready_containers = container_statuses
        .map(|statuses| statuses.iter().filter(|status| status.ready).count() as u32)
        .unwrap_or(0);
    let total_containers = pod
        .spec
        .as_ref()
        .map(|spec| spec.containers.len() as u32)
        .unwrap_or(0);
    let image_count = pod
        .spec
        .as_ref()
        .map(|spec| {
            spec.containers
                .iter()
                .filter_map(|container| container.image.as_ref())
                .collect::<HashSet<_>>()
                .len() as u32
        })
        .unwrap_or(0);
    let node = pod.spec.as_ref().and_then(|spec| spec.node_name.clone());
    let started_at = status
        .and_then(|status| status.start_time.as_ref())
        .map(|time| time.0.to_string());
    let phase = status.and_then(|status| status.phase.clone());

    Row {
        meta: row_meta(&pod.metadata, "Pod"),
        status: RowStatus::Pod(PodStatusRow {
            phase,
            restarts,
            ready_containers,
            total_containers,
            node,
            started_at,
            image_count,
        }),
    }
}

/// Projects a Deployment into its row: replica counts and condition types.
pub fn project_deployment(deployment: &Deployment) -> Row {
    let status = deployment.status.as_ref();
    let conditions = status
        .and_then(|status| status.conditions.as_ref())
        .map(|conditions| {
            conditions
                .iter()
                .map(|condition| ConditionRow {
                    type_: condition.type_.clone().unwrap_or_default(),
                    status: condition.status.clone().unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    Row {
        meta: row_meta(&deployment.metadata, "Deployment"),
        status: RowStatus::Deployment(DeploymentStatusRow {
            replicas_desired: deployment.spec.replicas,
            replicas_ready: status.and_then(|status| status.ready_replicas),
            replicas_available: status.and_then(|status| status.available_replicas),
            replicas_updated: status.and_then(|status| status.updated_replicas),
            replicas_unavailable: status.and_then(|status| status.unavailable_replicas),
            conditions,
        }),
    }
}

/// Projects a Node into its row: readiness, schedulability, and resource presence.
pub fn project_node(node: &Node) -> Row {
    let status = node.status.as_ref();
    let ready = status
        .and_then(|status| status.conditions.as_ref())
        .and_then(|conditions| {
            conditions
                .iter()
                .find(|condition| condition.type_ == "Ready")
        })
        .map(|condition| condition.status == "True");
    let unschedulable = node
        .spec
        .as_ref()
        .and_then(|spec| spec.unschedulable)
        .unwrap_or(false);
    let allocatable_present = status
        .and_then(|status| status.allocatable.as_ref())
        .map(|map| !map.is_empty())
        .unwrap_or(false);
    let capacity_present = status
        .and_then(|status| status.capacity.as_ref())
        .map(|map| !map.is_empty())
        .unwrap_or(false);
    let address_count = status
        .and_then(|status| status.addresses.as_ref())
        .map(|addresses| addresses.len() as u32)
        .unwrap_or(0);
    let condition_count = status
        .and_then(|status| status.conditions.as_ref())
        .map(|conditions| conditions.len() as u32)
        .unwrap_or(0);

    Row {
        meta: row_meta(&node.metadata, "Node"),
        status: RowStatus::Node(NodeStatusRow {
            ready,
            unschedulable,
            allocatable_present,
            capacity_present,
            address_count,
            condition_count,
        }),
    }
}

/// Projects an Event into its row: type, reason, count, and the involved object.
pub fn project_event(event: &Event) -> Row {
    Row {
        meta: row_meta(&event.metadata, "Event"),
        status: RowStatus::Event(EventStatusRow {
            type_: event.type_.clone(),
            reason: event.reason.clone(),
            count: event.count,
            involved_kind: event.involved_object.kind.clone(),
            involved_name: event.involved_object.name.clone(),
            involved_namespace: event.involved_object.namespace.clone(),
        }),
    }
}

/// Projects a Secret into its row: type and key count, never the key values.
pub fn project_secret(secret: &Secret) -> Row {
    let data_keys = secret.data.as_ref().map(|data| data.len()).unwrap_or(0);
    let string_data_keys = secret
        .string_data
        .as_ref()
        .map(|data| data.len())
        .unwrap_or(0);

    Row {
        meta: row_meta(&secret.metadata, "Secret"),
        status: RowStatus::Secret(SecretStatusRow {
            type_: secret.type_.clone(),
            key_count: (data_keys + string_data_keys) as u32,
        }),
    }
}

/// Metadata-only row used by the generic fallback and by Probe-class count estimates.
pub fn project_metadata_only(meta: &ObjectMeta, kind: &str) -> Row {
    Row {
        meta: row_meta(meta, kind),
        status: RowStatus::Metadata,
    }
}
