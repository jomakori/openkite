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
    config_map_entries, ingress_rules, secret_keys, service_ports, service_summary,
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
