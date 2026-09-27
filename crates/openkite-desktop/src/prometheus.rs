//! Prometheus detection for expanded metrics.

use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::Service;

/// Well-known Prometheus service names, tried when the label is absent.
pub const KNOWN_NAMES: &[&str] = &[
    "prometheus-kube-prometheus-prometheus",
    "prometheus-operated",
    "prometheus",
];

/// Whether a service looks like a Prometheus instance.
///
/// Matches the `app.kubernetes.io/name=prometheus` label, or a well-known name.
pub fn is_prometheus_service(name: &str, labels: Option<&BTreeMap<String, String>>) -> bool {
    if let Some(labels) = labels {
        if labels.get("app.kubernetes.io/name").map(String::as_str) == Some("prometheus") {
            return true;
        }
    }
    KNOWN_NAMES.contains(&name)
}

/// The first service that looks like Prometheus, if any.
///
/// Preserves slice order so earlier services win (mirrors the "first ready
/// endpoint" probe).
pub fn detect_prometheus(services: &[Service]) -> Option<String> {
    services.iter().find_map(|svc| {
        let name = svc.metadata.name.as_deref().unwrap_or_default();
        is_prometheus_service(name, svc.metadata.labels.as_ref()).then(|| name.to_string())
    })
}
