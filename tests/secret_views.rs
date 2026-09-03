//! Integration tests for the secret detail slide-over's pure helpers.
//!
//! No Dioxus runtime, no kube client — these pin the decode/reveal logic the
//! `#[component]` bodies consume. The k8s-openapi `Secret` fixtures exercise
//! both `data` (base64 bytes) and `string_data` (plain strings).

use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::ByteString;
use openkite::components::secret_detail::{
    decoded_value_for_key, row_id_for_secret, secret_kind_label,
};

fn secret_with_data(data: &[(&str, &str)]) -> Secret {
    let mut secret = Secret::default();
    let map = data
        .iter()
        .map(|(k, v)| (k.to_string(), ByteString(v.as_bytes().to_vec())))
        .collect();
    secret.data = Some(map);
    secret
}

#[test]
fn decoded_value_for_key_returns_data_bytes_as_utf8() {
    let secret = secret_with_data(&[("k", "hello")]);
    assert_eq!(decoded_value_for_key(&secret, "k"), "hello");
}

#[test]
fn decoded_value_for_key_falls_through_to_string_data() {
    let mut secret = Secret::default();
    let mut map = std::collections::BTreeMap::new();
    map.insert("k".to_string(), "world".to_string());
    secret.string_data = Some(map);
    assert_eq!(decoded_value_for_key(&secret, "k"), "world");
}

#[test]
fn decoded_value_for_key_returns_empty_for_missing_key() {
    let secret = Secret::default();
    assert_eq!(decoded_value_for_key(&secret, "missing"), "");
}

#[test]
fn row_id_for_secret_includes_namespace_when_present() {
    let mut secret = Secret::default();
    secret.metadata.namespace = Some("default".into());
    secret.metadata.name = Some("foo".into());
    assert_eq!(row_id_for_secret(&secret), "default/foo");
}

#[test]
fn row_id_for_secret_omits_namespace_when_absent() {
    let mut secret = Secret::default();
    secret.metadata.name = Some("foo".into());
    assert_eq!(row_id_for_secret(&secret), "foo");
}

#[test]
fn secret_kind_label_recognises_known_kinds() {
    assert_eq!(secret_kind_label(Some("kubernetes.io/tls")), "TLS");
    assert_eq!(secret_kind_label(Some("Opaque")), "Opaque");
    assert_eq!(secret_kind_label(None), "Opaque");
    assert_eq!(secret_kind_label(Some("custom")), "custom");
}
