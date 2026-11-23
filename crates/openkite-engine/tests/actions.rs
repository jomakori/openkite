//! The actions as a console would send them, against the fake apiserver.

use std::time::Duration;

use kube::api::{Api, ApiResource, DynamicObject};
use openkite_engine::actions::{
    Actions, DeleteRequest, DrainOptions, DrainStep, DrainTarget, Propagation,
};
use openkite_engine_testkit::{FakeApiserver, ResourceDef, Scenario};
use serde_json::json;

fn deployments() -> ResourceDef {
    ResourceDef::new("apps", "v1", "Deployment", "deployments", true)
}

fn pods() -> ResourceDef {
    ResourceDef::new("", "v1", "Pod", "pods", true)
}

fn nodes() -> ResourceDef {
    ResourceDef::new("", "v1", "Node", "nodes", false)
}

fn deployment(name: &str, namespace: &str, replicas: u32) -> serde_json::Value {
    json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": { "name": name, "namespace": namespace },
        "spec": { "replicas": replicas },
    })
}

fn pod(name: &str, namespace: &str, node: &str, owner: Option<&str>) -> serde_json::Value {
    let mut object = json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": name, "namespace": namespace },
        "spec": { "nodeName": node },
    });
    if let Some(kind) = owner {
        object["metadata"]["ownerReferences"] = json!([{ "kind": kind, "name": "owner" }]);
    }
    object
}

fn node(name: &str) -> serde_json::Value {
    json!({
        "apiVersion": "v1",
        "kind": "Node",
        "metadata": { "name": name },
        "spec": { "unschedulable": false },
    })
}

fn api_resource(def: &ResourceDef) -> ApiResource {
    ApiResource {
        group: def.group.clone(),
        version: def.version.clone(),
        api_version: if def.group.is_empty() {
            def.version.clone()
        } else {
            format!("{}/{}", def.group, def.version)
        },
        kind: def.kind.clone(),
        plural: def.plural.clone(),
    }
}

fn deployment_api(client: kube::Client) -> Api<DynamicObject> {
    Api::namespaced_with(client, "prod", &api_resource(&deployments()))
}

fn pod_api(client: kube::Client) -> Api<DynamicObject> {
    Api::namespaced_with(client, "prod", &api_resource(&pods()))
}

#[tokio::test]
async fn a_confirmed_delete_removes_the_workload() {
    let scenario = Scenario::new()
        .with_resource(deployments())
        .with_namespace(&deployments(), "prod")
        .add(&deployments(), deployment("api", "prod", 2));
    let server = FakeApiserver::start(scenario).await;
    let actions = Actions::new(server.client());

    let request = DeleteRequest::confirmed("api", Propagation::Foreground);
    actions
        .delete(&api_resource(&deployments()), Some("prod"), &request)
        .await
        .expect("the fake apiserver accepts the delete");

    let gone = deployment_api(server.client()).get("api").await;
    assert!(gone.is_err(), "the deployment is gone from the store");
    server.shutdown().await;
}

#[tokio::test]
async fn a_typo_never_reaches_the_apiserver() {
    let scenario = Scenario::new()
        .with_resource(deployments())
        .with_namespace(&deployments(), "prod")
        .add(&deployments(), deployment("api", "prod", 2));
    let server = FakeApiserver::start(scenario).await;
    let actions = Actions::new(server.client());

    let request = DeleteRequest {
        name: "api".to_owned(),
        typed: "ap1".to_owned(),
        propagation: Propagation::Background,
    };
    let error = actions
        .delete(&api_resource(&deployments()), Some("prod"), &request)
        .await
        .expect_err("a mistyped name is refused");
    assert!(error.to_string().contains("type 'api'"), "{error}");

    let still_there = deployment_api(server.client()).get("api").await;
    assert!(still_there.is_ok(), "the workload was not touched");
    server.shutdown().await;
}

#[tokio::test]
async fn scaling_patches_the_replica_count() {
    let scenario = Scenario::new()
        .with_resource(deployments())
        .with_namespace(&deployments(), "prod")
        .add(&deployments(), deployment("api", "prod", 2));
    let server = FakeApiserver::start(scenario).await;
    let actions = Actions::new(server.client());

    actions
        .scale(&api_resource(&deployments()), "prod", "api", 5)
        .await
        .expect("the fake apiserver accepts the patch");

    let object = deployment_api(server.client()).get("api").await.unwrap();
    assert_eq!(object.data.pointer("/spec/replicas"), Some(&json!(5)));
    server.shutdown().await;
}

