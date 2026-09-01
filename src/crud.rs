//! Resource CRUD helpers: manifest validation, mutation targets, apply
//! dispatch. Phase 1 wires `apply_mutation` to kube `Api::{create, patch,
//! delete, scale}` calls; this module ships the typed `Mutation` enum and a
//! placeholder `apply_mutation` that returns a Phase-1-pending error so the
//! UI flows end-to-end today.

use kube::Client;
use serde::{Deserialize, Serialize};
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

/// One propagation policy today: `Default` (kubectl foreground). Future
/// variants (`Orphan`, `Foreground`, `Background`) are additive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PropagationPolicy {
    Default,
}

impl Default for PropagationPolicy {
    fn default() -> Self {
        Self::Default
    }
}

impl PropagationPolicy {
    /// A short label for the confirm modal eyebrow.
    pub fn label(self) -> &'static str {
        match self {
            PropagationPolicy::Default => "Default (foreground)",
        }
    }
}

/// The four CRUD operations the UI dispatches.
#[derive(Debug, Clone, PartialEq)]
pub enum Mutation {
    /// Apply a new manifest to the cluster.
    Create(Value),
    /// Patch an existing manifest. The payload must carry
    /// `metadata.resourceVersion` so the server can detect lost-update.
    Edit(Value),
    /// Delete a single resource by triple. The propagation policy is fixed
    /// at `Default` today; a follow-up adds the select.
    Delete {
        kind: String,
        namespace: Option<String>,
        name: String,
        propagation: PropagationPolicy,
    },
    /// Scale a workload to a new replica count via SSA. The kind is one of
    /// Deployment / StatefulSet / ReplicaSet.
    Scale {
        kind: String,
        namespace: Option<String>,
        name: String,
        replicas: u32,
    },
}

impl Mutation {
    /// Short verb for logs and toast copy (`create` / `edit` / `delete` /
    /// `scale`).
    pub fn verb(&self) -> &'static str {
        match self {
            Mutation::Create(_) => "create",
            Mutation::Edit(_) => "edit",
            Mutation::Delete { .. } => "delete",
            Mutation::Scale { .. } => "scale",
        }
    }
}

