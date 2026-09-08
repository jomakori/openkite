//! Integration tests for config + network resource mapping.

use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::{ConfigMap, Secret, Service, ServicePort, ServiceSpec};
use k8s_openapi::api::networking::v1::{
    HTTPIngressPath, HTTPIngressRuleValue, Ingress, IngressBackend, IngressRule,
    IngressServiceBackend, IngressSpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;

use openkite::network::{
    config_data_preview, config_map_entries, config_map_row, format_ingress_hosts,
    format_ingress_paths, format_ports_summary, format_selector_short, ingress_row, ingress_rules,
    secret_key_count, secret_keys, secret_row, service_ports, service_row, service_summary,
    IngressRuleRow, ServicePortRow,
};

fn service(ports: Vec<ServicePort>, type_: &str, selector: &[(&str, &str)]) -> Service {
    Service {
        metadata: ObjectMeta::default(),
        spec: Some(ServiceSpec {
            type_: Some(type_.to_string()),
            selector: Some(
                selector
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ),
            ports: Some(ports),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn service_ports_extract_name_and_target() {
    let svc = service(
        vec![
            ServicePort {
                name: Some("http".into()),
                port: 80,
                target_port: Some(IntOrString::Int(8080)),
                protocol: Some("TCP".into()),
                node_port: None,
                ..Default::default()
            },
            ServicePort {
                name: None,
                port: 443,
                target_port: Some(IntOrString::String("https".into())),
                protocol: None,
                node_port: Some(30443),
                ..Default::default()
            },
        ],
        "NodePort",
        &[("app", "nginx")],
    );

    let ports = service_ports(&svc);
    assert_eq!(ports.len(), 2);
    assert_eq!(ports[0].name, "http");
    assert_eq!(ports[0].target_port, "8080");
    assert_eq!(ports[0].protocol, "TCP");
    assert_eq!(ports[1].target_port, "https");
    // `protocol: None` defaults to TCP.
    assert_eq!(ports[1].protocol, "TCP");
    assert_eq!(ports[1].node_port, Some(30443));
}

#[test]
fn service_summary_extracts_type_and_selector() {
    let svc = service(vec![], "ClusterIP", &[("app", "api")]);
    let summary = service_summary(&svc);
    assert_eq!(summary.type_, "ClusterIP");
    assert_eq!(summary.selector.get("app").map(String::as_str), Some("api"));
    assert_eq!(summary.cluster_ip, "");
}

#[test]
fn ingress_rules_extract_host_path_backend() {
    let ing = Ingress {
        metadata: ObjectMeta::default(),
        spec: Some(IngressSpec {
            rules: Some(vec![IngressRule {
                host: Some("example.com".into()),
                http: Some(HTTPIngressRuleValue {
                    paths: vec![HTTPIngressPath {
                        path: Some("/api".into()),
                        path_type: "Prefix".into(),
                        backend: IngressBackend {
                            service: Some(IngressServiceBackend {
                                name: "api".into(),
                                port: None,
                            }),
                            ..Default::default()
                        },
                    }],
                }),
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };

    let rules = ingress_rules(&ing);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].host, "example.com");
    assert_eq!(rules[0].path, "/api");
    assert_eq!(rules[0].path_type, "Prefix");
    assert_eq!(rules[0].backend, "api");
}

#[test]
fn config_map_entries_are_sorted() {
    let mut data = BTreeMap::new();
    data.insert("z".to_string(), "last".to_string());
    data.insert("a".to_string(), "first".to_string());
    let cm = ConfigMap {
        metadata: ObjectMeta::default(),
        data: Some(data),
        ..Default::default()
    };
    let entries = config_map_entries(&cm);
    assert_eq!(
        entries,
        vec![
            ("a".to_string(), "first".to_string()),
            ("z".to_string(), "last".to_string())
        ]
    );
}

#[test]
fn secret_keys_union_data_and_string_data() {
    let mut data = BTreeMap::new();
    data.insert(
        "password".to_string(),
        k8s_openapi::ByteString(vec![1, 2, 3]),
    );
    let mut string_data = BTreeMap::new();
    string_data.insert("username".to_string(), "admin".to_string());
    let secret = Secret {
        metadata: ObjectMeta::default(),
        data: Some(data),
        string_data: Some(string_data),
        ..Default::default()
    };
    assert_eq!(secret_keys(&secret), vec!["password", "username"]);
}

// ─────────────────────────────────────────────────────────────
// Pure formatters (preview cells).
// ─────────────────────────────────────────────────────────────

#[test]
fn config_data_preview_empty_is_dash() {
    assert_eq!(config_data_preview(&[]), "—");
}

#[test]
fn config_data_preview_one_or_two_keys_no_suffix() {
    let one = vec![("a".to_string(), "1".to_string())];
    assert_eq!(config_data_preview(&one), "1 keys: a");

    let two = vec![
        ("a".to_string(), "1".to_string()),
        ("b".to_string(), "2".to_string()),
    ];
    assert_eq!(config_data_preview(&two), "2 keys: a, b");
}

#[test]
fn config_data_preview_many_keys_shows_remainder() {
    let entries = vec![
        ("a".to_string(), "1".to_string()),
        ("b".to_string(), "2".to_string()),
        ("c".to_string(), "3".to_string()),
        ("d".to_string(), "4".to_string()),
    ];
    assert_eq!(config_data_preview(&entries), "4 keys: a, b +2");
}

#[test]
fn format_ports_summary_empty_is_dash() {
    assert_eq!(format_ports_summary(&[]), "—");
}

#[test]
fn format_ports_summary_same_target_shows_port_only() {
    let rows = vec![ServicePortRow {
        name: "http".into(),
        port: 80,
        target_port: "80".into(),
        protocol: "TCP".into(),
        node_port: None,
    }];
    assert_eq!(format_ports_summary(&rows), "80");
}

#[test]
fn format_ports_summary_named_target_uses_arrow() {
    let rows = vec![ServicePortRow {
        name: "https".into(),
        port: 443,
        target_port: "https".into(),
        protocol: "TCP".into(),
        node_port: None,
    }];
    assert_eq!(format_ports_summary(&rows), "443→https");
}

#[test]
fn format_selector_short_empty_is_dash() {
    assert_eq!(format_selector_short(&BTreeMap::new()), "—");
}

#[test]
fn format_selector_short_flat_kv_and_truncate() {
    let mut one = BTreeMap::new();
    one.insert("app".to_string(), "api".to_string());
    assert_eq!(format_selector_short(&one), "app=api");

    let mut many = BTreeMap::new();
    many.insert("a".to_string(), "1".to_string());
    many.insert("b".to_string(), "2".to_string());
    many.insert("c".to_string(), "3".to_string());
    many.insert("d".to_string(), "4".to_string());
    let s = format_selector_short(&many);
    assert!(s.starts_with("a=1, b=2, c=3"), "got: {s}");
    assert!(s.ends_with("+1"), "got: {s}");
}

#[test]
fn format_ingress_paths_empty_is_dash() {
    assert_eq!(format_ingress_paths(&[]), "—");
}

#[test]
fn format_ingress_paths_joins_host_path() {
    let rules = vec![
        IngressRuleRow {
            host: "example.com".into(),
            path: "/api".into(),
            path_type: "Prefix".into(),
            backend: "api".into(),
        },
        IngressRuleRow {
            host: "other.com".into(),
            path: String::new(),
            path_type: "Prefix".into(),
            backend: "web".into(),
        },
    ];
    // Leading slash stripped; host-only rule renders bare host.
    assert_eq!(format_ingress_paths(&rules), "example.com/api, other.com");
}

#[test]
fn format_ingress_hosts_dedupes_and_sorts() {
    let rules = vec![
        IngressRuleRow {
            host: "b.com".into(),
            path: "/1".into(),
            path_type: "Prefix".into(),
            backend: "x".into(),
        },
        IngressRuleRow {
            host: "a.com".into(),
            path: "/2".into(),
            path_type: "Prefix".into(),
            backend: "y".into(),
        },
        IngressRuleRow {
            host: "b.com".into(),
            path: "/3".into(),
            path_type: "Prefix".into(),
            backend: "z".into(),
        },
    ];
    assert_eq!(format_ingress_hosts(&rules), "a.com, b.com");
}

#[test]
fn secret_key_count_union() {
    let mut data = BTreeMap::new();
    data.insert("a".to_string(), k8s_openapi::ByteString(vec![1]));
    let mut string_data = BTreeMap::new();
    string_data.insert("b".to_string(), "v".to_string());
    string_data.insert("a".to_string(), "dup".to_string());
    let secret = Secret {
        metadata: ObjectMeta::default(),
        data: Some(data),
        string_data: Some(string_data),
        ..Default::default()
    };
    assert_eq!(secret_key_count(&secret), 2);
}

// ─────────────────────────────────────────────────────────────
// Row builders (cell layout pin).
// ─────────────────────────────────────────────────────────────

#[test]
fn config_map_row_layout_and_preview() {
    let mut data = BTreeMap::new();
    data.insert("key".to_string(), "val".to_string());
    let cm = ConfigMap {
        metadata: ObjectMeta {
            name: Some("app-config".into()),
            namespace: Some("default".into()),
            ..Default::default()
        },
        data: Some(data),
        ..Default::default()
    };
    let row = config_map_row(&cm);
    assert_eq!(row.id, "default/app-config");
    assert_eq!(row.cells[0].text, "app-config");
    assert_eq!(row.cells[1].text, "1 keys: key");
    assert_eq!(row.cells[2].text, "1");
    assert_eq!(row.cells[4].text, "ConfigMap");
}

#[test]
fn secret_row_masked_type_and_key_count() {
    let mut data = BTreeMap::new();
    data.insert("password".to_string(), k8s_openapi::ByteString(vec![1]));
    let secret = Secret {
        metadata: ObjectMeta {
            name: Some("app-secret".into()),
            namespace: Some("prod".into()),
            ..Default::default()
        },
        type_: Some("Opaque".into()),
        data: Some(data),
        ..Default::default()
    };
    let row = secret_row(&secret);
    assert_eq!(row.id, "prod/app-secret");
    assert_eq!(row.cells[0].text, "app-secret");
    assert_eq!(row.cells[1].text, "Opaque");
    // Key count only — the value is never surfaced.
    assert_eq!(row.cells[2].text, "1");
    assert_eq!(row.cells[4].text, "Secret");
}

#[test]
fn service_row_layout_uses_formatters() {
    let svc = service(
        vec![ServicePort {
            name: Some("http".into()),
            port: 80,
            target_port: Some(IntOrString::Int(8080)),
            protocol: None,
            node_port: None,
            ..Default::default()
        }],
        "LoadBalancer",
        &[("app", "nginx")],
    );
    let row = service_row(&svc);
    assert_eq!(row.cells[0].text, "");
    assert_eq!(row.cells[1].text, "LoadBalancer");
    assert_eq!(row.cells[2].text, "—");
    assert_eq!(row.cells[3].text, "80→8080");
    assert_eq!(row.cells[4].text, "app=nginx");
    assert_eq!(row.cells[6].text, "Service");
}

#[test]
fn ingress_row_layout_uses_host_path_formatters() {
    let ing = Ingress {
        metadata: ObjectMeta {
            name: Some("edge".into()),
            namespace: Some("default".into()),
            ..Default::default()
        },
        spec: Some(IngressSpec {
            ingress_class_name: Some("nginx".into()),
            rules: Some(vec![IngressRule {
                host: Some("example.com".into()),
                http: Some(HTTPIngressRuleValue {
                    paths: vec![HTTPIngressPath {
                        path: Some("/api".into()),
                        path_type: "Prefix".into(),
                        backend: IngressBackend {
                            service: Some(IngressServiceBackend {
                                name: "api".into(),
                                port: None,
                            }),
                            ..Default::default()
                        },
                    }],
                }),
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let row = ingress_row(&ing);
    assert_eq!(row.id, "default/edge");
    assert_eq!(row.cells[1].text, "nginx");
    assert_eq!(row.cells[2].text, "example.com");
    assert_eq!(row.cells[3].text, "example.com/api");
    assert_eq!(row.cells[5].text, "Ingress");
}
