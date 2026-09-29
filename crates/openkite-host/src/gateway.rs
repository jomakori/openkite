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
use openkite_api::secret::SecretObject;
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
}
