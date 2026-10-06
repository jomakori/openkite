//! Capability gating predicates (OKT-126).
//!
//! The terminal view and the native-chrome settings render only when the host
//! reports the matching capability. These tests pin that the runtime surfaces
//! the host's verdict instead of falling back to "assume desktop".

use std::sync::{Arc, Mutex};

use openkite_api::capability::Capabilities;
use openkite_api::crud::{Mutation, PropagationPolicy};
use openkite_api::gateway::{Gateway, GatewayError, GatewayFuture};
use openkite_api::secret::{SecretObject, SecretRef};
use openkite_ui::runtime::{
    cluster_switch_can_render, mutations_can_render, native_chrome_can_render, terminal_can_render,
};

// The runtime slots are process-global; hold this guard in every test that
// mutates them so parallel test threads cannot gate on each other's state.
static RUNTIME_SLOTS: Mutex<()> = Mutex::new(());

fn gate_lock() -> std::sync::MutexGuard<'static, ()> {
    RUNTIME_SLOTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

    fn secret_refs(&self) -> GatewayFuture<'_, Result<Vec<SecretRef>, GatewayError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[test]
fn terminal_surface_gates_on_reported_capability() {
    let _gate = gate_lock();
    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::in_process()))));
    assert!(terminal_can_render());

    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::server_side()))));
    assert!(!terminal_can_render());

    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::minimal()))));
    assert!(!terminal_can_render());
}

#[test]
fn native_chrome_surface_gates_on_reported_capability() {
    let _gate = gate_lock();
    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::in_process()))));
    assert!(native_chrome_can_render());

    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::server_side()))));
    assert!(!native_chrome_can_render());

    openkite_ui::runtime::set_gateway(None);
    assert!(!native_chrome_can_render());
}

#[test]
fn cluster_switch_gates_on_the_gateway_the_host_owns() {
    let _gate = gate_lock();
    // The in-process gateway owns the kubeconfig, so it can list contexts.
    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::in_process()))));
    assert!(cluster_switch_can_render());

    // A server-side gateway serves exactly one cluster: no context list.
    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::server_side()))));
    assert!(!cluster_switch_can_render());

    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::minimal()))));
    assert!(!cluster_switch_can_render());

    openkite_ui::runtime::set_gateway(None);
    openkite_ui::runtime::set_published_capabilities(None);
    assert!(!cluster_switch_can_render());
}

#[test]
fn cluster_switch_follows_the_published_descriptor() {
    let _gate = gate_lock();
    openkite_ui::runtime::set_gateway(None);
    openkite_ui::runtime::set_published_capabilities(Some(Capabilities::in_process()));
    assert!(cluster_switch_can_render());

    openkite_ui::runtime::set_published_capabilities(Some(Capabilities::server_side()));
    assert!(!cluster_switch_can_render());
    openkite_ui::runtime::set_published_capabilities(None);
}

#[test]
fn missing_gateway_means_no_surfaces_render() {
    let _gate = gate_lock();
    openkite_ui::runtime::set_gateway(None);
    openkite_ui::runtime::set_published_capabilities(None);
    assert!(!terminal_can_render());
    assert!(!native_chrome_can_render());
    assert!(!cluster_switch_can_render());
    assert!(!mutations_can_render());
}

#[test]
fn mutations_gate_on_the_gateway_the_host_owns() {
    let _gate = gate_lock();
    // The in-process gateway applies mutations through the contract.
    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::in_process()))));
    assert!(mutations_can_render());

    // The browser host answers a read-only bridge behind a server-side gateway.
    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::server_side()))));
    assert!(!mutations_can_render());
    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::minimal()))));
    assert!(!mutations_can_render());

    openkite_ui::runtime::set_gateway(None);
    assert!(!mutations_can_render());

    // The published descriptor outranks the gateway, like every other gate.
    openkite_ui::runtime::set_published_capabilities(Some(Capabilities::in_process()));
    assert!(mutations_can_render());
    openkite_ui::runtime::set_published_capabilities(None);
}

#[test]
fn published_descriptor_gates_without_a_gateway() {
    let _gate = gate_lock();
    openkite_ui::runtime::set_gateway(None);
    openkite_ui::runtime::set_published_capabilities(Some(Capabilities::in_process()));
    assert!(terminal_can_render());
    assert!(native_chrome_can_render());

    openkite_ui::runtime::set_published_capabilities(Some(Capabilities::server_side()));
    assert!(!terminal_can_render());
    assert!(!native_chrome_can_render());

    openkite_ui::runtime::set_published_capabilities(None);
    assert!(!terminal_can_render());
    assert!(!native_chrome_can_render());
}

#[test]
fn published_descriptor_outranks_the_gateway() {
    let _gate = gate_lock();
    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::server_side()))));
    openkite_ui::runtime::set_published_capabilities(Some(Capabilities::in_process()));
    assert!(terminal_can_render());
    assert!(native_chrome_can_render());

    openkite_ui::runtime::set_gateway(Some(Arc::new(FixedGateway(Capabilities::in_process()))));
    openkite_ui::runtime::set_published_capabilities(Some(Capabilities::server_side()));
    assert!(!terminal_can_render());
    assert!(!native_chrome_can_render());

    openkite_ui::runtime::set_gateway(None);
    openkite_ui::runtime::set_published_capabilities(None);
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
