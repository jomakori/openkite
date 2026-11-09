//! A mock gateway serving fixture data to a preview host, which has no cluster credential.
//!
//! The host's surfaces claim to render an empty, a loading and an error state
//! (G4). A claim like that is only checkable if the state can be *driven*, so
//! the fixture gateway takes its answer from `?fixture=` on the request: the
//! gate navigates the same route four times and reads `data-state` back out of
//! the DOM instead of trusting a screenshot. Nothing here is reachable from a
//! host that has a cluster credential: the state is read in [`crate::routes`]
//! only on the fixture path.

use std::collections::BTreeMap;
use std::sync::Arc;

use openkite_api::capability::Capabilities;
use openkite_api::crud::Mutation;
use openkite_api::gateway::{Gateway, GatewayError, GatewayFuture};
use openkite_api::secret::{SecretObject, SecretRef};

/// The answer the fixture host gives, and so the state its surfaces paint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FixtureState {
    /// The host has data: the surfaces paint rows.
    #[default]
    Ready,
    /// The host answered, with nothing in it.
    Empty,
    /// The host has not answered yet.
    Loading,
    /// The host's gateway refused the read.
    Error,
}

impl FixtureState {
    /// Read the state from a `?fixture=` query value. An absent or unknown
    /// value is [`FixtureState::Ready`], so every existing URL keeps its page.
    pub fn from_query(value: Option<&str>) -> Self {
        match value.unwrap_or_default() {
            "empty" => Self::Empty,
            "loading" => Self::Loading,
            "error" => Self::Error,
            _ => Self::Ready,
        }
    }

    /// The `data-state` value this answer paints.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Empty => "empty",
            Self::Loading => "loading",
            Self::Error => "error",
        }
    }

    /// Whether the host is still waiting on its gateway.
    pub fn is_loading(self) -> bool {
        matches!(self, Self::Loading)
    }
}

/// The message the fixture gateway refuses with, so the error surface has
/// something a person can read rather than a bare status code.
pub const FIXTURE_ERROR: &str = "the fixture gateway reports the cluster as unreachable";

/// A fixture secret that looks real enough to render the console.
fn fixture_secret_object() -> SecretObject {
    SecretObject {
        name: "example-secret".to_string(),
        namespace: Some("default".to_string()),
        type_: Some("Opaque".to_string()),
        data: BTreeMap::from([("password".to_string(), b"secret123".to_vec())]),
        string_data: BTreeMap::new(),
    }
}

