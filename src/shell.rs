//! App shell model (OKT-31): sidebar structure + cluster/namespace/status
//! state.
//!
//! The pure-logic half of the shell: the unified sidebar model (core
//! sections + plugin registrations merged in order), the cluster/namespace
//! selection state, and the status-bar model (cluster, connection, plugin
//! status items). The Dioxus views (top bar, sidebar, status bar) consume
//! these — wired when the shell view lands.

use crate::plugin_api::{RegistrationStore, SidebarItem, StatusItem};

/// A sidebar entry: core nav or plugin-registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellNavItem {
    pub label: String,
    pub route: String,
    /// Plugin that contributed this item (`None` = core).
    pub plugin: Option<String>,
}

/// A sidebar section: core (built-in) or one per plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSection {
    pub label: String,
    pub items: Vec<ShellNavItem>,
}

/// Core sidebar sections (the shell's own navigation).
pub fn core_sections() -> Vec<ShellSection> {
    vec![ShellSection {
        label: "Overview".into(),
        items: vec![
            ShellNavItem {
                label: "Cluster".into(),
                route: "/cluster".into(),
                plugin: None,
            },
            ShellNavItem {
                label: "Workloads".into(),
                route: "/workloads".into(),
                plugin: None,
            },
            ShellNavItem {
                label: "Config".into(),
                route: "/config".into(),
                plugin: None,
            },
        ],
    }]
}

/// The ordered sidebar model: core sections, then one section per plugin
/// that registered sidebar items (plugin-name order, entries in
/// registration order). Plugins without sidebar items contribute no section.
pub fn sidebar_model(store: &RegistrationStore) -> Vec<ShellSection> {
    let mut sections = core_sections();
    for plugin in store.plugins() {
        let items: Vec<ShellNavItem> = store
            .get(&plugin)
            .map(|reg| {
                reg.sidebar
                    .iter()
                    .map(|item| ShellNavItem {
                        label: item.label.clone(),
                        route: item.route.clone(),
                        plugin: Some(plugin.clone()),
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !items.is_empty() {
            sections.push(ShellSection {
                label: plugin.clone(),
                items,
            });
        }
    }
    sections
}

/// Cluster/namespace selection + connection state (top bar + status bar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellState {
    /// Connected cluster context name (`None` = no cluster selected).
    pub cluster: Option<String>,
    /// Currently selected namespace (defaults to `"default"`).
    pub namespace: String,
    /// Whether a kube client is connected.
    pub connected: bool,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            cluster: None,
            namespace: "default".into(),
            connected: false,
        }
    }
}

impl ShellState {
    /// Label for the cluster slot: context name or a muted fallback.
    pub fn cluster_label(&self) -> String {
        self.cluster.clone().unwrap_or_else(|| "no cluster".into())
    }

    /// Connection pill text.
    pub fn status_label(&self) -> &'static str {
        if self.connected {
            "Connected"
        } else {
            "Disconnected"
        }
    }

    /// Clamp the selected namespace to the available list: if the current
    /// selection is missing (cluster switched), fall back to `"default"`.
    /// Returns the effective namespace.
    pub fn ensure_namespace(&mut self, namespaces: &[String]) -> &str {
        if namespaces.iter().any(|ns| *ns == self.namespace) {
            return &self.namespace;
        }
        self.namespace = "default".into();
        &self.namespace
    }
}

/// One status-bar slot: core (`cluster`/`connection`/`version`) or a plugin
/// status item (label + color).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarEntry {
    pub label: String,
    pub color: Option<String>,
    pub plugin: Option<String>,
}

/// The status-bar model: cluster, connection, version, then plugin status
/// items in plugin order.
pub fn status_bar_model(
    state: &ShellState,
    store: &RegistrationStore,
    version: &str,
) -> Vec<StatusBarEntry> {
    let mut entries = vec![
        StatusBarEntry {
            label: format!("{} · {}", state.cluster_label(), state.status_label()),
            color: Some(if state.connected { "green" } else { "red" }.to_string()),
            plugin: None,
        },
        StatusBarEntry {
            label: format!("v{version}"),
            color: None,
            plugin: None,
        },
    ];
    for (plugin, item) in store.all_status_items() {
        entries.push(StatusBarEntry {
            label: item.label.clone(),
            color: Some(item.color.clone()),
            plugin: Some(plugin.to_string()),
        });
    }
    entries
}

