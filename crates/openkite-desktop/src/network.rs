//! Config + network resource mapping: Services, Ingress, ConfigMaps, Secrets.
//!
//! P1 surface (the helper functions): turn each resource into the pure rows
//! and cells a table view can render. P2 surface (the `ConfigKind` enum,
//! `*_columns` / `*_row` builders, and cell-text formatters) feeds the
//! `/config` Config + Network view at `src/views/config.rs`.

use std::collections::{BTreeMap, BTreeSet};

use k8s_openapi::api::core::v1::{ConfigMap, Secret, Service};
use k8s_openapi::api::networking::v1::Ingress;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;

use crate::components::resource_table::{Cell, ColumnDef, ResourceRow};
use crate::workloads::age_cell;

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

// -- P2 surface: Config + Network view table layer --

/// The four kinds the `/config` view lists. Order is the tab order
/// (ConfigMaps / Secrets / Services / Ingress) and matches the ticket title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigKind {
    ConfigMaps,
    Secrets,
    Services,
    Ingress,
}

impl ConfigKind {
    /// Every kind, in tab order.
    pub const ALL: [ConfigKind; 4] = [
        ConfigKind::ConfigMaps,
        ConfigKind::Secrets,
        ConfigKind::Services,
        ConfigKind::Ingress,
    ];

    /// Human-readable tab label. Matches `kubectl get` casing.
    pub fn label(self) -> &'static str {
        match self {
            ConfigKind::ConfigMaps => "ConfigMaps",
            ConfigKind::Secrets => "Secrets",
            ConfigKind::Services => "Services",
            ConfigKind::Ingress => "Ingress",
        }
    }

    /// Singular API kind string (matches the `kind` field of a manifest).
    /// Used by the CRUD modal's starter/Edit doc.
    pub fn kind_str(self) -> &'static str {
        match self {
            ConfigKind::ConfigMaps => "ConfigMap",
            ConfigKind::Secrets => "Secret",
            ConfigKind::Services => "Service",
            ConfigKind::Ingress => "Ingress",
        }
    }

    /// `apiVersion` for the CRUD modal's starter/Edit doc.
    pub fn api_version(self) -> &'static str {
        match self {
            ConfigKind::ConfigMaps | ConfigKind::Secrets | ConfigKind::Services => "v1",
            ConfigKind::Ingress => "networking.k8s.io/v1",
        }
    }
}

/// Column skeleton for Config/Network kinds: Name leading, the kind's middle
/// columns, Age second-to-last, Type last. Diverges from `src/workloads.rs`'s
/// `kind_columns` only in the trailing column name — these kinds have no
/// generic Status, just the kind-discriminating type cell.
fn kind_columns(middle: &[ColumnDef]) -> Vec<ColumnDef> {
    let mut columns = vec![ColumnDef {
        key: "name",
        label: "Name",
        width: None,
        sortable: true,
    }];
    columns.extend(middle.iter().cloned());
    columns.extend([
        ColumnDef {
            key: "age",
            label: "Age",
            width: Some(80),
            sortable: true,
        },
        ColumnDef {
            key: "type",
            label: "Type",
            width: Some(110),
            sortable: true,
        },
    ]);
    columns
}

/// Namespace-scoped object id (used as the row key and action target).
fn object_id(namespace: Option<&str>, name: &str) -> String {
    match namespace {
        Some(ns) => format!("{ns}/{name}"),
        None => name.to_string(),
    }
}

// -- Cell-text formatters (pure, unit-testable) --

/// `"3 keys: app.conf, env +1"` — first 2 keys joined; the count is the full
/// count, with a `+N` suffix when there are more. Used in the ConfigMap
/// "Data" cell.
pub fn config_data_preview(entries: &[(String, String)]) -> String {
    if entries.is_empty() {
        return "—".into();
    }
    let count = entries.len();
    let preview: Vec<&str> = entries.iter().take(2).map(|(k, _)| k.as_str()).collect();
    let preview_str = preview.join(", ");
    if count > 2 {
        format!("{} keys: {} +{}", count, preview_str, count - 2)
    } else {
        format!("{} keys: {}", count, preview_str)
    }
}

