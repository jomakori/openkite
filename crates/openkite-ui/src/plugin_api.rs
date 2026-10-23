//! Plugin API bridge: the JS surface plugins call.
//!
//! External plugins are JS bundles the host evaluates in its webview. This module
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
//! The runtime wiring lives in the host's bridge
//! (`openkite_host::bridge::Bridge::handle_post` → registration store / kube
//! dispatch); the app shell mounts the handler.

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

/// A renderer declaration: the plugin says "I render body content for this
/// path" so the host's `Route::Plugin` wildcard can route into the JS slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererSpec {
    pub path: String,
}

/// A control a plugin's surface offers, and — when the plugin cannot honour it
/// on this host — why not (plugin system v1.1).
///
/// The plugin owns the label and the action id; the crate owns the button, so
/// a control the plugin declares it cannot support still shows up on the
/// surface, disabled, with `data-unsupported` and the reason — instead of
/// disappearing and leaving the user to guess.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginControl {
    pub label: String,
    /// What the host's handler switches on when it wires this control.
    #[serde(default)]
    pub action: String,
    /// Why the plugin cannot honour this control here. `None` = it can, and
    /// the control is live once the host wires a handler.
    #[serde(default)]
    pub unsupported: Option<String>,
    /// The plugin that declared it, stamped by the registry on registration.
    /// The crate attributes the painted button with it, so the plugin's own
    /// scoped sheet can reach controls the host renders for it.
    #[serde(default)]
    pub plugin: String,
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
    #[serde(default)]
    pub renderers: Vec<RendererSpec>,
    /// The controls the plugin's own surface offers (v1.1).
    #[serde(default)]
    pub controls: Vec<PluginControl>,
    /// The plugin's stylesheet, as authored (v1.1). Never injected raw: the
    /// shell scopes it to the plugin's own section through
    /// [`crate::plugin_css::scoped_style`].
    #[serde(default)]
    pub style: Option<String>,
}

impl PluginRegistration {
    /// Validate every entry: non-empty labels/titles, route/renderer paths
    /// start with `/` and contain no spaces.
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
        for renderer in &self.renderers {
            if !renderer.path.starts_with('/') || renderer.path.contains(' ') {
                return Err(format!(
                    "renderer path '{}' must start with / and have no spaces",
                    renderer.path
                ));
            }
        }
        // v1.1: a control the plugin cannot honour must say why — an
        // unexplained disabled button is the dead control the shell refuses.
        for control in &self.controls {
            if control.label.trim().is_empty() {
                return Err("control label must not be empty".into());
            }
            if control.action.trim().is_empty() {
                return Err(format!(
                    "control '{}' must name the action it fires",
                    control.label
                ));
            }
            if let Some(reason) = &control.unsupported {
                if reason.trim().is_empty() {
                    return Err(format!(
                        "control '{}' is marked unsupported without a reason",
                        control.label
                    ));
                }
            }
        }
        // v1.1: the stylesheet rides the registration payload, so it is
        // bounded here and scoped at render time.
        if let Some(style) = &self.style {
            if style.len() > crate::plugin_css::MAX_STYLE_BYTES {
                return Err(format!(
                    "plugin stylesheet is {} bytes, over the {} byte limit",
                    style.len(),
                    crate::plugin_css::MAX_STYLE_BYTES
                ));
            }
        }
        Ok(())
    }
}

/// The wire types live in the contract crate, re-exported here so the shell,
/// the bridge and both hosts keep one import path.
pub use openkite_api::bridge::{ApiRequest, ApiResponse, BridgeRequest};

/// Per-plugin registration store, keyed by plugin name (the manifest `name`).
///
/// v1.1 also keeps the load *failures*: a plugin the host could not load has
/// no registrations, but it does have a place in the navigation — disabled,
/// with the reason — so the shell tells the truth instead of dropping it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegistrationStore {
    by_plugin: BTreeMap<String, PluginRegistration>,
    failures: BTreeMap<String, String>,
}