/// Convenience: convert a plugin [`SidebarItem`] for a [`ShellNavItem`]
/// (used by views and tests alike).
pub fn nav_item_from_plugin(plugin: &str, item: &SidebarItem) -> ShellNavItem {
    ShellNavItem {
        label: item.label.clone(),
        route: item.route.clone(),
        plugin: Some(plugin.into()),
    }
}

/// Convenience: status items contributed by a plugin.
pub fn status_items_of(store: &RegistrationStore, plugin: &str) -> Vec<&StatusItem> {
    store
        .get(plugin)
        .map(|reg| reg.status.iter().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_api::{PluginRegistration, RouteSpec, SidebarItem, StatusItem};

    fn reg_with_sidebar(plugin: &str, label: &str, route: &str) -> (String, PluginRegistration) {
        (
            plugin.into(),
            PluginRegistration {
                sidebar: vec![SidebarItem {
                    label: label.into(),
                    icon: String::new(),
                    route: route.into(),
                }],
                routes: vec![RouteSpec {
                    path: route.into(),
                    title: label.into(),
                }],
                status: vec![StatusItem {
                    label: format!("{plugin}: ok"),
                    color: "green".into(),
                }],
            },
        )
    }

    #[test]
    fn sidebar_model_lists_core_first_then_plugin_sections() {
        let mut store = RegistrationStore::new();
        store.upsert(
            "argocd",
            reg_with_sidebar("argocd", "Applications", "/argocd/apps").1,
        );
        store.upsert("istio", reg_with_sidebar("istio", "Mesh", "/istio/mesh").1);
        let sections = sidebar_model(&store);

        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].label, "Overview");
        assert_eq!(sections[0].items.len(), 3);
        assert!(sections[0].items.iter().all(|i| i.plugin.is_none()));

        assert_eq!(sections[1].label, "argocd");
        assert_eq!(sections[1].items[0].label, "Applications");
        assert_eq!(sections[1].items[0].plugin.as_deref(), Some("argocd"));

        assert_eq!(sections[2].label, "istio");
    }

    #[test]
    fn sidebar_model_skips_plugins_without_sidebar_items() {
        let mut store = RegistrationStore::new();
        store.upsert("silent", PluginRegistration::default());
        let sections = sidebar_model(&store);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].label, "Overview");
    }

    #[test]
    fn shell_state_falls_back_to_no_cluster_and_disconnected() {
        let state = ShellState::default();
        assert_eq!(state.cluster_label(), "no cluster");
        assert_eq!(state.status_label(), "Disconnected");
        let connected = ShellState {
            cluster: Some("prod".into()),
            connected: true,
            ..ShellState::default()
        };
        assert_eq!(connected.cluster_label(), "prod");
        assert_eq!(connected.status_label(), "Connected");
    }

    #[test]
    fn ensure_namespace_clamps_to_default_when_missing() {
        let mut state = ShellState {
            namespace: "team-a".into(),
            ..ShellState::default()
        };
        let available = vec!["default".to_string(), "team-b".to_string()];
        assert_eq!(state.ensure_namespace(&available), "default");
        assert_eq!(state.namespace, "default");

        let mut state = ShellState {
            namespace: "team-a".into(),
            ..ShellState::default()
        };
        let available = vec!["default".to_string(), "team-a".to_string()];
        assert_eq!(state.ensure_namespace(&available), "team-a");
    }

    #[test]
    fn status_bar_model_merges_core_and_plugin_entries() {
        let mut store = RegistrationStore::new();
        store.upsert(
            "argocd",
            reg_with_sidebar("argocd", "Applications", "/argocd/apps").1,
        );
        let state = ShellState {
            cluster: Some("prod".into()),
            connected: true,
            ..ShellState::default()
        };
        let entries = status_bar_model(&state, &store, "0.8.0");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].label, "prod · Connected");
        assert_eq!(entries[0].color.as_deref(), Some("green"));
        assert_eq!(entries[1].label, "v0.8.0");
        assert_eq!(entries[2].label, "argocd: ok");
        assert_eq!(entries[2].plugin.as_deref(), Some("argocd"));
    }

    #[test]
    fn helpers_expose_plugin_items_and_status() {
        let mut store = RegistrationStore::new();
        let (name, reg) = reg_with_sidebar("argocd", "Applications", "/argocd/apps");
        store.upsert(&name, reg);
        let item = store.get("argocd").unwrap().sidebar[0].clone();
        let nav = nav_item_from_plugin("argocd", &item);
        assert_eq!(nav.route, "/argocd/apps");
        assert_eq!(status_items_of(&store, "argocd").len(), 1);
        assert!(status_items_of(&store, "missing").is_empty());
    }
}