/// Demo pods for a host with no cluster credential (OKT-210): enough variety
/// (phases, ready/restart counts, nodes, QoS classes, namespaces) that
/// `/workloads` with `?fixture=ready` (the unset default) paints rows instead
/// of an empty table. Parsed through the same `pods` + `pod_row` path a real
/// cluster answer takes, so the row shape matches exactly.
pub fn demo_pod_list() -> serde_json::Value {
    serde_json::json!({
        "items": [
            {
                "apiVersion": "v1",
                "kind": "Pod",
                "metadata": {
                    "name": "web-7d9f6c9b8-4mjkq",
                    "namespace": "default",
                    "creationTimestamp": "2024-11-01T09:00:00Z",
                    "ownerReferences": [
                        {"apiVersion": "apps/v1", "kind": "ReplicaSet", "name": "web-7d9f6c9b8", "uid": "demo-1", "controller": true}
                    ]
                },
                "spec": {
                    "containers": [{"name": "web", "image": "web:1.4.2"}],
                    "nodeName": "gke-pool-1-9f3a"
                },
                "status": {
                    "phase": "Running",
                    "qosClass": "Guaranteed",
                    "containerStatuses": [
                        {"name": "web", "image": "web:1.4.2", "imageID": "", "ready": true, "restartCount": 0, "state": {"running": {"startedAt": "2024-11-01T09:00:05Z"}}}
                    ]
                }
            },
            {
                "apiVersion": "v1",
                "kind": "Pod",
                "metadata": {
                    "name": "web-7d9f6c9b8-9plrz",
                    "namespace": "default",
                    "creationTimestamp": "2024-11-01T09:00:00Z",
                    "ownerReferences": [
                        {"apiVersion": "apps/v1", "kind": "ReplicaSet", "name": "web-7d9f6c9b8", "uid": "demo-1", "controller": true}
                    ]
                },
                "spec": {
                    "containers": [
                        {"name": "web", "image": "web:1.4.2"},
                        {"name": "sidecar", "image": "envoy:1.29"}
                    ],
                    "nodeName": "gke-pool-1-9f3a"
                },
                "status": {
                    "phase": "Running",
                    "qosClass": "Burstable",
                    "containerStatuses": [
                        {"name": "web", "image": "web:1.4.2", "imageID": "", "ready": true, "restartCount": 1, "state": {"running": {"startedAt": "2024-11-02T03:00:00Z"}}},
                        {"name": "sidecar", "image": "envoy:1.29", "imageID": "", "ready": false, "restartCount": 4, "state": {"waiting": {"reason": "ContainerCreating"}}}
                    ]
                }
            },
            {
                "apiVersion": "v1",
                "kind": "Pod",
                "metadata": {
                    "name": "checkout-api-0",
                    "namespace": "default",
                    "creationTimestamp": "2024-11-05T12:30:00Z",
                    "ownerReferences": [
                        {"apiVersion": "apps/v1", "kind": "StatefulSet", "name": "checkout-api", "uid": "demo-2", "controller": true}
                    ]
                },
                "spec": {
                    "containers": [{"name": "checkout-api", "image": "checkout-api:2.0.0"}]
                },
                "status": {
                    "phase": "Pending",
                    "qosClass": "BestEffort"
                }
            },
            {
                "apiVersion": "v1",
                "kind": "Pod",
                "metadata": {
                    "name": "cache-6b7f9c5d4-klmno",
                    "namespace": "openkite",
                    "creationTimestamp": "2024-10-20T18:00:00Z",
                    "ownerReferences": [
                        {"apiVersion": "apps/v1", "kind": "ReplicaSet", "name": "cache-6b7f9c5d4", "uid": "demo-3", "controller": true}
                    ]
                },
                "spec": {
                    "containers": [{"name": "cache", "image": "redis:7.2"}],
                    "nodeName": "gke-pool-2-7c1e"
                },
                "status": {
                    "phase": "Running",
                    "qosClass": "Burstable",
                    "containerStatuses": [
                        {"name": "cache", "image": "redis:7.2", "imageID": "", "ready": false, "restartCount": 12, "state": {"waiting": {"reason": "CrashLoopBackOff"}}}
                    ]
                }
            },
            {
                "apiVersion": "v1",
                "kind": "Pod",
                "metadata": {
                    "name": "metrics-server-0",
                    "namespace": "openkite",
                    "creationTimestamp": "2024-09-15T07:45:00Z",
                    "ownerReferences": [
                        {"apiVersion": "apps/v1", "kind": "StatefulSet", "name": "metrics-server", "uid": "demo-4", "controller": true}
                    ]
                },
                "spec": {
                    "containers": [{"name": "metrics-server", "image": "metrics-server:0.7.1"}],
                    "nodeName": "gke-pool-2-7c1e"
                },
                "status": {
                    "phase": "Running",
                    "qosClass": "Guaranteed",
                    "containerStatuses": [
                        {"name": "metrics-server", "image": "metrics-server:0.7.1", "imageID": "", "ready": true, "restartCount": 0, "state": {"running": {"startedAt": "2024-09-15T07:45:10Z"}}}
                    ]
                }
            }
        ]
    })
}

/// A fixture gateway serving demo data when no cluster is available.
pub struct FixtureGateway {
    state: FixtureState,
}

impl FixtureGateway {
    /// A gateway that answers as `state` says.
    pub fn new(state: FixtureState) -> Self {
        Self { state }
    }
}

impl Gateway for FixtureGateway {
    fn capabilities(&self) -> Capabilities {
        Capabilities::server_side()
    }

    fn apply(&self, _mutation: Mutation) -> GatewayFuture<'_, Result<(), GatewayError>> {
        Box::pin(async {
            Err(GatewayError::new(
                "fixture gateway does not support mutations",
            ))
        })
    }

    fn secret(
        &self,
        _namespace: String,
        _name: String,
    ) -> GatewayFuture<'_, Result<SecretObject, GatewayError>> {
        let state = self.state;
        Box::pin(async move {
            if state == FixtureState::Error {
                return Err(GatewayError::new(FIXTURE_ERROR));
            }
            Ok(fixture_secret_object())
        })
    }

    fn secret_refs(&self) -> GatewayFuture<'_, Result<Vec<SecretRef>, GatewayError>> {
        let state = self.state;
        Box::pin(async move {
            match state {
                FixtureState::Error => Err(GatewayError::new(FIXTURE_ERROR)),
                FixtureState::Empty | FixtureState::Loading => Ok(Vec::new()),
                FixtureState::Ready => Ok(vec![SecretRef {
                    name: "example-secret".to_string(),
                    namespace: "default".to_string(),
                }]),
            }
        })
    }
}

/// Create a fixture gateway wrapped in Arc for injection into routes.
pub fn make_fixture_gateway() -> Arc<dyn Gateway> {
    make_fixture_gateway_in(FixtureState::Ready)
}

/// Create a fixture gateway that answers as `state` says.
pub fn make_fixture_gateway_in(state: FixtureState) -> Arc<dyn Gateway> {
    Arc::new(FixtureGateway::new(state))
}