impl RegistrationStore {
    /// An empty registration store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a plugin's registrations.
    ///
    /// A successful registration clears any failure recorded for that plugin:
    /// the plugin loaded, so the disabled entry must go (hot reload).
    pub fn upsert(&mut self, plugin: &str, registration: PluginRegistration) {
        self.failures.remove(plugin);
        self.by_plugin.insert(plugin.to_string(), registration);
    }

    /// Record that a plugin failed to load, and drop whatever it had
    /// registered before: a section built from a stale registration is worse
    /// than an entry that says it is unavailable.
    pub fn fail(&mut self, plugin: &str, reason: &str) {
        self.by_plugin.remove(plugin);
        self.failures.insert(plugin.to_string(), reason.to_string());
    }

    /// Whether the plugin is currently recorded as failed.
    pub fn is_failed(&self, plugin: &str) -> bool {
        self.failures.contains_key(plugin)
    }

    /// The recorded load failures: `(plugin, reason)`, sorted by plugin name.
    pub fn failures(&self) -> Vec<(&str, &str)> {
        self.failures
            .iter()
            .map(|(plugin, reason)| (plugin.as_str(), reason.as_str()))
            .collect()
    }

    /// Drop a plugin's registrations (hot-reload Removed).
    pub fn remove(&mut self, plugin: &str) -> bool {
        let failed = self.failures.remove(plugin).is_some();
        self.by_plugin.remove(plugin).is_some() || failed
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

    /// Whether no plugins have registrations.
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

    /// All registered renderer paths across plugins (for the router's
    /// JS-route slot fallback): `(plugin, path)`. Order is stable
    /// (BTreeMap by plugin name, then registration order).
    pub fn all_renderer_paths(&self) -> Vec<(&str, &str)> {
        self.by_plugin
            .iter()
            .flat_map(|(plugin, reg)| {
                reg.renderers
                    .iter()
                    .map(move |renderer| (plugin.as_str(), renderer.path.as_str()))
            })
            .collect()
    }

    /// The plugin that registered `path` (sidebar entry, route or renderer),
    /// for surfaces that need to draw the owning plugin's own affordances.
    pub fn owner_of_route(&self, path: &str) -> Option<&str> {
        self.by_plugin
            .iter()
            .find(|(_, reg)| {
                reg.routes.iter().any(|route| route.path == path)
                    || reg.renderers.iter().any(|renderer| renderer.path == path)
                    || reg.sidebar.iter().any(|item| item.route == path)
            })
            .map(|(plugin, _)| plugin.as_str())
    }

    /// The controls the plugin owning `path` declared (v1.1). Empty when no
    /// plugin owns the path or the plugin declared none.
    pub fn controls_for_route(&self, path: &str) -> Vec<PluginControl> {
        self.owner_of_route(path)
            .and_then(|plugin| self.by_plugin.get(plugin))
            .map(|reg| reg.controls.clone())
            .unwrap_or_default()
    }

    /// Every plugin stylesheet, as authored: `(plugin, css)`.
    pub fn all_styles(&self) -> Vec<(&str, &str)> {
        self.by_plugin
            .iter()
            .filter_map(|(plugin, reg)| reg.style.as_deref().map(|css| (plugin.as_str(), css)))
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
    registerRouteRenderer: (path) => register("route_renderer", { path }),
    registerStyles: (css) => register("style", { css }),
    registerControl: (control) => register("control", control),
    // The plugin's side of the control model: the shell paints the buttons a
    // plugin declared with `registerControl`, and hands clicks back here.
    onControl: function (handler) {
      window.openkite._controlHandler = handler;
    },
    _control: function (pluginName, action) {
      const handler = window.openkite._controlHandler;
      if (typeof handler === "function") handler(action, pluginName);
    },
    _renderRoute: function (path, container) {
      return function unmount() {};
    },
    api: {
      list: (kind, ns) => call({ op: "list", kind, ns: ns || null }),
      get: (kind, ns, name) => call({ op: "get", kind, ns, name }),
      watch: (kind, ns) => call({ op: "watch", kind, ns: ns || null }),
      logs: (name, ns, container) => call({ op: "logs", name, ns, container: container || null }),
      exec: (name, ns, container, cmd) => call({ op: "exec", name, ns, container: container || null, cmd })
    },
    _pushHandlers: new Map(),
    subscribe: function (opts, handler) {
      const kind = typeof opts === "string" ? opts : (opts && opts.kind);
      const ns = (typeof opts === "string" ? null : (opts && opts.ns)) || null;
      let sub = null;
      let cancelled = false;
      call({ op: "subscribe", kind, ns })
        .then((res) => {
          sub = res && res.sub;
          if (sub == null) return;
          if (cancelled) {
            call({ op: "unsubscribe", sub }).catch(() => {});
            return;
          }
          window.openkite._pushHandlers.set(sub, handler);
          if (res.initial !== undefined && handler) {
            try {
              handler({ sub: sub, kind: kind, ns: ns, rows: res.initial, revision: 0, initial: true });
            } catch (e) {
              console.warn("openkite subscribe handler threw:", e);
            }
          }
        })
        .catch((err) => console.warn("openkite subscribe failed:", err));
      return function unsubscribe() {
        cancelled = true;
        if (sub == null) return;
        window.openkite._pushHandlers.delete(sub);
        call({ op: "unsubscribe", sub }).catch(() => {});
      };
    },
    _pushState: function (msg) {
      const handler = window.openkite._pushHandlers.get(msg && msg.sub);
      if (!handler) return;
      try {
        handler(msg);
      } catch (e) {
        console.warn("openkite push handler threw:", e);
      }
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
            renderers: vec![RendererSpec {
                path: "/argocd/apps".into(),
            }],
            controls: Vec::new(),
            style: None,
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

        let mut bad_renderer = good.clone();
        bad_renderer.renderers[0].path = "argocd".into();
        assert!(bad_renderer.validate().unwrap_err().contains("renderer"));
    }

    #[test]
    fn registration_validation_catches_empty_sidebar_and_spaced_paths() {
        let mut sidebar_bad = registration();
        sidebar_bad.sidebar[0].label = " ".into();
        assert_eq!(
            sidebar_bad.validate().unwrap_err(),
            "sidebar item label must not be empty"
        );

        let mut spaced_route = registration();
        spaced_route.routes[0].path = "/argocd apps".into();
        assert!(spaced_route.validate().unwrap_err().contains("spaces"));

        let mut spaced_renderer = registration();
        spaced_renderer.renderers[0].path = "/argocd apps".into();
        assert!(spaced_renderer.validate().unwrap_err().contains("spaces"));
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
    fn api_request_describe_covers_list_watch_logs() {
        assert_eq!(
            ApiRequest::List {
                kind: "pods".into(),
                ns: Some("default".into()),
            }
            .describe(),
            "list pods"
        );
        assert_eq!(
            ApiRequest::Watch {
                kind: "pods".into(),
                ns: None,
            }
            .describe(),
            "watch pods"
        );
        assert_eq!(
            ApiRequest::Logs {
                name: "pod-1".into(),
                ns: "default".into(),
                container: Some("app".into()),
            }
            .describe(),
            "logs pod-1"
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
        assert_eq!(store.all_renderer_paths().len(), 1);
        assert!(store.remove("argocd"));
        assert!(store.is_empty());
        assert!(!store.remove("argocd"));
    }

    #[test]
    fn registration_store_get_status_and_plugin_names() {
        let mut store = RegistrationStore::new();
        assert_eq!(store.get("argocd"), None);
        assert!(store.plugins().is_empty());

        store.upsert("argocd", registration());
        assert_eq!(store.get("argocd").map(|r| r.routes.len()), Some(1));
        assert_eq!(store.all_status_items().len(), 1);
        assert_eq!(store.plugins(), vec!["argocd".to_string()]);
        assert!(!store.is_empty());
    }

    #[test]
    fn bridge_js_exposes_the_contract() {
        let js = OPENKITE_BRIDGE_JS;
        for needle in [
            "window.openkite",
            "registerSidebar",
            "registerRoute",
            "registerStatusItem",
            "registerRouteRenderer",
            "registerStyles",
            "registerControl",
            "onControl",
            "_control",
            "_renderRoute",
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
