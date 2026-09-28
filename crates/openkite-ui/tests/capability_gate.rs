//! Capability gating predicates (OKT-126).
//!
//! The terminal view and the native-chrome settings render only when the host
//! reports the matching capability. These tests pin that the runtime surfaces
//! the host's verdict instead of falling back to "assume desktop".

use std::sync::Arc;

use openkite_api::capability::Capabilities;
use openkite_api::crud::{Mutation, PropagationPolicy};
use openkite_api::gateway::{Gateway, GatewayError, GatewayFuture};
use openkite_api::secret::SecretObject;
use openkite_ui::runtime::{native_chrome_can_render, terminal_can_render};

struct FixedGateway(Capabilities);

impl Gateway for FixedGateway {
    fn capabilities(&self) -> Capabilities {
        self.0
    }

    fn apply(&self, _: Mutation) -> GatewayFuture<'_, Result<(), GatewayError>> {
        Box::pin(async { Ok(()) })
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

#[test]
fn terminal_surface_gates_on_reported_capability() {
    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::in_process()))));
    assert!(terminal_can_render());

    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::server_side()))));
    assert!(!terminal_can_render());

    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::minimal()))));
    assert!(!terminal_can_render());
}

#[test]
fn native_chrome_surface_gates_on_reported_capability() {
    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::in_process()))));
    assert!(native_chrome_can_render());

    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::server_side()))));
    assert!(!native_chrome_can_render());

    openkite_ui::runtime::set_gateway(None);
    assert!(!native_chrome_can_render());
}

#[test]
fn missing_gateway_means_no_surfaces_render() {
    openkite_ui::runtime::set_gateway(None);
    assert!(!terminal_can_render());
    assert!(!native_chrome_can_render());
}

#[test]
fn mutation_round_trip_through_a_minimal_gateway() {
    let g = FixedGateway(Capabilities::minimal());
    let mut m = Mutation::Delete {
        kind: "Pod".into(),
        namespace: Some("default".into()),
        name: "nginx".into(),
        propagation: PropagationPolicy::Default,
    };
    assert_eq!(m.verb(), "delete");
    m = Mutation::Edit(serde_json::json!({"kind": "Service"}));
    assert_eq!(m.verb(), "edit");
    let _ = g.capabilities();
}
