//! Integration tests for the secret detail slide-over's pure helpers.
//!
//! No Dioxus runtime, no cluster client — these pin the decode/reveal logic the
//! `#[component]` bodies consume. The [`SecretObject`] fixtures exercise both
//! `data` (bytes) and `string_data` (plain strings).

use openkite_api::secret::SecretObject;
use openkite_ui::components::secret_detail::{
    decoded_value_for_key, row_id_for_secret, secret_kind_label,
};

fn secret_with_data(data: &[(&str, &str)]) -> SecretObject {
    let mut secret = SecretObject::default();
    let map = data
        .iter()
        .map(|(k, v)| (k.to_string(), v.as_bytes().to_vec()))
        .collect();
    secret.data = map;
    secret
}

#[test]
fn decoded_value_for_key_returns_data_bytes_as_utf8() {
    let secret = secret_with_data(&[("k", "hello")]);
    assert_eq!(decoded_value_for_key(&secret, "k"), "hello");
}

#[test]
fn decoded_value_for_key_falls_through_to_string_data() {
    let mut secret = SecretObject::default();
    let mut map = std::collections::BTreeMap::new();
    map.insert("k".to_string(), "world".to_string());
    secret.string_data = map;
    assert_eq!(decoded_value_for_key(&secret, "k"), "world");
}

#[test]
fn decoded_value_for_key_returns_empty_for_missing_key() {
    let secret = SecretObject::default();
    assert_eq!(decoded_value_for_key(&secret, "missing"), "");
}

#[test]
fn row_id_for_secret_includes_namespace_when_present() {
    let secret = SecretObject {
        name: "foo".into(),
        namespace: Some("default".into()),
        ..Default::default()
    };
    assert_eq!(row_id_for_secret(&secret), "default/foo");
}

#[test]
fn row_id_for_secret_omits_namespace_when_absent() {
    let secret = SecretObject {
        name: "foo".into(),
        ..Default::default()
    };
    assert_eq!(row_id_for_secret(&secret), "foo");
}

#[test]
fn secret_kind_label_recognises_known_kinds() {
    assert_eq!(secret_kind_label(Some("kubernetes.io/tls")), "TLS");
    assert_eq!(secret_kind_label(Some("Opaque")), "Opaque");
    assert_eq!(secret_kind_label(None), "Opaque");
    assert_eq!(secret_kind_label(Some("custom")), "custom");
}

#[test]
fn secret_kind_label_covers_remaining_known_kinds() {
    assert_eq!(
        secret_kind_label(Some("kubernetes.io/dockerconfigjson")),
        "Docker config"
    );
    assert_eq!(
        secret_kind_label(Some("kubernetes.io/service-account-token")),
        "Service account token"
    );
    assert_eq!(secret_kind_label(Some("")), "Opaque");
}
