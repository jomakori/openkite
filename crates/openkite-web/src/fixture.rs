//! A mock gateway serving fixture data to a preview host, which has no cluster credential.

use std::sync::Arc;

use openkite_api::capability::Capabilities;
use openkite_api::gateway::{Gateway, GatewayError, GatewayFuture};
use openkite_api::pod::{ContainerInfo, PodObject, PodSummary};
use openkite_api::secret::SecretRef;

/// A fixture pod that looks real enough to render the console.
fn fixture_pod_object() -> PodObject {
    PodObject {
        name: "example-pod".to_string(),
        namespace: Some("default".to_string()),
        summary: PodSummary {
            phase: "Running".to_string(),
            node: "node-1".to_string(),
            pod_ip: "10.0.0.1".to_string(),
            qos: "BestEffort".to_string(),
            reason: None,
            message: None,
        },
        containers: vec![ContainerInfo {
            name: "app".to_string(),
            image: "openkite:latest".to_string(),
            ready: true,
            restarts: 0,
            state: "Running".to_string(),
        }],
        labels: [
            ("app".to_string(), "openkite".to_string()),
            ("version".to_string(), "demo".to_string()),
        ]
        .into(),
        annotations: [(
            "description".to_string(),
            "Fixture pod for preview".to_string(),
        )]
        .into(),
        yaml: r#"apiVersion: v1
kind: Pod
metadata:
  name: example-pod
  namespace: default
  labels:
    app: openkite
    version: demo
  annotations:
    description: Fixture pod for preview
spec:
  containers:
  - name: app
    image: openkite:latest
status:
  phase: Running
  podIP: 10.0.0.1
  qosClass: BestEffort
"#
        .to_string(),
    }
}

/// A fixture gateway serving demo data when no cluster is available.
pub struct FixtureGateway;

impl Gateway for FixtureGateway {
    fn capabilities(&self) -> Capabilities {
        Capabilities::server_side()
    }

    fn secret(
        &self,
        _namespace: &str,
        _name: &str,
    ) -> GatewayFuture<openkite_api::secret::SecretObject> {
        let err = Err(GatewayError::NotFound(
            "fixtures do not support secret access".to_string(),
        ));
        Box::pin(async move { err })
    }

    fn list_secrets(&self, _namespace: Option<&str>) -> GatewayFuture<Vec<SecretRef>> {
        Box::pin(async { Ok(vec![]) })
    }

    fn pod(&self, _namespace: &str, _name: &str) -> GatewayFuture<openkite_api::pod::PodObject> {
        let pod = fixture_pod_object();
        Box::pin(async move { Ok(pod) })
    }

    fn list_pods(
        &self,
        _namespace: Option<&str>,
    ) -> GatewayFuture<Vec<openkite_api::pod::PodObject>> {
        let pods = vec![fixture_pod_object()];
        Box::pin(async move { Ok(pods) })
    }

    fn capability(&self, _name: &str) -> GatewayFuture<bool> {
        Box::pin(async { Ok(false) })
    }

    fn apply_mutation(&self, _mutation: &openkite_api::crud::Mutation) -> GatewayFuture<()> {
        Box::pin(async {
            Err(GatewayError::Forbidden(
                "fixture gateway does not support mutations".to_string(),
            ))
        })
    }
}

/// Create a fixture gateway wrapped in Arc for injection into routes.
pub fn make_fixture_gateway() -> Arc<dyn Gateway> {
    Arc::new(FixtureGateway)
}
