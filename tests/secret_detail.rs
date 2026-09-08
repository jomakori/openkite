//! Integration tests for the secret-detail helpers: value decoding, row
//! id shape, bulk-reveal gate, and kind labels.

use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

use openkite::components::secret_detail::{
    bulk_reveal_predicate, decoded_value_for_key, row_id_for_secret, secret_kind_label,
};

fn secret(
    namespace: Option<&str>,
    name: &str,
    data: BTreeMap<String, k8s_openapi::ByteString>,
) -> Secret {
    Secret {
        metadata: ObjectMeta {
            name: Some(name.into()),
            namespace: namespace.map(str::to_string),
            ..Default::default()
        },
        data: Some(data),
        ..Default::default()
    }
}

#[test]
fn decoded_value_for_key_reads_binary_data() {
    let mut data = BTreeMap::new();
    data.insert(
        "password".to_string(),
        k8s_openapi::ByteString(b"hunter2".to_vec()),
    );
    let s = secret(Some("default"), "app", data);
    assert_eq!(decoded_value_for_key(&s, "password"), "hunter2");
}

#[test]
fn decoded_value_for_key_reads_string_data() {
    let s = Secret {
        metadata: ObjectMeta {
            name: Some("app".into()),
            ..Default::default()
        },
        string_data: Some(BTreeMap::from([("user".to_string(), "admin".to_string())])),
        ..Default::default()
    };
    assert_eq!(decoded_value_for_key(&s, "user"), "admin");
}

#[test]
fn decoded_value_for_key_missing_is_empty() {
    let s = secret(None, "app", BTreeMap::new());
    assert_eq!(decoded_value_for_key(&s, "nope"), "");
}

#[test]
fn decoded_value_for_key_binary_data_takes_precedence() {
    let mut data = BTreeMap::new();
    data.insert(
        "key".to_string(),
        k8s_openapi::ByteString(b"binary".to_vec()),
    );
    let s = Secret {
        metadata: ObjectMeta::default(),
        data: Some(data),
        string_data: Some(BTreeMap::from([("key".to_string(), "text".to_string())])),
        ..Default::default()
    };
    assert_eq!(decoded_value_for_key(&s, "key"), "binary");
}

#[test]
fn row_id_for_secret_includes_namespace_when_present() {
    assert_eq!(
        row_id_for_secret(&secret(Some("prod"), "app", BTreeMap::new())),
        "prod/app"
    );
    assert_eq!(
        row_id_for_secret(&secret(None, "node-1", BTreeMap::new())),
        "node-1"
    );
}

#[test]
fn bulk_reveal_predicate_exact_trimmed_match() {
    assert!(bulk_reveal_predicate("my-secret", "my-secret"));
    assert!(bulk_reveal_predicate("  my-secret  ", "my-secret"));
    assert!(!bulk_reveal_predicate("my-secret", "my-secret-2"));
    // Case-sensitive — a mistyped case does not reveal.
    assert!(!bulk_reveal_predicate("MY-SECRET", "my-secret"));
    assert!(!bulk_reveal_predicate("", "my-secret"));
}

#[test]
fn secret_kind_label_maps_well_known_kinds() {
    assert_eq!(secret_kind_label(None), "Opaque");
    assert_eq!(secret_kind_label(Some("")), "Opaque");
    assert_eq!(secret_kind_label(Some("Opaque")), "Opaque");
    assert_eq!(secret_kind_label(Some("kubernetes.io/tls")), "TLS");
    assert_eq!(
        secret_kind_label(Some("kubernetes.io/dockerconfigjson")),
        "Docker config"
    );
    assert_eq!(
        secret_kind_label(Some("kubernetes.io/service-account-token")),
        "Service account token"
    );
}

#[test]
fn secret_kind_label_passes_through_unknown_kinds() {
    assert_eq!(
        secret_kind_label(Some("example.com/custom")),
        "example.com/custom"
    );
}
