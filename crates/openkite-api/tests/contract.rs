//! Contract tests: the owned types round-trip, and the gateway trait is
//! object-safe behind `Arc<dyn Gateway>`.

use std::collections::BTreeMap;
use std::sync::Arc;

use openkite_api::bridge::{ApiRequest, ApiResponse, BridgeRequest};
use openkite_api::capability::{Capabilities, GatewayKind};
use openkite_api::crud::{
    target_summary, typed_name_matches, validate_for_edit, Mutation, PropagationPolicy,
};
use openkite_api::gateway::{Gateway, GatewayError, GatewayFuture};
use openkite_api::secret::{
    bulk_reveal_predicate, decoded_value_for_key, row_id_for_secret, secret_keys,
    secret_kind_label, SecretObject,
};

fn secret() -> SecretObject {
    let mut data = BTreeMap::new();
    data.insert("password".to_string(), b"hunter2".to_vec());
    let mut string_data = BTreeMap::new();
    string_data.insert("username".to_string(), "admin".to_string());
    SecretObject {
        name: "db-credentials".into(),
        namespace: Some("default".into()),
        type_: Some("kubernetes.io/tls".into()),
        data,
        string_data,
    }
}

#[test]
fn secret_keys_are_the_sorted_union_of_data_and_string_data() {
    assert_eq!(secret_keys(&secret()), vec!["password", "username"]);
}

#[test]
fn decoded_value_prefers_data_then_string_data() {
    let secret = secret();
    assert_eq!(decoded_value_for_key(&secret, "password"), "hunter2");
    assert_eq!(decoded_value_for_key(&secret, "username"), "admin");
    assert_eq!(decoded_value_for_key(&secret, "absent"), "");
}

#[test]
fn row_id_and_labels_match_the_table_conventions() {
    let secret = secret();
    assert_eq!(row_id_for_secret(&secret), "default/db-credentials");
    assert_eq!(secret_kind_label(secret.type_.as_deref()), "TLS");
    assert_eq!(secret_kind_label(None), "Opaque");

    let mut bare = secret.clone();
    bare.namespace = None;
    assert_eq!(row_id_for_secret(&bare), "db-credentials");
}

#[test]
fn bulk_reveal_gate_trims_but_stays_case_sensitive() {
    assert!(bulk_reveal_predicate(" db-credentials ", "db-credentials"));
    assert!(!bulk_reveal_predicate("DB-credentials", "db-credentials"));
}

#[test]
fn mutation_gates_and_serde_round_trip() {
    let doc = serde_json::json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": { "name": "web", "namespace": "default", "resourceVersion": "42" }
    });
    assert!(validate_for_edit(&doc).is_ok());
    assert_eq!(target_summary(&Mutation::Edit(doc)).name, "web");
    assert!(typed_name_matches("web", "web"));

    let wire = serde_json::to_string(&PropagationPolicy::Default).unwrap();
    assert_eq!(wire, "\"default\"");
}

#[test]
fn capability_descriptor_round_trips() {
    let desktop = Capabilities::in_process();
    assert_eq!(desktop.gateway, GatewayKind::InProcess);
    assert!(desktop.plugins);
    assert!(desktop.terminal);
    assert!(!desktop.exec);

    let wire = serde_json::to_string(&desktop).unwrap();
    assert_eq!(
        serde_json::from_str::<Capabilities>(&wire).unwrap(),
        desktop
    );

    let browser = Capabilities::server_side();
    assert_eq!(browser.gateway, GatewayKind::ServerSide);
    assert!(!browser.plugins);
}

#[test]
fn bridge_envelope_round_trips_and_describes() {
    let request = ApiRequest::List {
        kind: "pods".into(),
        ns: Some("default".into()),
    };
    assert_eq!(request.describe(), "list pods");
    let envelope = BridgeRequest {
        id: 7,
        plugin: "demo".into(),
        request: request.clone(),
    };
    let wire = serde_json::to_string(&envelope).unwrap();
    assert_eq!(
        serde_json::from_str::<BridgeRequest>(&wire).unwrap(),
        envelope
    );

    let ok = ApiResponse::Ok {
        result: serde_json::json!({ "items": [] }),
    };
    let wire = serde_json::to_string(&ok).unwrap();
    assert_eq!(serde_json::from_str::<ApiResponse>(&wire).unwrap(), ok);
}

struct StubGateway;

impl Gateway for StubGateway {
    fn capabilities(&self) -> Capabilities {
        Capabilities::in_process()
    }

    fn apply(&self, mutation: Mutation) -> GatewayFuture<'_, Result<(), GatewayError>> {
        Box::pin(async move { Err(GatewayError::new(format!("{}: stub", mutation.verb()))) })
    }

    fn secret(
        &self,
        namespace: String,
        name: String,
    ) -> GatewayFuture<'_, Result<SecretObject, GatewayError>> {
        Box::pin(async move {
            Ok(SecretObject {
                name,
                namespace: Some(namespace),
                ..SecretObject::default()
            })
        })
    }
}

#[tokio::test]
async fn gateway_is_object_safe_and_awaitable() {
    let gateway: Arc<dyn Gateway> = Arc::new(StubGateway);
    assert_eq!(gateway.capabilities().gateway, GatewayKind::InProcess);

    let applied = gateway
        .apply(Mutation::Delete {
            kind: "Pod".into(),
            namespace: Some("default".into()),
            name: "nginx".into(),
            propagation: PropagationPolicy::Default,
        })
        .await;
    assert_eq!(applied.unwrap_err().to_string(), "delete: stub");

    let fetched = gateway.secret("default".into(), "db".into()).await.unwrap();
    assert_eq!(fetched.name, "db");
    assert_eq!(fetched.namespace.as_deref(), Some("default"));
}