/// One-line triple the confirm modal renders. Pure formatter.
pub fn target_summary(m: &Mutation) -> ManifestRef {
    match m {
        Mutation::Create(doc) | Mutation::Edit(doc) => ManifestRef {
            api_version: doc
                .get("apiVersion")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            kind: doc
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            name: doc
                .pointer("/metadata/name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            namespace: doc
                .pointer("/metadata/namespace")
                .and_then(Value::as_str)
                .map(str::to_string),
        },
        Mutation::Delete {
            kind,
            namespace,
            name,
            ..
        }
        | Mutation::Scale {
            kind,
            namespace,
            name,
            ..
        } => ManifestRef {
            api_version: String::new(),
            kind: kind.clone(),
            name: name.clone(),
            namespace: namespace.clone(),
        },
    }
}

/// Edit-time guard: a patch payload must carry `metadata.resourceVersion` so
/// the server can detect lost-update. Runs after [`validate_manifest`].
pub fn validate_for_edit(doc: &Value) -> Result<(), String> {
    let _ = validate_manifest(doc)?;
    let rv = doc
        .pointer("/metadata/resourceVersion")
        .and_then(Value::as_str)
        .ok_or("missing metadata.resourceVersion (edit requires the live RV)")?;
    if rv.is_empty() {
        return Err("metadata.resourceVersion must not be empty".into());
    }
    Ok(())
}

/// Dispatch a mutation. Today: returns the Phase-1-pending error so the UI
/// flows end-to-end. Future Phase 1: kube `Api::create` / `Api::patch` /
/// `Api::delete` (with propagation) / `Api::patch` for scale (SSA).
pub async fn apply_mutation(_client: &Client, m: &Mutation) -> Result<(), String> {
    Err(format!("{}: cluster mutation lands in Phase 1", m.verb()))
}

/// The "you typed this to confirm" gate for the destructive confirm modal.
/// Returns `true` only when the typed string exactly equals the resource
/// name (case-sensitive, no fuzzy, no trim). Pure function — fully testable.
pub fn typed_name_matches(typed: &str, name: &str) -> bool {
    typed == name
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pod_doc() -> Value {
        json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "nginx-7c5d", "namespace": "default" }
        })
    }

    #[test]
    fn target_summary_for_create_extracts_triple() {
        let m = Mutation::Create(pod_doc());
        let r = target_summary(&m);
        assert_eq!(r.kind, "Pod");
        assert_eq!(r.name, "nginx-7c5d");
        assert_eq!(r.namespace.as_deref(), Some("default"));
    }

    #[test]
    fn target_summary_for_delete_uses_explicit_fields() {
        let m = Mutation::Delete {
            kind: "Pod".into(),
            namespace: Some("default".into()),
            name: "nginx-7c5d".into(),
            propagation: PropagationPolicy::Default,
        };
        let r = target_summary(&m);
        assert_eq!(r.kind, "Pod");
        assert_eq!(r.name, "nginx-7c5d");
        assert_eq!(r.namespace.as_deref(), Some("default"));
        assert_eq!(r.api_version, "");
    }

    #[test]
    fn validate_for_edit_accepts_doc_with_resource_version() {
        let doc = json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": { "name": "web", "resourceVersion": "12345" }
        });
        assert!(validate_for_edit(&doc).is_ok());
    }

    #[test]
    fn validate_for_edit_rejects_missing_resource_version() {
        let doc = json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": { "name": "web" }
        });
        assert_eq!(
            validate_for_edit(&doc).unwrap_err(),
            "missing metadata.resourceVersion (edit requires the live RV)"
        );
    }

    #[test]
    fn validate_for_edit_rejects_empty_resource_version() {
        let doc = json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": { "name": "web", "resourceVersion": "" }
        });
        assert_eq!(
            validate_for_edit(&doc).unwrap_err(),
            "metadata.resourceVersion must not be empty"
        );
    }

    #[test]
    fn propagation_policy_default_serializes_as_default() {
        let json = serde_json::to_string(&PropagationPolicy::Default).unwrap();
        assert_eq!(json, "\"default\"");
        let back: PropagationPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(back, PropagationPolicy::Default);
        assert_eq!(PropagationPolicy::default(), PropagationPolicy::Default);
        assert_eq!(PropagationPolicy::Default.label(), "Default (foreground)");
    }

    #[tokio::test]
    async fn apply_mutation_returns_phase1_placeholder_error_today() {
        // The Phase-1-pending contract: every variant surfaces a string
        // starting with the verb and the "cluster mutation lands in Phase 1"
        // suffix. When the Phase 1 follow-up lands, the test updates
        // in lockstep with the real implementation.
        let client = Client::try_default().await.ok();
        // Take the client once before the loop; the placeholder never
        // sends a request, so an absent cluster still exercises the
        // Phase-1 contract path for every variant.
        let client = client
            .as_ref()
            .expect("client (placeholder needs no cluster)");
        let doc = pod_doc();
        for m in [
            Mutation::Create(doc.clone()),
            Mutation::Edit(doc.clone()),
            Mutation::Delete {
                kind: "Pod".into(),
                namespace: Some("default".into()),
                name: "nginx-7c5d".into(),
                propagation: PropagationPolicy::Default,
            },
            Mutation::Scale {
                kind: "Deployment".into(),
                namespace: Some("default".into()),
                name: "web".into(),
                replicas: 3,
            },
        ] {
            let err = apply_mutation(client, &m).await.unwrap_err();
            let expected_verb = m.verb();
            assert!(
                err.starts_with(expected_verb) && err.contains("Phase 1"),
                "expected `{expected_verb}…Phase 1`, got {err:?}"
            );
        }
    }

    #[test]
    fn typed_name_matches_is_case_sensitive_and_exact() {
        assert!(typed_name_matches("nginx-7c5d", "nginx-7c5d"));
        assert!(!typed_name_matches("nginx", "nginx-7c5d"));
        assert!(!typed_name_matches("Nginx-7c5d", "nginx-7c5d"));
        assert!(!typed_name_matches("nginx-7c5d ", "nginx-7c5d"));
        assert!(!typed_name_matches("", "nginx-7c5d"));
    }
}