/// `"80→8080, 443→https"` — one entry per port, `→` separator, named target
/// ports stringify verbatim. Used in the Service "Ports" cell.
pub fn format_ports_summary(ports: &[ServicePortRow]) -> String {
    if ports.is_empty() {
        return "—".into();
    }
    let parts: Vec<String> = ports
        .iter()
        .map(|p| {
            if p.target_port == p.port.to_string() {
                p.port.to_string()
            } else {
                format!("{}→{}", p.port, p.target_port)
            }
        })
        .collect();
    parts.join(", ")
}

/// `"app=api"` or `"app=api, tier=frontend"` — flat k=v comma list, stable
/// ordering via the BTreeMap from `service_summary`. Truncate at 3 keys.
pub fn format_selector_short(selector: &BTreeMap<String, String>) -> String {
    if selector.is_empty() {
        return "—".into();
    }
    let pairs: Vec<String> = selector
        .iter()
        .take(3)
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    let mut s = pairs.join(", ");
    if selector.len() > 3 {
        s.push_str(&format!(" +{}", selector.len() - 3));
    }
    s
}

/// `"example.com/api, other.com"` — host + path joined, one per rule.
/// Rules with no http paths render as just the host.
pub fn format_ingress_paths(rules: &[IngressRuleRow]) -> String {
    if rules.is_empty() {
        return "—".into();
    }
    let parts: Vec<String> = rules
        .iter()
        .map(|r| {
            if r.path.is_empty() {
                r.host.clone()
            } else {
                format!("{}/{}", r.host, r.path.trim_start_matches('/'))
            }
        })
        .collect();
    parts.join(", ")
}

/// Unique hosts joined: `"example.com, other.com"`. Used in the Ingress
/// "Hosts" cell.
pub fn format_ingress_hosts(rules: &[IngressRuleRow]) -> String {
    let unique: BTreeSet<&str> = rules.iter().map(|r| r.host.as_str()).collect();
    if unique.is_empty() {
        "—".into()
    } else {
        unique.into_iter().collect::<Vec<_>>().join(", ")
    }
}

/// Number of keys in a Secret's data + string_data. Cheap, maskable.
pub fn secret_key_count(secret: &Secret) -> usize {
    secret_keys(secret).len()
}

// -- Column definitions (one per kind) --

/// ConfigMap columns: Name / Data / Keys / Age / Type.
pub fn config_map_columns() -> Vec<ColumnDef> {
    kind_columns(&[
        ColumnDef {
            key: "data",
            label: "Data",
            width: Some(220),
            sortable: true,
        },
        ColumnDef {
            key: "keys",
            label: "Keys",
            width: Some(80),
            sortable: true,
        },
    ])
}

/// Secret columns: Name / Type (the Secret's type) / Keys (count) / Age / Type.
/// No value column: the table view is permanently masked.
pub fn secret_columns() -> Vec<ColumnDef> {
    kind_columns(&[
        ColumnDef {
            key: "type",
            label: "Type",
            width: Some(140),
            sortable: true,
        },
        ColumnDef {
            key: "keys",
            label: "Keys",
            width: Some(80),
            sortable: true,
        },
    ])
}

/// Service columns: Name / Svc Type / Cluster IP / Ports / Selector / Age / Type.
pub fn service_columns() -> Vec<ColumnDef> {
    kind_columns(&[
        ColumnDef {
            key: "svc_type",
            label: "Svc Type",
            width: Some(120),
            sortable: true,
        },
        ColumnDef {
            key: "cluster_ip",
            label: "Cluster IP",
            width: Some(140),
            sortable: true,
        },
        ColumnDef {
            key: "ports",
            label: "Ports",
            width: Some(220),
            sortable: true,
        },
        ColumnDef {
            key: "selector",
            label: "Selector",
            width: Some(200),
            sortable: true,
        },
    ])
}

