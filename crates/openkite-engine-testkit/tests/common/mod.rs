//! Shared fixtures for the testkit integration tests.
#![allow(dead_code)]

use std::io;

use openkite_engine_testkit::{FakeApiserver, ResourceDef, Scenario};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Core `pods`, namespaced.
pub fn pods() -> ResourceDef {
    ResourceDef::new("", "v1", "Pod", "pods", true)
}

/// Core `nodes`, cluster scoped.
pub fn nodes() -> ResourceDef {
    ResourceDef::new("", "v1", "Node", "nodes", false)
}

/// `apps/v1` `deployments`, namespaced.
pub fn deployments() -> ResourceDef {
    ResourceDef::new("apps", "v1", "Deployment", "deployments", true)
}

/// A scenario serving those three resources, with `count` pods in namespace `apps`.
pub fn scenario(count: usize) -> (Scenario, ResourceDef) {
    let pods = pods();
    let mut scenario = Scenario::new()
        .with_resource(pods.clone())
        .with_resource(nodes())
        .with_resource(deployments());
    for index in 0..count {
        scenario = scenario.add(
            &pods,
            json!({
                "apiVersion": "v1",
                "kind": "Pod",
                "metadata": {
                    "name": format!("pod-{index:02}"),
                    "namespace": "apps",
                    "labels": {"tier": "web"},
                },
                "spec": {"containers": [{"name": "app", "image": "busybox"}]},
            }),
        );
    }
    (scenario, pods)
}

/// A running fake apiserver with `count` pods seeded.
pub async fn with_pods(count: usize) -> (FakeApiserver, ResourceDef) {
    let (scenario, pods) = scenario(count);
    (FakeApiserver::start(scenario).await, pods)
}

/// Sends one HTTP/1.1 GET and returns the whole response as text.
pub async fn raw_get(base_url: &str, path: &str, accept: &str) -> io::Result<String> {
    let addr = base_url.trim_start_matches("http://").trim_end_matches('/');
    let mut stream = tokio::net::TcpStream::connect(addr).await?;
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nAccept: {accept}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    Ok(String::from_utf8_lossy(&response).into_owned())
}
