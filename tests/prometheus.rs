//! Integration tests for Prometheus detection.

use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::Service;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

use openkite::prometheus::{detect_prometheus, is_prometheus_service};

fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn svc(name: &str, pairs: &[(&str, &str)]) -> Service {
    Service {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            labels: Some(labels(pairs)),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn label_matches_prometheus() {
    let l = labels(&[("app.kubernetes.io/name", "prometheus")]);
    assert!(is_prometheus_service("anything", Some(&l)));
}

#[test]
fn known_name_matches_without_label() {
    assert!(is_prometheus_service("prometheus-operated", None));
    assert!(is_prometheus_service(
        "prometheus-kube-prometheus-prometheus",
        None
    ));
}

#[test]
fn unrelated_service_does_not_match() {
    assert!(!is_prometheus_service("nginx", None));
    let l = labels(&[("app.kubernetes.io/name", "nginx")]);
    assert!(!is_prometheus_service("nginx", Some(&l)));
}

#[test]
fn detect_finds_first_prometheus_in_order() {
    let services = vec![
        svc("nginx", &[]),
        svc("prometheus-operated", &[]),
        svc("prometheus-kube-prometheus-prometheus", &[]),
    ];
    assert_eq!(
        detect_prometheus(&services).as_deref(),
        Some("prometheus-operated")
    );
}

#[test]
fn detect_returns_none_when_absent() {
    let services = vec![svc("nginx", &[]), svc("api", &[])];
    assert!(detect_prometheus(&services).is_none());
}
