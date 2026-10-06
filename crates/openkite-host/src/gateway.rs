//! The kube-rs adapter behind the contract: the host-side implementation of
//! `openkite_api::gateway::Gateway`.
//!
//! This is the only place a `kube`/`k8s-openapi` type is turned into a contract
//! type. The trait itself never names one, which is what keeps the client out
//! of the wasm build of `openkite-ui` by type-checking alone.

use k8s_openapi::api::core::v1::{ContainerState, Pod, Secret};
use kube::{Api, Client};

use openkite_api::capability::{Capabilities, GatewayKind};
use openkite_api::crud::Mutation;
use openkite_api::gateway::{Gateway, GatewayError, GatewayFuture};
use openkite_api::pod::{ContainerInfo, PodObject, PodSummary};
use openkite_api::secret::{SecretObject, SecretRef};
use serde_saphyr::to_string as yaml_to_string;
use std::collections::BTreeMap;

/// Dispatch a mutation. Today: returns the Phase-1-pending error so the UI
/// flows end-to-end. Future Phase 1: kube `Api::create` / `Api::patch` /
/// `Api::delete` (with propagation) / `Api::patch` for scale (SSA).
pub async fn apply_mutation(_client: &Client, m: &Mutation) -> Result<(), String> {
    Err(format!("{}: cluster mutation lands in Phase 1", m.verb()))
}

/// Copy the parts of a Kubernetes Secret the console reads into the owned
/// contract type: `ByteString` becomes plain bytes, metadata flattens to
/// name/namespace/type.
pub fn secret_object(secret: &Secret) -> SecretObject {
    SecretObject {
        name: secret.metadata.name.clone().unwrap_or_default(),
        namespace: secret.metadata.namespace.clone(),
        type_: secret.type_.clone(),
        data: secret
            .data
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| (key, value.0))
            .collect(),
        string_data: secret.string_data.clone().unwrap_or_default(),
    }
}

/// Copy a Secret's identity out of kube: namespace + name only, never values.
pub fn secret_ref(secret: &Secret) -> SecretRef {
    SecretRef {
        namespace: secret.metadata.namespace.clone().unwrap_or_default(),
        name: secret.metadata.name.clone().unwrap_or_default(),
    }
}

/// Render the state sub-object into the inspector's container row label.
fn state_from_kube(state: Option<&ContainerState>) -> String {
    let Some(state) = state else {
        return "Pending".to_string();
    };
    let running = state.running.is_some();
    let waiting_reason = state.waiting.as_ref().and_then(|w| w.reason.clone());
    let terminated_reason = state.terminated.as_ref().and_then(|t| t.reason.clone());
    let exit_code = state.terminated.as_ref().map(|t| t.exit_code);
    openkite_api::pod::state_label(
        running,
        waiting_reason.as_deref(),
        terminated_reason.as_deref(),
        exit_code,
    )
}

/// Copy the parts of a Kubernetes Pod the inspector and log viewer render
/// into the owned `PodObject` contract. The host serialises the pod to YAML
/// once here so the UI never imports `serde-saphyr`; a serialise failure
/// falls back to an empty string and the YAML tab renders the placeholder.
pub fn pod_object(pod: &Pod) -> PodObject {
    let spec_containers = pod
        .spec
        .as_ref()
        .map(|s| s.containers.as_slice())
        .unwrap_or(&[]);
    let statuses = pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_deref())
        .unwrap_or(&[]);

    let containers: Vec<ContainerInfo> = spec_containers
        .iter()
        .map(|c| {
            let status = statuses.iter().find(|s| s.name == c.name);
            ContainerInfo {
                name: c.name.clone(),
                image: c.image.clone().unwrap_or_default(),
                ready: status.map(|s| s.ready).unwrap_or(false),
                restarts: status.map(|s| s.restart_count).unwrap_or(0),
                state: state_from_kube(status.and_then(|s| s.state.as_ref())),
            }
        })
        .collect();

    let status = pod.status.as_ref();
    let summary = PodSummary {
        phase: status
            .and_then(|s| s.phase.clone())
            .unwrap_or_else(|| "Unknown".to_string()),
        node: pod
            .spec
            .as_ref()
            .and_then(|s| s.node_name.clone())
            .unwrap_or_default(),
        pod_ip: status.and_then(|s| s.pod_ip.clone()).unwrap_or_default(),
        qos: status.and_then(|s| s.qos_class.clone()).unwrap_or_default(),
        reason: status.and_then(|s| s.reason.clone()),
        message: status.and_then(|s| s.message.clone()),
    };

    let labels: BTreeMap<String, String> = pod.metadata.labels.clone().unwrap_or_default();
    let annotations: BTreeMap<String, String> =
        pod.metadata.annotations.clone().unwrap_or_default();

    let yaml = yaml_to_string(pod).unwrap_or_default();

    PodObject {
        name: pod.metadata.name.clone().unwrap_or_default(),
        namespace: pod.metadata.namespace.clone(),
        summary,
        containers,
        labels,
        annotations,
        yaml,
    }
}

/// The kube gateway: a host's cluster client answering the contract's typed
/// requests directly over kube-rs. The `kind` is the one thing the desktop and
/// the browser hosts do not agree on — the desktop reaches kube in-process,
/// the browser reaches it on the server side of the same origin.
pub struct KubeGateway {
    client: Client,
    kind: GatewayKind,
}

impl KubeGateway {
    /// Wrap a connected cluster client for the desktop host.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            kind: GatewayKind::InProcess,
        }
    }

    /// Wrap a connected cluster client for the browser host, whose wasm client
    /// cannot reach kube and posts its gateway calls back to the server.
    pub fn server_side(client: Client) -> Self {
        Self {
            client,
            kind: GatewayKind::ServerSide,
        }
    }

    /// The client behind the gateway (the desktop host still needs it for
    /// reflectors and direct reads).
    pub fn client(&self) -> &Client {
        &self.client
    }
}

impl Gateway for KubeGateway {
    fn capabilities(&self) -> Capabilities {
        match self.kind {
            GatewayKind::InProcess => Capabilities::in_process(),
            GatewayKind::ServerSide => Capabilities::server_side(),
        }
    }

    fn apply(&self, mutation: Mutation) -> GatewayFuture<'_, Result<(), GatewayError>> {
        Box::pin(async move {
            apply_mutation(&self.client, &mutation)
                .await
                .map_err(GatewayError::new)
        })
    }

    fn secret(
        &self,
        namespace: String,
        name: String,
    ) -> GatewayFuture<'_, Result<SecretObject, GatewayError>> {
        Box::pin(async move {
            let secrets: Api<Secret> = Api::namespaced(self.client.clone(), &namespace);
            let secret = secrets
                .get(&name)
                .await
                .map_err(|err| GatewayError::new(err.to_string()))?;
            Ok(secret_object(&secret))
        })
    }

    fn secret_refs(&self) -> GatewayFuture<'_, Result<Vec<SecretRef>, GatewayError>> {
        Box::pin(async move {
            let secrets: Api<Secret> = Api::all(self.client.clone());
            let list = secrets
                .list(&Default::default())
                .await
                .map_err(|err| GatewayError::new(err.to_string()))?;
            Ok(list.items.iter().map(secret_ref).collect())
        })
    }
}
