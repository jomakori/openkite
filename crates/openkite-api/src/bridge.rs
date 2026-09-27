//! The bridge wire contract: the tagged envelope a plugin or console POSTs to
//! `/openkite`, and the kube operations it may ask for.
//!
//! Both hosts speak this: the desktop reaches it through the wry asset handler,
//! the browser host mounts it on axum. The dispatch itself needs a cluster
//! client, so it lives with the host runtime (`openkite_host::bridge`).

use serde::{Deserialize, Serialize};

/// A kube operation a plugin requests through `openkite.api.*`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ApiRequest {
    /// Register a UI contribution (`kind`: `sidebar` | `route` | `status`).
    Register {
        kind: String,
        payload: serde_json::Value,
    },
    /// List a resource kind, optionally namespaced (`ns: null` = all).
    List { kind: String, ns: Option<String> },
    /// Fetch one resource.
    Get {
        kind: String,
        ns: String,
        name: String,
    },
    /// Open a watch stream on a kind.
    Watch { kind: String, ns: Option<String> },
    /// Tail/follow container logs.
    Logs {
        name: String,
        ns: String,
        #[serde(default)]
        container: Option<String>,
    },
    /// Spawn a command in a container (PTY later).
    Exec {
        name: String,
        ns: String,
        #[serde(default)]
        container: Option<String>,
        cmd: Vec<String>,
    },
    /// Subscribe to live updates for a kind (additive push channel, OKT-91).
    /// Answers `{sub: <id>, initial: <snapshot>}`; ongoing updates arrive out
    /// of band via `window.openkite._pushState`.
    Subscribe { kind: String, ns: Option<String> },
    /// Cancel a subscription by id. Answers `{unsubscribed: <bool>}`.
    Unsubscribe { sub: u64 },
}

impl ApiRequest {
    /// A short human-readable label for logs/debugging (e.g. `list pods`).
    pub fn describe(&self) -> String {
        match self {
            ApiRequest::Register { kind, .. } => format!("register {kind}"),
            ApiRequest::List { kind, .. } => format!("list {kind}"),
            ApiRequest::Get { kind, name, .. } => format!("get {kind}/{name}"),
            ApiRequest::Watch { kind, .. } => format!("watch {kind}"),
            ApiRequest::Logs { name, .. } => format!("logs {name}"),
            ApiRequest::Exec { name, cmd, .. } => format!("exec {name} {}", cmd.join(" ")),
            ApiRequest::Subscribe { kind, .. } => format!("subscribe {kind}"),
            ApiRequest::Unsubscribe { sub } => format!("unsubscribe {sub}"),
        }
    }
}

/// A bridge response: tagged ok/error so the wire format is unambiguous.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ApiResponse {
    /// Structured payload (JSON of the resource/list).
    Ok { result: serde_json::Value },
    /// Human-readable failure.
    Error { error: String },
}

/// Envelope exchanged over the bridge channel: `{channel: "openkite", id,
/// plugin, request}`. `plugin` is stamped by the host from
/// `window.__openkite_plugin` (set before each bundle evaluates).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeRequest {
    pub id: u64,
    pub plugin: String,
    pub request: ApiRequest,
}
