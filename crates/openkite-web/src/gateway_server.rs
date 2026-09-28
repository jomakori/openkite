use k8s_openapi::api::core::v1::Secret;
use kube::{Api, Client};

use openkite_api::capability::Capabilities;
use openkite_api::crud::Mutation;
use openkite_api::gateway::{Gateway, GatewayError, GatewayFuture};
use openkite_api::secret::SecretObject;

/// The server-side gateway: the browser host's kube client, answering the
/// contract's typed requests directly over kube-rs.
///
/// Identical in shape to `openkite_host::gateway::KubeGateway` — the trait
/// is implemented once and both hosts share it. The browser holds no
/// credentials: this struct lives in the axum process and the wasm client
/// reaches it over `POST /api/gateway`.
pub struct ServerGateway {
    client: Client,
}

impl ServerGateway {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl Gateway for ServerGateway {
    fn capabilities(&self) -> Capabilities {
        Capabilities::server_side()
    }

    fn apply(&self, mutation: Mutation) -> GatewayFuture<'_, Result<(), GatewayError>> {
        let client = self.client.clone();
        Box::pin(async move {
            openkite_host::gateway::apply_mutation(&client, &mutation)
                .await
                .map_err(GatewayError::new)
        })
    }

    fn secret(
        &self,
        namespace: String,
        name: String,
    ) -> GatewayFuture<'_, Result<SecretObject, GatewayError>> {
        let client = self.client.clone();
        Box::pin(async move {
            let secrets: Api<Secret> = Api::namespaced(client, &namespace);
            let secret = secrets
                .get(&name)
                .await
                .map_err(|err| GatewayError::new(err.to_string()))?;
            Ok(openkite_host::gateway::secret_object(&secret))
        })
    }
}
