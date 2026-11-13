//! Discovery documents served both ways.

mod common;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::APIResource;

#[tokio::test]
async fn aggregated_discovery_lists_core_resources() {
    let (server, _) = common::with_pods(0).await;
    let client = server.client();

    let discovery = client
        .list_core_api_versions_aggregated()
        .await
        .expect("aggregated core discovery");
    let core = discovery.items.first().expect("core group");
    let version = core.versions.first().expect("v1");
    let resources: Vec<&str> = version
        .resources
        .iter()
        .filter_map(|resource| resource.resource.as_deref())
        .collect();

    assert_eq!(version.version.as_deref(), Some("v1"));
    assert!(resources.contains(&"pods"));
    assert!(resources.contains(&"nodes"));
    server.shutdown().await;
}

#[tokio::test]
async fn aggregated_discovery_carries_scope_and_kind() {
    let (server, _) = common::with_pods(0).await;
    let client = server.client();

    let discovery = client
        .list_core_api_versions_aggregated()
        .await
        .expect("aggregated core discovery");
    let resources = &discovery.items[0].versions[0].resources;

    let pods = resources
        .iter()
        .find(|resource| resource.resource.as_deref() == Some("pods"))
        .expect("pods");
    assert_eq!(pods.scope.as_deref(), Some("Namespaced"));
    let kind = pods.response_kind.as_ref().expect("response kind");
    assert_eq!(kind.kind.as_deref(), Some("Pod"));

    let nodes = resources
        .iter()
        .find(|resource| resource.resource.as_deref() == Some("nodes"))
        .expect("nodes");
    assert_eq!(nodes.scope.as_deref(), Some("Cluster"));

    server.shutdown().await;
}

#[tokio::test]
async fn aggregated_discovery_lists_named_groups() {
    let (server, _) = common::with_pods(0).await;
    let client = server.client();

    let discovery = client
        .list_api_groups_aggregated()
        .await
        .expect("aggregated group discovery");
    let apps = discovery
        .items
        .iter()
        .find(|group| {
            group
                .metadata
                .as_ref()
                .and_then(|meta| meta.name.as_deref())
                == Some("apps")
        })
        .expect("apps group");
    let version = apps.versions.first().expect("apps/v1");
    assert_eq!(version.version.as_deref(), Some("v1"));
    assert!(version
        .resources
        .iter()
        .any(|resource| resource.resource.as_deref() == Some("deployments")));

    server.shutdown().await;
}

#[tokio::test]
async fn legacy_discovery_lists_core_resources() {
    let (server, _) = common::with_pods(0).await;
    let client = server.client();

    let list = client
        .list_core_api_resources("v1")
        .await
        .expect("legacy core discovery");
    let pods = find(&list.resources, "pods").expect("pods");
    assert_eq!(list.group_version, "v1");
    assert_eq!(pods.kind, "Pod");
    assert!(pods.namespaced);
    assert!(pods.verbs.iter().any(|verb| verb == "list"));
    assert!(find(&list.resources, "nodes").is_some_and(|nodes| !nodes.namespaced));

    server.shutdown().await;
}

#[tokio::test]
async fn legacy_discovery_lists_named_groups() {
    let (server, _) = common::with_pods(0).await;
    let client = server.client();

    let groups = client.list_api_groups().await.expect("legacy groups");
    let apps = groups
        .groups
        .iter()
        .find(|group| group.name == "apps")
        .expect("apps group");
    assert!(apps
        .versions
        .iter()
        .any(|version| version.group_version == "apps/v1"));

    let list = client
        .list_api_group_resources("apps/v1")
        .await
        .expect("legacy group discovery");
    assert_eq!(list.group_version, "apps/v1");
    assert!(find(&list.resources, "deployments").is_some());

    server.shutdown().await;
}

fn find<'a>(resources: &'a [APIResource], name: &str) -> Option<&'a APIResource> {
    resources.iter().find(|resource| resource.name == name)
}
