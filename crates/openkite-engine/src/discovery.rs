//! Discovery: fetch the served API surface via aggregated discovery, falling back to the legacy fan-out.
use k8s_openapi::apimachinery::pkg::apis::meta::v1::APIResourceList;
use kube::core::discovery::v2::{
    APIGroupDiscovery, APIGroupDiscoveryList, APIResourceDiscovery, APIVersionDiscovery,
};
use kube::Client;

use crate::catalogue::{Catalogue, CatalogueEntry};
use crate::error::EngineError;

/// Fetch every served GVK from the core group and every other API group.
pub async fn fetch(client: &Client) -> Result<Catalogue, EngineError> {
    let mut entries = core_entries(client).await?;
    entries.extend(group_entries(client).await?);
    Ok(Catalogue { entries })
}

async fn core_entries(client: &Client) -> Result<Vec<CatalogueEntry>, EngineError> {
    match client.list_core_api_versions_aggregated().await {
        Ok(list) if !list.items.is_empty() => Ok(entries_from_aggregated(list)),
        _ => legacy_core_entries(client).await,
    }
}

async fn group_entries(client: &Client) -> Result<Vec<CatalogueEntry>, EngineError> {
    match client.list_api_groups_aggregated().await {
        Ok(list) if !list.items.is_empty() => Ok(entries_from_aggregated(list)),
        _ => legacy_group_entries(client).await,
    }
}

async fn legacy_core_entries(client: &Client) -> Result<Vec<CatalogueEntry>, EngineError> {
    let versions = client
        .list_core_api_versions()
        .await
        .map_err(EngineError::Discovery)?;
    let mut entries = Vec::new();
    for version in versions.versions {
        let list = client
            .list_core_api_resources(&version)
            .await
            .map_err(EngineError::Discovery)?;
        entries.extend(entries_from_resource_list(String::new(), &version, list));
    }
    Ok(entries)
}

async fn legacy_group_entries(client: &Client) -> Result<Vec<CatalogueEntry>, EngineError> {
    let groups = client
        .list_api_groups()
        .await
        .map_err(EngineError::Discovery)?;
    let mut entries = Vec::new();
    for group in groups.groups {
        for version in &group.versions {
            let list = client
                .list_api_group_resources(&version.group_version)
                .await
                .map_err(EngineError::Discovery)?;
            entries.extend(entries_from_resource_list(
                group.name.clone(),
                &version.version,
                list,
            ));
        }
    }
    Ok(entries)
}

fn entries_from_resource_list(
    group: String,
    version: &str,
    list: APIResourceList,
) -> Vec<CatalogueEntry> {
    list.resources
        .into_iter()
        .filter(|resource| !resource.name.contains('/'))
        .map(|resource| CatalogueEntry {
            group: group.clone(),
            version: version.to_owned(),
            kind: resource.kind,
            plural: resource.name,
            singular: resource.singular_name,
            namespaced: resource.namespaced,
            verbs: resource.verbs,
            short_names: resource.short_names.unwrap_or_default(),
            categories: resource.categories.unwrap_or_default(),
            additional_printer_columns: Vec::new(),
        })
        .collect()
}

fn entries_from_aggregated(list: APIGroupDiscoveryList) -> Vec<CatalogueEntry> {
    list.items
        .into_iter()
        .flat_map(group_entries_from_v2)
        .collect()
}

fn group_entries_from_v2(group: APIGroupDiscovery) -> Vec<CatalogueEntry> {
    let name = group.metadata.and_then(|m| m.name).unwrap_or_default();
    group
        .versions
        .into_iter()
        .flat_map(move |version| version_entries_from_v2(&name, version))
        .collect()
}

fn version_entries_from_v2(group_name: &str, version: APIVersionDiscovery) -> Vec<CatalogueEntry> {
    let version_name = version.version.unwrap_or_default();
    version
        .resources
        .into_iter()
        .map(move |resource| entry_from_v2(group_name, &version_name, resource))
        .collect()
}