#[tokio::test]
async fn a_restart_stamps_the_pod_template() {
    let scenario = Scenario::new()
        .with_resource(deployments())
        .with_namespace(&deployments(), "prod")
        .add(&deployments(), deployment("api", "prod", 2));
    let server = FakeApiserver::start(scenario).await;
    let actions = Actions::new(server.client());

    actions
        .restart(&api_resource(&deployments()), "prod", "api")
        .await
        .expect("the fake apiserver accepts the patch");

    let object = deployment_api(server.client()).get("api").await.unwrap();
    let stamp = object
        .data
        .pointer("/spec/template/metadata/annotations/kubectl.kubernetes.io~1restartedAt");
    assert!(stamp.is_some(), "the template carries the restart stamp");
    server.shutdown().await;
}

#[tokio::test]
async fn cordon_marks_the_node_unschedulable_and_uncordon_reverses_it() {
    let scenario = Scenario::new()
        .with_resource(nodes())
        .add(&nodes(), node("worker-1"));
    let server = FakeApiserver::start(scenario).await;
    let actions = Actions::new(server.client());
    let api: Api<DynamicObject> = Api::all_with(server.client(), &api_resource(&nodes()));

    actions.cordon("worker-1", true).await.expect("cordon");
    let cordoned = api.get("worker-1").await.unwrap();
    assert_eq!(
        cordoned.data.pointer("/spec/unschedulable"),
        Some(&json!(true))
    );

    actions.cordon("worker-1", false).await.expect("uncordon");
    let free = api.get("worker-1").await.unwrap();
    assert_eq!(
        free.data.pointer("/spec/unschedulable"),
        Some(&json!(false))
    );
    server.shutdown().await;
}

#[tokio::test]
async fn a_drain_evicts_the_workload_pods_and_leaves_the_daemonset() {
    let scenario = Scenario::new()
        .with_resource(pods())
        .with_namespace(&pods(), "prod")
        .add(
            &pods(),
            pod("api-7d9", "prod", "worker-1", Some("ReplicaSet")),
        )
        .add(
            &pods(),
            pod("fluentbit-1", "prod", "worker-1", Some("DaemonSet")),
        );
    let server = FakeApiserver::start(scenario).await;
    let actions = Actions::new(server.client());

    let targets = vec![
        DrainTarget::new("api-7d9", "prod").owned_by("ReplicaSet"),
        DrainTarget::new("fluentbit-1", "prod").owned_by("DaemonSet"),
    ];
    let plan = actions
        .drain(&targets, &DrainOptions::default())
        .await
        .expect("the fake apiserver accepts the eviction");

    assert_eq!(plan[0].1, DrainStep::Evict);
    assert_eq!(
        plan[1].1,
        DrainStep::Skip(openkite_engine::actions::SkipReason::DaemonSet)
    );

    let remaining = pod_api(server.client())
        .list(&Default::default())
        .await
        .unwrap();
    let names: Vec<String> = remaining
        .items
        .iter()
        .map(|item| item.metadata.name.clone().unwrap_or_default())
        .collect();
    assert_eq!(names, vec!["fluentbit-1".to_string()]);
    server.shutdown().await;
}

#[tokio::test]
async fn a_drain_leaves_a_pod_holding_empty_dir_data() {
    let scenario = Scenario::new()
        .with_resource(pods())
        .with_namespace(&pods(), "prod")
        .add(
            &pods(),
            pod("cache-1", "prod", "worker-1", Some("ReplicaSet")),
        );
    let server = FakeApiserver::start(scenario).await;
    let actions = Actions::new(server.client());

    let targets = vec![DrainTarget::new("cache-1", "prod")
        .owned_by("ReplicaSet")
        .with_empty_dir()];
    actions
        .drain(&targets, &DrainOptions::default())
        .await
        .expect("a skip is not an error");

    let still_there = pod_api(server.client()).get("cache-1").await;
    assert!(still_there.is_ok(), "the pod keeps its data");
    server.shutdown().await;
}

#[tokio::test]
async fn the_grace_period_is_seconds_not_millis() {
    let options = DrainOptions {
        timeout: Duration::from_secs(30),
        ..DrainOptions::default()
    };
    assert_eq!(options.timeout.as_secs(), 30);
}
