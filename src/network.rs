//! Config + network resource mapping: Services, Ingress, ConfigMaps, Secrets.

use std::collections::{BTreeMap, BTreeSet};

use k8s_openapi::api::core::v1::{ConfigMap, Secret, Service};
use k8s_openapi::api::networking::v1::Ingress;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;

/// A single service port row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServicePortRow {
    pub name: String,
    pub port: i32,
    pub target_port: String,
    pub protocol: String,
    pub node_port: Option<i32>,
}

/// A service's high-level summary.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceSummary {
    pub type_: String,
    pub cluster_ip: String,
    pub external_name: String,
    pub selector: BTreeMap<String, String>,
}

/// A single ingress routing rule row (one per path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngressRuleRow {
    pub host: String,
    pub path: String,
    pub path_type: String,
    pub backend: String,
}

/// Format an `IntOrString` (a port can be a number or a named port).
fn int_or_string(v: &IntOrString) -> String {
    match v {
        IntOrString::Int(i) => i.to_string(),
        IntOrString::String(s) => s.clone(),
    }
}

/// Extract the service port rows from a Service.
pub fn service_ports(service: &Service) -> Vec<ServicePortRow> {
    service
        .spec
        .as_ref()
        .and_then(|s| s.ports.as_deref())
        .unwrap_or_default()
        .iter()
        .map(|p| ServicePortRow {
            name: p.name.clone().unwrap_or_default(),
            port: p.port,
            target_port: p
                .target_port
                .as_ref()
                .map(int_or_string)
                .unwrap_or_default(),
            protocol: p.protocol.clone().unwrap_or_else(|| "TCP".to_string()),
            node_port: p.node_port,
        })
        .collect()
}

/// Extract a Service's high-level summary.
pub fn service_summary(service: &Service) -> ServiceSummary {
    let spec = service.spec.as_ref();
    ServiceSummary {
        type_: spec
            .and_then(|s| s.type_.clone())
            .unwrap_or_else(|| "ClusterIP".to_string()),
        cluster_ip: spec.and_then(|s| s.cluster_ip.clone()).unwrap_or_default(),
        external_name: spec
            .and_then(|s| s.external_name.clone())
            .unwrap_or_default(),
        selector: spec.and_then(|s| s.selector.clone()).unwrap_or_default(),
    }
}

/// Extract the routing rules from an Ingress (one row per path).
pub fn ingress_rules(ingress: &Ingress) -> Vec<IngressRuleRow> {
    let mut rows = Vec::new();
    let Some(spec) = ingress.spec.as_ref() else {
        return rows;
    };
    let Some(rules) = spec.rules.as_deref() else {
        return rows;
    };
    for rule in rules {
        let host = rule.host.clone().unwrap_or_default();
        if let Some(http) = &rule.http {
            for path in &http.paths {
                rows.push(IngressRuleRow {
                    host: host.clone(),
                    path: path.path.clone().unwrap_or_default(),
                    path_type: path.path_type.clone(),
                    backend: path
                        .backend
                        .service
                        .as_ref()
                        .map(|s| s.name.clone())
                        .unwrap_or_default(),
                });
            }
        }
    }
    rows
}

/// Extract a ConfigMap's data entries (sorted by key).
pub fn config_map_entries(config_map: &ConfigMap) -> Vec<(String, String)> {
    config_map
        .data
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect()
}

/// The keys present in a Secret (from `data` and `string_data`), sorted.
pub fn secret_keys(secret: &Secret) -> Vec<String> {
    let mut keys = BTreeSet::new();
    if let Some(data) = &secret.data {
        keys.extend(data.keys().cloned());
    }
    if let Some(string_data) = &secret.string_data {
        keys.extend(string_data.keys().cloned());
    }
    keys.into_iter().collect()
}