/// Ingress columns: Name / Class / Hosts / Paths / Age / Type.
pub fn ingress_columns() -> Vec<ColumnDef> {
    kind_columns(&[
        ColumnDef {
            key: "class",
            label: "Class",
            width: Some(120),
            sortable: true,
        },
        ColumnDef {
            key: "hosts",
            label: "Hosts",
            width: Some(200),
            sortable: true,
        },
        ColumnDef {
            key: "paths",
            label: "Paths",
            width: Some(280),
            sortable: true,
        },
    ])
}

// -- Row mappers (one per kind) --

/// Build a ConfigMap row. The "Data" cell carries a key count + first 2
/// keys (no values).
pub fn config_map_row(cm: &ConfigMap) -> ResourceRow {
    let entries = config_map_entries(cm);
    let name = cm.metadata.name.clone().unwrap_or_default();
    let namespace = cm.metadata.namespace.clone();
    let data_preview = config_data_preview(&entries);
    let keys_count = entries.len() as f64;
    let age = age_cell(&cm.metadata.creation_timestamp);
    ResourceRow {
        id: object_id(namespace.as_deref(), &name),
        namespace,
        cells: vec![
            Cell::text(name),
            Cell::text(data_preview),
            Cell::number(entries.len().to_string(), keys_count),
            age,
            Cell::text("ConfigMap"),
        ],
    }
}

/// Build a Secret row. Values are NEVER read — only the key count.
pub fn secret_row(secret: &Secret) -> ResourceRow {
    let name = secret.metadata.name.clone().unwrap_or_default();
    let namespace = secret.metadata.namespace.clone();
    let type_ = secret.type_.clone().unwrap_or_else(|| "Opaque".to_string());
    let key_count = secret_key_count(secret);
    let age = age_cell(&secret.metadata.creation_timestamp);
    ResourceRow {
        id: object_id(namespace.as_deref(), &name),
        namespace,
        cells: vec![
            Cell::text(name),
            Cell::text(type_),
            Cell::number(key_count.to_string(), key_count as f64),
            age,
            Cell::text("Secret"),
        ],
    }
}

/// Build a Service row: Name / Svc Type / Cluster IP / Ports / Selector /
/// Age / Type.
pub fn service_row(service: &Service) -> ResourceRow {
    let name = service.metadata.name.clone().unwrap_or_default();
    let namespace = service.metadata.namespace.clone();
    let summary = service_summary(service);
    let ports = service_ports(service);
    let ports_text = format_ports_summary(&ports);
    let selector_text = format_selector_short(&summary.selector);
    let cluster_ip = if summary.cluster_ip.is_empty() {
        "—".to_string()
    } else {
        summary.cluster_ip.clone()
    };
    let age = age_cell(&service.metadata.creation_timestamp);
    ResourceRow {
        id: object_id(namespace.as_deref(), &name),
        namespace,
        cells: vec![
            Cell::text(name),
            Cell::text(summary.type_),
            Cell::text(cluster_ip),
            Cell::text(ports_text),
            Cell::text(selector_text),
            age,
            Cell::text("Service"),
        ],
    }
}

/// Build an Ingress row: Name / Class / Hosts / Paths / Age / Type.
pub fn ingress_row(ingress: &Ingress) -> ResourceRow {
    let name = ingress.metadata.name.clone().unwrap_or_default();
    let namespace = ingress.metadata.namespace.clone();
    let rules = ingress_rules(ingress);
    let class = ingress
        .spec
        .as_ref()
        .and_then(|s| s.ingress_class_name.clone())
        .unwrap_or_else(|| "—".to_string());
    let hosts_text = format_ingress_hosts(&rules);
    let paths_text = format_ingress_paths(&rules);
    let age = age_cell(&ingress.metadata.creation_timestamp);
    ResourceRow {
        id: object_id(namespace.as_deref(), &name),
        namespace,
        cells: vec![
            Cell::text(name),
            Cell::text(class),
            Cell::text(hosts_text),
            Cell::text(paths_text),
            age,
            Cell::text("Ingress"),
        ],
    }
}
