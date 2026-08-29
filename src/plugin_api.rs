//! Plugin API bridge (OKT-46): the JS surface plugins call.
//!
//! External plugins are JS bundles evaluated in the wry webview. This module
//! owns the testable half of the bridge:
//!
//! - [`PluginRegistration`] — the typed model of what a plugin registers
//!   (sidebar items, routes, status items), stored per-plugin in
//!   [`RegistrationStore`] and rendered by core views.
//! - [`ApiRequest`] / [`ApiResponse`] — the kube operations a plugin can
//!   request through `openkite.api.*`, serialized as a tagged envelope
//!   ([`BridgeRequest`]).
//! - [`OPENKITE_BRIDGE_JS`] — the injected bootstrap that defines the
//!   `window.openkite` global (register* + api.* with promise plumbing over
//!   same-origin `fetch` POSTs to the `/openkite` asset handler).
//!
//! The runtime wiring lives in [`crate::bridge`] ([`crate::bridge::Bridge::handle_post`]
//! → registration store / kube dispatch); mounting the handler into the app
//! shell is the interactive remainder (OKT-31).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A sidebar entry a plugin contributes (shown under a plugin section).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarItem {
    pub label: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub route: String,
}

/// A routed plugin view (`route` must start with `/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteSpec {
    pub path: String,
    #[serde(default)]
    pub title: String,
}

/// A status-bar widget a plugin registers (label + optional dot color).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusItem {
    pub label: String,
    #[serde(default)]
    pub color: String,
}

/// Everything a plugin registered, keyed by kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRegistration {
    #[serde(default)]
    pub sidebar: Vec<SidebarItem>,
    #[serde(default)]
    pub routes: Vec<RouteSpec>,
    #[serde(default)]
    pub status: Vec<StatusItem>,
}

impl PluginRegistration {
    /// Validate every entry: non-empty labels/titles, route paths start with
    /// `/` and contain no spaces.
    pub fn validate(&self) -> Result<(), String> {
        for item in &self.sidebar {
            if item.label.trim().is_empty() {
                return Err("sidebar item label must not be empty".into());
            }
        }
        for route in &self.routes {
            if !route.path.starts_with('/') || route.path.contains(' ') {
                return Err(format!(
                    "route path '{}' must start with / and have no spaces",
                    route.path
                ));
            }
        }
        for item in &self.status {
            if item.label.trim().is_empty() {
                return Err("status item label must not be empty".into());
            }
        }
        Ok(())
    }
}

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

/// Envelope exchanged over the ipc channel: `{channel: "openkite", id,
/// plugin, request}`. `plugin` is stamped by the host from
/// `window.__openkite_plugin` (set before each bundle evaluates).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeRequest {
    pub id: u64,
    pub plugin: String,
    pub request: ApiRequest,
}

/// Per-plugin registration store, keyed by plugin name (the manifest `name`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegistrationStore {
    by_plugin: BTreeMap<String, PluginRegistration>,
}

impl RegistrationStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a plugin's registrations.
    pub fn upsert(&mut self, plugin: &str, registration: PluginRegistration) {
        self.by_plugin.insert(plugin.to_string(), registration);
    }

    /// Drop a plugin's registrations (hot-reload Removed).
    pub fn remove(&mut self, plugin: &str) -> bool {
        self.by_plugin.remove(plugin).is_some()
    }

    /// A plugin's registrations, if present.
    pub fn get(&self, plugin: &str) -> Option<&PluginRegistration> {
        self.by_plugin.get(plugin)
    }

    /// All registered sidebar items across plugins (for the shell sidebar).
    pub fn all_sidebar_items(&self) -> Vec<(&str, &SidebarItem)> {
        self.by_plugin
            .iter()
            .flat_map(|(plugin, reg)| reg.sidebar.iter().map(move |item| (plugin.as_str(), item)))
            .collect()
    }

    /// All registered routes across plugins (for the router).
    pub fn all_routes(&self) -> Vec<(&str, &RouteSpec)> {
        self.by_plugin
            .iter()
            .flat_map(|(plugin, reg)| reg.routes.iter().map(move |route| (plugin.as_str(), route)))
            .collect()
    }

    /// Number of plugins with registrations.
    pub fn len(&self) -> usize {
        self.by_plugin.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_plugin.is_empty()
    }

    /// Plugin names with registrations, sorted (stable iteration order).
    pub fn plugins(&self) -> Vec<String> {
        self.by_plugin.keys().cloned().collect()
    }

    /// All registered status items across plugins (for the status bar).
    pub fn all_status_items(&self) -> Vec<(&str, &StatusItem)> {
        self.by_plugin
            .iter()
            .flat_map(|(plugin, reg)| reg.status.iter().map(move |item| (plugin.as_str(), item)))
            .collect()
    }
}