fn entry_from_v2(
    group_name: &str,
    version_name: &str,
    resource: APIResourceDiscovery,
) -> CatalogueEntry {
    let kind = resource
        .response_kind
        .and_then(|k| k.kind)
        .unwrap_or_default();
    CatalogueEntry {
        group: group_name.to_owned(),
        version: version_name.to_owned(),
        kind,
        plural: resource.resource.unwrap_or_default(),
        singular: resource.singular_resource.unwrap_or_default(),
        namespaced: resource.scope.as_deref() == Some("Namespaced"),
        verbs: resource.verbs,
        short_names: resource.short_names,
        categories: resource.categories,
        additional_printer_columns: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AGGREGATED: &str = r#"{
      "kind": "APIGroupDiscoveryList",
      "apiVersion": "apidiscovery.k8s.io/v2",
      "metadata": {},
      "items": [
        {
          "metadata": {"name": ""},
          "versions": [
            {
              "version": "v1",
              "resources": [
                {
                  "resource": "pods",
                  "responseKind": {"group": "", "version": "v1", "kind": "Pod"},
                  "scope": "Namespaced",
                  "singularResource": "pod",
                  "verbs": ["get", "list", "watch"],
                  "shortNames": ["po"],
                  "categories": ["all"]
                },
                {
                  "resource": "nodes",
                  "responseKind": {"group": "", "version": "v1", "kind": "Node"},
                  "scope": "Cluster",
                  "singularResource": "node",
                  "verbs": ["get", "list", "watch"],
                  "shortNames": ["no"]
                }
              ]
            }
          ]
        },
        {
          "metadata": {"name": "apps"},
          "versions": [
            {
              "version": "v1",
              "resources": [
                {
                  "resource": "deployments",
                  "responseKind": {"group": "apps", "version": "v1", "kind": "Deployment"},
                  "scope": "Namespaced",
                  "singularResource": "deployment",
                  "verbs": ["get", "list", "watch", "create"]
                }
              ]
            }
          ]
        }
      ]
    }"#;

    const LEGACY: &str = r#"{
      "kind": "APIResourceList",
      "apiVersion": "v1",
      "groupVersion": "v1",
      "resources": [
        {
          "name": "pods",
          "singularName": "pod",
          "namespaced": true,
          "kind": "Pod",
          "verbs": ["get", "list"],
          "shortNames": ["po"],
          "categories": ["all"]
        },
        {
          "name": "pods/status",
          "singularName": "",
          "namespaced": true,
          "kind": "Pod",
          "verbs": ["get", "patch"]
        }
      ]
    }"#;

    #[test]
    fn aggregated_discovery_yields_one_entry_per_gvk() {
        let list: APIGroupDiscoveryList = serde_json::from_str(AGGREGATED).expect("fixture");
        let entries = entries_from_aggregated(list);
        assert_eq!(entries.len(), 3);

        let pods = entries
            .iter()
            .find(|entry| entry.plural == "pods")
            .expect("pods");
        assert_eq!(pods.group, "");
        assert_eq!(pods.version, "v1");
        assert_eq!(pods.kind, "Pod");
        assert_eq!(pods.singular, "pod");
        assert!(pods.namespaced);
        assert_eq!(pods.short_names, ["po"]);
        assert_eq!(pods.categories, ["all"]);
        assert!(pods.verbs.contains(&"watch".to_owned()));

        let nodes = entries
            .iter()
            .find(|entry| entry.plural == "nodes")
            .expect("nodes");
        assert!(!nodes.namespaced);

        let deployments = entries
            .iter()
            .find(|entry| entry.plural == "deployments")
            .expect("deployments");
        assert_eq!(deployments.group, "apps");
        assert_eq!(deployments.kind, "Deployment");
    }

    #[test]
    fn legacy_discovery_skips_subresources() {
        let list: APIResourceList = serde_json::from_str(LEGACY).expect("fixture");
        let entries = entries_from_resource_list(String::new(), "v1", list);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].plural, "pods");
        assert_eq!(entries[0].singular, "pod");
        assert!(entries[0].namespaced);
        assert_eq!(entries[0].short_names, ["po"]);
    }

    #[test]
    fn legacy_entries_carry_the_group_they_came_from() {
        let list: APIResourceList = serde_json::from_str(LEGACY).expect("fixture");
        let entries = entries_from_resource_list("apps".to_owned(), "v1", list);
        assert_eq!(entries[0].group, "apps");
    }
}
