//! Integration tests for manifest validation.

use serde_json::json;

use openkite::crud::{
    apply_mutation, target_summary, typed_name_matches, validate_for_edit, validate_manifest,
    Mutation, PropagationPolicy,
};

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
fn target_summary_formats_kind_namespace_name() {
    let m = Mutation::Delete {
        kind: "Pod".into(),
        namespace: Some("default".into()),
        name: "nginx-7c5d".into(),
        propagation: PropagationPolicy::Default,
    };
    let r = target_summary(&m);
    assert_eq!(r.kind, "Pod");
    assert_eq!(r.namespace.as_deref(), Some("default"));
    assert_eq!(r.name, "nginx-7c5d");
}

#[test]
fn validate_for_edit_rejects_missing_resource_version() {
    // A patch payload without metadata.resourceVersion cannot detect
    // lost-update — the edit gate must reject it.
    let doc = json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": "nginx-7c5d", "namespace": "default" }
    });
    assert!(validate_for_edit(&doc).is_err());

    let with_rv = json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": "nginx-7c5d",
            "namespace": "default",
            "resourceVersion": "12345",
        }
    });
    assert!(validate_for_edit(&with_rv).is_ok());
}

#[test]
fn propagation_policy_default_serializes_as_default() {
    // The confirm modal sends PropagationPolicy::Default; the JSON
    // wire form must be the literal string "default" (serde
    // rename_all = "lowercase").
    let p = PropagationPolicy::Default;
    assert_eq!(serde_json::to_string(&p).unwrap(), "\"default\"");
}

#[test]
fn apply_mutation_returns_phase1_placeholder_error_today() {
    // Pins the deferred-work contract: until the bridge mutation ops
    // land, every variant returns the Phase-1-pending error. When a
    // Phase 1 follow-up replaces these stubs, this test updates in
    // lockstep (that's the point of the pin).
    let doc = json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": { "name": "nginx", "namespace": "default" }
    });
    let cases = [
        Mutation::Create(doc.clone()),
        Mutation::Edit(doc.clone()),
        Mutation::Delete {
            kind: "Pod".into(),
            namespace: Some("default".into()),
            name: "nginx".into(),
            propagation: PropagationPolicy::Default,
        },
        Mutation::Scale {
            kind: "Deployment".into(),
            namespace: Some("default".into()),
            name: "web".into(),
            replicas: 3,
        },
    ];
    // The placeholder signature takes &Client but never touches it;
    // drive it with a client built from a fake http URI (no cluster
    // behind it — the placeholder never sends a request).
    let config = kube::Config::new("http://127.0.0.1:8080".parse().expect("valid uri"));
    let dummy = kube::Client::try_from(config).expect("client builds from config");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread tokio runtime");
    for m in cases {
        let err = rt.block_on(apply_mutation(&dummy, &m)).unwrap_err();
        assert!(
            err.contains("Phase 1"),
            "expected Phase-1-pending error, got: {err}"
        );
    }
}

#[test]
fn typed_name_matches_is_exact_case_sensitive() {
    assert!(typed_name_matches("nginx-7c5d", "nginx-7c5d"));
    assert!(!typed_name_matches("nginx-7c5d ", "nginx-7c5d"));
    assert!(!typed_name_matches("NGINX-7c5d", "nginx-7c5d"));
    assert!(!typed_name_matches("", "nginx-7c5d"));
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
