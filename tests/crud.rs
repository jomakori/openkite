//! Integration tests for manifest validation.

use serde_json::json;

use openkite::crud::validate_manifest;

#[test]
fn validates_a_complete_manifest() {
    let doc = json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": { "name": "web", "namespace": "default" }
    });
    let r = validate_manifest(&doc).unwrap();
    assert_eq!(r.api_version, "apps/v1");
    assert_eq!(r.kind, "Deployment");
    assert_eq!(r.name, "web");
    assert_eq!(r.namespace.as_deref(), Some("default"));
}

#[test]
fn namespace_is_optional() {
    let doc = json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": "app" }
    });
    let r = validate_manifest(&doc).unwrap();
    assert_eq!(r.namespace, None);
}

#[test]
fn missing_api_version_rejected() {
    let doc = json!({ "kind": "Pod", "metadata": { "name": "app" } });
    assert_eq!(validate_manifest(&doc).unwrap_err(), "missing apiVersion");
}

#[test]
fn missing_metadata_rejected() {
    let doc = json!({ "apiVersion": "v1", "kind": "Pod" });
    assert_eq!(validate_manifest(&doc).unwrap_err(), "missing metadata");
}

#[test]
fn missing_name_rejected() {
    let doc = json!({ "apiVersion": "v1", "kind": "Pod", "metadata": { "namespace": "default" } });
    assert_eq!(
        validate_manifest(&doc).unwrap_err(),
        "missing metadata.name"
    );
}