/// The injected bootstrap: defines `window.openkite` (register* + api.*) with
/// promise plumbing over same-origin `fetch` POSTs to the `/openkite` asset
/// handler (dioxus-desktop's asset-handler registry — see
/// `docs/plugin-architecture.md` for why the ipc channel was replaced). The
/// host sets `window.__openkite_plugin` to the evaluating plugin's manifest
/// name before each bundle, then evaluates the bundle (hot reload =
/// re-evaluate).
pub const OPENKITE_BRIDGE_JS: &str = r##"(() => {
  if (window.openkite) return;
  let nextId = 1;
  const plugin = () => window.__openkite_plugin || "unknown";
  async function call(request) {
    const id = nextId++;
    const res = await fetch("/openkite", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id, plugin: plugin(), request })
    });
    if (!res.ok) throw new Error("bridge HTTP " + res.status);
    const body = await res.json();
    if (body.status === "error") throw new Error(body.error);
    return body.result;
  }
  function register(kind, payload) {
    call({ op: "register", kind, payload }).catch((err) =>
      console.warn("openkite register " + kind + " failed:", err)
    );
  }
  window.openkite = {
    registerSidebar: (item) => register("sidebar", item),
    registerRoute: (route) => register("route", route),
    registerStatusItem: (item) => register("status", item),
    api: {
      list: (kind, ns) => call({ op: "list", kind, ns: ns || null }),
      get: (kind, ns, name) => call({ op: "get", kind, ns, name }),
      watch: (kind, ns) => call({ op: "watch", kind, ns: ns || null }),
      logs: (name, ns, container) => call({ op: "logs", name, ns, container: container || null }),
      exec: (name, ns, container, cmd) => call({ op: "exec", name, ns, container: container || null, cmd })
    }
  };
})();
"##;

#[cfg(test)]
mod tests {
    use super::*;

    fn registration() -> PluginRegistration {
        PluginRegistration {
            sidebar: vec![SidebarItem {
                label: "Applications".into(),
                icon: "grid".into(),
                route: "/argocd/apps".into(),
            }],
            routes: vec![RouteSpec {
                path: "/argocd/apps".into(),
                title: "ArgoCD Applications".into(),
            }],
            status: vec![StatusItem {
                label: "ArgoCD: Synced".into(),
                color: "green".into(),
            }],
        }
    }

    #[test]
    fn registration_round_trips_through_json() {
        let json = serde_json::to_string(&registration()).unwrap();
        let back: PluginRegistration = serde_json::from_str(&json).unwrap();
        assert_eq!(back, registration());
    }

    #[test]
    fn registration_validation_catches_bad_routes_and_empty_labels() {
        let good = registration();
        assert!(good.validate().is_ok());

        let mut bad_route = good.clone();
        bad_route.routes[0].path = "argocd/apps".into();
        assert!(bad_route.validate().unwrap_err().contains("route"));

        let mut bad_label = good.clone();
        bad_label.status[0].label = "  ".into();
        assert!(bad_label.validate().unwrap_err().contains("status"));
    }

    #[test]
    fn api_request_serializes_as_tagged_ops() {
        let req = ApiRequest::List {
            kind: "pods".into(),
            ns: Some("default".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"op":"list","kind":"pods","ns":"default"}"#);
        let back: ApiRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn api_request_describe_is_human_readable() {
        assert_eq!(
            ApiRequest::Get {
                kind: "deployments".into(),
                ns: "default".into(),
                name: "web".into(),
            }
            .describe(),
            "get deployments/web"
        );
        assert_eq!(
            ApiRequest::Exec {
                name: "pod-1".into(),
                ns: "default".into(),
                container: None,
                cmd: vec!["sh".into(), "-c".into(), "ls".into()],
            }
            .describe(),
            "exec pod-1 sh -c ls"
        );
    }

    #[test]
    fn register_request_round_trips_with_json_payload() {
        let req: ApiRequest = serde_json::from_str(
            r#"{"op":"register","kind":"sidebar","payload":{"label":"Applications"}}"#,
        )
        .unwrap();
        assert!(matches!(
            &req,
            ApiRequest::Register { kind, .. } if kind == "sidebar"
        ));
        assert_eq!(req.describe(), "register sidebar");
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""op":"register""#));
        // And the payload survives.
        let back: ApiRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, req);
    }

    #[test]
    fn bridge_request_envelope_round_trips() {
        let envelope = BridgeRequest {
            id: 7,
            plugin: "argocd".into(),
            request: ApiRequest::Logs {
                name: "pod-1".into(),
                ns: "argocd".into(),
                container: None,
            },
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains(r#""plugin":"argocd""#));
        assert!(json.contains(r#""op":"logs""#));
        let back: BridgeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, envelope);
    }
    #[test]
    fn api_response_tagged_ok_and_err() {
        let ok: ApiResponse =
            serde_json::from_str(r#"{"status":"ok","result":{"items":[]}}"#).unwrap();
        assert_eq!(
            ok,
            ApiResponse::Ok {
                result: serde_json::json!({"items": []})
            }
        );
        let err: ApiResponse =
            serde_json::from_str(r#"{"status":"error","error":"no such pod"}"#).unwrap();
        assert_eq!(
            err,
            ApiResponse::Error {
                error: "no such pod".into()
            }
        );
    }

    #[test]
    fn registration_store_tracks_plugins_and_clears_on_remove() {
        let mut store = RegistrationStore::new();
        assert!(store.is_empty());
        store.upsert("argocd", registration());
        assert_eq!(store.len(), 1);
        assert_eq!(store.all_sidebar_items().len(), 1);
        assert_eq!(store.all_routes().len(), 1);
        assert!(store.remove("argocd"));
        assert!(store.is_empty());
        assert!(!store.remove("argocd"));
    }

    #[test]
    fn bridge_js_exposes_the_contract() {
        let js = OPENKITE_BRIDGE_JS;
        for needle in [
            "window.openkite",
            "registerSidebar",
            "registerRoute",
            "registerStatusItem",
            "api: {",
            "list: (kind, ns)",
            "get: (kind, ns, name)",
            "watch: (kind, ns)",
            "logs: (name, ns, container)",
            "exec: (name, ns, container, cmd)",
            r#"fetch("/openkite""#,
            "op: \"register\"",
            "__openkite_plugin",
        ] {
            assert!(js.contains(needle), "bridge JS missing {needle}");
        }
    }
}
