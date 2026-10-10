//! A mock gateway serving fixture data to a preview host, which has no cluster credential.

use std::collections::BTreeMap;
use std::sync::Arc;

use openkite_api::capability::Capabilities;
use openkite_api::crud::Mutation;
use openkite_api::gateway::{Gateway, GatewayError, GatewayFuture};
use openkite_api::secret::{SecretObject, SecretRef};

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

/// A fixture gateway serving demo data when no cluster is available.
pub struct FixtureGateway;

impl Gateway for FixtureGateway {
    fn capabilities(&self) -> Capabilities {
        Capabilities::server_side()
    }

    fn apply(&self, _mutation: Mutation) -> GatewayFuture<Result<(), GatewayError>> {
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
    ) -> GatewayFuture<Result<SecretObject, GatewayError>> {
        let secret = fixture_secret_object();
        Box::pin(async move { Ok(secret) })
    }

    fn secret_refs(&self) -> GatewayFuture<Result<Vec<SecretRef>, GatewayError>> {
        let refs = vec![SecretRef {
            name: "example-secret".to_string(),
            namespace: "default".to_string(),
        }];
        Box::pin(async move { Ok(refs) })
    }
}

/// Create a fixture gateway wrapped in Arc for injection into routes.
pub fn make_fixture_gateway() -> Arc<dyn Gateway> {
    Arc::new(FixtureGateway)
}
