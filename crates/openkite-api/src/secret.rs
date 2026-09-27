//! The owned secret surface: what the slide-over masks, without a kube type.
//!
//! The helpers here are pure and carry the two rules the slide-over depends
//! on: values are masked unless explicitly revealed, and the key set is the
//! sorted union of `data` and `string_data`.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// An owned mirror of the parts of a Kubernetes Secret the console reads.
/// The host fills it from `k8s_openapi::api::core::v1::Secret`, decoding
/// `ByteString` to `Vec<u8>`; nothing kube-shaped crosses into the UI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretObject {
    /// `metadata.name`, or `""` when the payload carried none.
    pub name: String,
    /// `metadata.namespace`.
    pub namespace: Option<String>,
    /// `type`, unlabelled by the host (the console maps it for display).
    pub type_: Option<String>,
    /// `data`: base64-decoded by the host, keyed by entry name.
    pub data: BTreeMap<String, Vec<u8>>,
    /// `string_data`: plaintext entries.
    pub string_data: BTreeMap<String, String>,
}

/// The keys present in a Secret (from `data` and `string_data`), sorted.
pub fn secret_keys(secret: &SecretObject) -> Vec<String> {
    let mut keys = BTreeSet::new();
    keys.extend(secret.data.keys().cloned());
    keys.extend(secret.string_data.keys().cloned());
    keys.into_iter().collect()
}

/// Look up `key` in `data` first (decoded `Vec<u8>`), then `string_data`.
/// Returns the UTF-8 lossy representation. This is the only function in the
/// project that produces a plaintext Secret value; it is called inside the
/// slide-over to construct a `MaskedSecret::new(plaintext)`, and the plaintext
/// is never held outside a `MaskedSecret` for longer than the `display()` call.
pub fn decoded_value_for_key(secret: &SecretObject, key: &str) -> String {
    if let Some(bytes) = secret.data.get(key) {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    if let Some(s) = secret.string_data.get(key) {
        return s.clone();
    }
    String::new()
}

/// The `ns/name` (or bare `name`) row id for a secret — the same shape
/// `workloads::object_id` produces, kept here so the mapper and the slide-over
/// agree without depending on the private `workloads` helper.
pub fn row_id_for_secret(secret: &SecretObject) -> String {
    match secret.namespace.as_deref() {
        Some(ns) => format!("{ns}/{}", secret.name),
        None => secret.name.clone(),
    }
}

/// The typed-name gate for bulk reveal: `true` iff `typed.trim() == name`
/// (case-sensitive, no fuzzy).
pub fn bulk_reveal_predicate(typed: &str, name: &str) -> bool {
    typed.trim() == name
}

/// Map a `Secret.type` string to a human label. `None` / `Some("Opaque")` /
/// empty string all collapse to `"Opaque"` (the kube default); unknown kinds
/// pass through so a CRD-defined type still surfaces in the table.
pub fn secret_kind_label(kind: Option<&str>) -> String {
    match kind.unwrap_or("Opaque") {
        "kubernetes.io/tls" => "TLS".into(),
        "kubernetes.io/dockerconfigjson" => "Docker config".into(),
        "kubernetes.io/service-account-token" => "Service account token".into(),
        "Opaque" | "" => "Opaque".into(),
        other => other.to_string(),
    }
}
