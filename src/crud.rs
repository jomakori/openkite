//! Resource CRUD helpers: manifest validation before apply.

use serde_json::Value;

/// The minimal identity of a Kubernetes manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestRef {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
}

/// Validate a parsed manifest has the fields required to apply it.
///
/// Rejects manifests missing `apiVersion`, `kind`, or `metadata.name` with a
/// human-readable message — the first gate in the create/edit flow.
pub fn validate_manifest(doc: &Value) -> Result<ManifestRef, String> {
    let api_version = doc
        .get("apiVersion")
        .and_then(Value::as_str)
        .ok_or("missing apiVersion")?;
    let kind = doc
        .get("kind")
        .and_then(Value::as_str)
        .ok_or("missing kind")?;
    let metadata = doc.get("metadata").ok_or("missing metadata")?;
    let name = metadata
        .get("name")
        .and_then(Value::as_str)
        .ok_or("missing metadata.name")?;

    let namespace = metadata
        .get("namespace")
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(ManifestRef {
        api_version: api_version.to_string(),
        kind: kind.to_string(),
        name: name.to_string(),
        namespace,
    })
}
