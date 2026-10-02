//! App shell model: sidebar structure + cluster/namespace/status
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
    /// Count the design's nav badge shows next to the entry. Hosts publish one
    /// when they have a count to show; `None` — or the `Some(0)` [`nav_badge`]
    /// collapses — renders no badge.
    pub badge: Option<String>,
}

/// A sidebar section: core (built-in) or one per plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSection {
    /// Section header. Empty renders no header at all.
    pub label: String,
    /// Inline accent for the header and its entries (`None` = the stylesheet's
    /// own colour, no inline style). Plugin sections carry their contributor's
    /// accent; the built-in ones do not.
    pub accent: Option<String>,
    pub items: Vec<ShellNavItem>,
}

impl ShellSection {
    /// Whether this section is plugin-owned: the shell renders it through the
    /// plugin slot, never as core navigation.
    pub fn is_plugin(&self) -> bool {
        self.items.iter().any(|item| item.plugin.is_some())
    }

    /// The plugin that owns the section, when one does.
    pub fn plugin(&self) -> Option<&str> {
        self.items.iter().find_map(|item| item.plugin.as_deref())
    }
}

/// The section-variant class the shell applies to a plugin-owned section.
///
/// The reference gives Argo CD its own `.argo` zone, and that variant is the
/// shell's to draw: a plugin names itself and its entries, never its own
/// styling. Normalised names match, so `argocd`, `argo-cd` and `Argo CD` all
/// land on the reference's `.argo`; `None` keeps the default treatment.
pub fn plugin_section_variant(plugin: &str) -> Option<&'static str> {
    let normalized: String = plugin
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    match normalized.as_str() {
        "argocd" | "argo" => Some("argo"),
        _ => None,
    }
}

/// The variant class for a section: the contributing plugin's, or none for
/// core navigation.
pub fn section_variant(section: &ShellSection) -> Option<&'static str> {
    section.plugin().and_then(plugin_section_variant)
}

/// Live counts for the core navigation, one per resource the reference badges.
///
/// A field is `None` while the host has no live count for that kind — an
/// unwatched kind, or a scope with no objects — so an absent count and an
/// empty scope both render no badge instead of a zero.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NavCounts {
    pub nodes: Option<u64>,
    pub pods: Option<u64>,
    pub deployments: Option<u64>,
    pub services: Option<u64>,
    pub config_maps: Option<u64>,
    pub storage: Option<u64>,
    pub network: Option<u64>,
}

/// The `.nav-badge` text for a live count: `None` while loading and for an
/// empty scope alike, because the design never renders a zero badge.
pub fn nav_badge(count: Option<u64>) -> Option<String> {
    count
        .filter(|count| *count > 0)
        .map(|count| count.to_string())
}

/// Core sidebar sections (the shell's own navigation), with no counts — the
/// disconnected shape.
pub fn core_sections() -> Vec<ShellSection> {
    core_sections_with_counts(&NavCounts::default())
}

/// The reference sidebar's three core sections, with per-entry live counts.
///
/// Cluster (Overview, Nodes) · Workloads (Pods, Deployments, Services) ·
/// Config & Storage (ConfigMaps, Storage, Network). Entries carry the console's
/// own routes; it has no per-kind route yet, so a family shares its section's
/// route and the first entry on a route is the current one.
pub fn core_sections_with_counts(counts: &NavCounts) -> Vec<ShellSection> {
    let item = |label: &str, route: &str, badge: Option<String>| ShellNavItem {
        label: label.into(),
        route: route.into(),
        plugin: None,
        badge,
    };
    vec![
        ShellSection {
            label: "Cluster".into(),
            accent: None,
            items: vec![
                item("Overview", "/", None),
                item("Nodes", "/cluster", nav_badge(counts.nodes)),
            ],
        },
        ShellSection {
            label: "Workloads".into(),
            accent: None,
            items: vec![
                item("Pods", "/workloads", nav_badge(counts.pods)),
                item("Deployments", "/workloads", nav_badge(counts.deployments)),
                item("Services", "/workloads", nav_badge(counts.services)),
            ],
        },
        ShellSection {
            label: "Config & Storage".into(),
            accent: None,
            items: vec![
                item("ConfigMaps", "/config", nav_badge(counts.config_maps)),
                item("Storage", "/config", nav_badge(counts.storage)),
                item("Network", "/config", nav_badge(counts.network)),
            ],
        },
    ]
}

/// The ordered sidebar model: core sections, then one section per plugin
/// that registered sidebar items (plugin-name order, entries in
/// registration order). Plugins without sidebar items contribute no section.
pub fn sidebar_model(store: &RegistrationStore) -> Vec<ShellSection> {
    let mut sections = core_sections();
    sections.extend(plugin_sections(store));
    sections
}

/// The plugin-added sidebar sections only — the contents of the shell's plugin
/// slot, which the interactive shell renders after the core navigation.
///
/// A plugin never draws its own section: it registers entries and the shell
/// builds the section, so the styling (including the reference's per-plugin
/// variant, [`section_variant`]) stays with the shell. Same ordering
/// rules as [`sidebar_model`]; plugins without sidebar items contribute no
/// section.
pub fn plugin_sections(store: &RegistrationStore) -> Vec<ShellSection> {
    let mut sections = Vec::new();
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
                        badge: None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !items.is_empty() {
            sections.push(ShellSection {
                label: plugin,
                accent: None,
                items,
            });
        }
    }
    sections
}

/// One breadcrumb in the top bar (OKT-154): a step between the cluster and
/// the route the console is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crumb {
    pub label: String,
    /// The step the user is on — the design's `.breadcrumbs .current`.
    pub current: bool,
}

/// The top bar's breadcrumbs: the cluster, the section the route belongs to,
/// then the route itself. Sections come from the same sidebar model the
/// sidebar renders, so the two never disagree.
///
/// A route the model does not know (a plugin route before its registration
/// arrives) renders as the cluster plus the path, rather than an empty trail.
pub fn breadcrumbs(route: &str, cluster: Option<&str>, sections: &[ShellSection]) -> Vec<Crumb> {
    let mut crumbs = Vec::new();
    if let Some(cluster) = cluster {
        crumbs.push(Crumb {
            label: cluster.to_string(),
            current: false,
        });
    }
    let found = sections.iter().find_map(|section| {
        section
            .items
            .iter()
            .find(|item| item.route == route)
            .map(|item| (section, item))
    });
    match found {
        Some((section, item)) => {
            if !section.label.is_empty() {
                crumbs.push(Crumb {
                    label: section.label.clone(),
                    current: false,
                });
            }
            crumbs.push(Crumb {
                label: item.label.clone(),
                current: true,
            });
        }
        None if !route.trim_matches('/').is_empty() => crumbs.push(Crumb {
            label: route.trim_matches('/').to_string(),
            current: true,
        }),
        None => {}
    }
    crumbs
}

/// The avatar's initials: the first letter of the first two words of the
/// host's identity, uppercased. Empty when the host names nobody.
pub fn initials(identity: &str) -> String {
    identity
        .split_whitespace()
        .take(2)
        .filter_map(|word| word.chars().next())
        .flat_map(|c| c.to_uppercase())
        .collect()
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
    /// Detected Prometheus service name, if any (status-bar indicator).
    pub prometheus: Option<String>,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            cluster: None,
            namespace: "default".into(),
            connected: false,
            prometheus: None,
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
        if namespaces.contains(&self.namespace) {
            return &self.namespace;
        }
        self.namespace = "default".into();
        &self.namespace
    }
}

/// Prune a multi-select namespace set to the available list: drop namespaces
/// that no longer exist (cluster switched), and fall back to `["default"]`
/// when the set empties out. Pure — the namespace-chip row and the runtime
/// refresh both call this.
pub fn prune_namespaces(selected: &mut Vec<String>, available: &[String]) {
    selected.retain(|ns| available.contains(ns));
    if selected.is_empty() {
        selected.push("default".into());
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

/// The status-bar model: cluster, connection, version, Prometheus (when
/// detected), then plugin status items in plugin order.
pub fn status_bar_model(
    state: &ShellState,
    store: &RegistrationStore,
    version: &str,
) -> Vec<StatusBarEntry> {
    let mut entries = vec![StatusBarEntry {
        label: format!("{} · {}", state.cluster_label(), state.status_label()),
        color: Some(if state.connected { "green" } else { "red" }.to_string()),
        plugin: None,
    }];
    if !version.is_empty() {
        entries.push(StatusBarEntry {
            label: format!("v{version}"),
            color: None,
            plugin: None,
        });
    }
    if let Some(prometheus) = &state.prometheus {
        entries.push(StatusBarEntry {
            label: format!("Prometheus · {prometheus}"),
            color: Some("green".into()),
            plugin: None,
        });
    }
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
        badge: None,
    }
}

/// Convenience: status items contributed by a plugin.
pub fn status_items_of<'a>(store: &'a RegistrationStore, plugin: &str) -> Vec<&'a StatusItem> {
    store
        .get(plugin)
        .map(|reg| reg.status.iter().collect())
        .unwrap_or_default()
}

/// Map a status-item color to a CSS `background` value for the status dot.
///
/// Known keywords map to theme variables; anything else must look like a
/// plain CSS color (`#hex` or a color function) — arbitrary strings fall
/// back to the muted default, so a plugin cannot smuggle extra CSS
/// declarations (or a `url()` beacon) through an inline style.
pub fn status_dot_color(color: &str) -> String {
    let trimmed = color.trim();
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "green" | "ok" | "healthy" => "var(--success)".into(),
        "yellow" | "warn" | "progressing" => "var(--warn)".into(),
        "red" | "error" | "critical" => "var(--danger)".into(),
        "blue" | "info" => "var(--accent)".into(),
        other if is_css_color(other) => other.to_string(),
        _ => "var(--subtle)".into(),
    }
}

/// One status-bar slot as render data: the label, and the inline
/// `background` for its dot (`display: none` when the entry carries no dot).
/// The status footer renders these; both hosts build them from the same model.
pub fn status_rows(entries: &[StatusBarEntry]) -> Vec<(String, String)> {
    entries
        .iter()
        .map(|entry| {
            let dot = match entry.color.as_deref() {
                Some(color) => format!("background: {}", status_dot_color(color)),
                None => "display: none".into(),
            };
            (entry.label.clone(), dot)
        })
        .collect()
}

/// A conservative CSS color check: short hex or one of a few color
/// functions, with no characters that could break out of the value.
fn is_css_color(s: &str) -> bool {
    if let Some(hex) = s.strip_prefix('#') {
        return (s.len() <= 9) && hex.bytes().all(|b| b.is_ascii_hexdigit());
    }
    if !s.ends_with(')') || s.contains([';', '{', '}', '"', '\'', '<']) {
        return false;
    }
    match s.split_once('(') {
        Some((func, _)) => matches!(func, "rgb" | "rgba" | "hsl" | "hsla" | "oklch"),
        None => false,
    }
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
                renderers: Vec::new(),
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

        assert_eq!(sections.len(), 5, "three core sections then two plugins");
        assert_eq!(
            sections
                .iter()
                .take(3)
                .map(|section| section.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Cluster", "Workloads", "Config & Storage"]
        );
        assert_eq!(sections[0].items.len(), 2);
        assert!(sections[0].items.iter().all(|i| i.plugin.is_none()));
        assert!(!sections[0].is_plugin());

        assert_eq!(sections[3].label, "argocd");
        assert_eq!(sections[3].items[0].label, "Applications");
        assert_eq!(sections[3].items[0].plugin.as_deref(), Some("argocd"));
        assert!(sections[3].is_plugin());

        assert_eq!(sections[4].label, "istio");
    }

    #[test]
    fn sidebar_model_skips_plugins_without_sidebar_items() {
        let mut store = RegistrationStore::new();
        store.upsert("silent", PluginRegistration::default());
        let sections = sidebar_model(&store);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].label, "Cluster");
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
    fn prune_namespaces_drops_missing_and_defaults_when_empty() {
        let mut selected = vec!["team-a".to_string(), "ghost".to_string()];
        let available = vec!["default".to_string(), "team-a".to_string()];
        prune_namespaces(&mut selected, &available);
        assert_eq!(selected, vec!["team-a".to_string()]);

        let mut selected = vec!["ghost".to_string()];
        prune_namespaces(&mut selected, &available);
        assert_eq!(selected, vec!["default".to_string()]);

        let mut selected = vec!["team-a".to_string()];
        prune_namespaces(&mut selected, &available);
        assert_eq!(selected, vec!["team-a".to_string()]);
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

    #[test]
    fn status_rows_map_colors_and_hide_undotted_entries() {
        let entries = vec![
            StatusBarEntry {
                label: "prod · Connected".into(),
                color: Some("green".into()),
                plugin: None,
            },
            StatusBarEntry {
                label: "v0.0.0".into(),
                color: None,
                plugin: None,
            },
        ];
        let rows = status_rows(&entries);
        assert_eq!(rows[0].0, "prod · Connected");
        assert_eq!(rows[0].1, "background: var(--success)");
        assert_eq!(rows[1].0, "v0.0.0");
        assert_eq!(rows[1].1, "display: none");
    }

    #[test]
    fn core_sections_match_the_reference_and_carry_no_counts_when_disconnected() {
        let sections = core_sections();
        assert_eq!(
            sections
                .iter()
                .map(|section| section.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Cluster", "Workloads", "Config & Storage"]
        );
        assert!(sections.iter().all(|section| !section.is_plugin()));
        fn routes(section: &ShellSection) -> Vec<(&str, &str)> {
            section
                .items
                .iter()
                .map(|item| (item.label.as_str(), item.route.as_str()))
                .collect()
        }
        assert_eq!(
            routes(&sections[0]),
            vec![("Overview", "/"), ("Nodes", "/cluster")]
        );
        assert_eq!(
            routes(&sections[1]),
            vec![
                ("Pods", "/workloads"),
                ("Deployments", "/workloads"),
                ("Services", "/workloads")
            ]
        );
        assert_eq!(
            routes(&sections[2]),
            vec![
                ("ConfigMaps", "/config"),
                ("Storage", "/config"),
                ("Network", "/config")
            ]
        );
        // Disconnected: every badge is absent, never "0".
        assert!(sections
            .iter()
            .flat_map(|section| section.items.iter())
            .all(|item| item.badge.is_none()));
    }

    #[test]
    fn counts_badge_only_positive_numbers_in_their_own_rows() {
        let counts = NavCounts {
            nodes: Some(8),
            pods: Some(124),
            deployments: Some(37),
            services: Some(29),
            config_maps: Some(46),
            // A known-but-empty scope is absent, not zero.
            storage: Some(0),
            // An unwatched kind has no count at all.
            network: None,
        };
        let sections = core_sections_with_counts(&counts);
        let badge = |label: &str| {
            sections
                .iter()
                .flat_map(|section| section.items.iter())
                .find(|item| item.label == label)
                .and_then(|item| item.badge.clone())
        };
        assert_eq!(badge("Nodes").as_deref(), Some("8"));
        assert_eq!(badge("Pods").as_deref(), Some("124"));
        assert_eq!(badge("Deployments").as_deref(), Some("37"));
        assert_eq!(badge("Services").as_deref(), Some("29"));
        assert_eq!(badge("ConfigMaps").as_deref(), Some("46"));
        assert_eq!(badge("Storage"), None);
        assert_eq!(badge("Network"), None);
        assert_eq!(badge("Overview"), None);
    }

    #[test]
    fn nav_badge_treats_loading_and_an_empty_scope_alike() {
        assert_eq!(nav_badge(None), None);
        assert_eq!(nav_badge(Some(0)), None);
        assert_eq!(nav_badge(Some(1)).as_deref(), Some("1"));
    }

    #[test]
    fn plugin_sections_take_the_reference_variant_the_shell_styles() {
        let mut store = RegistrationStore::new();
        store.upsert(
            "argocd",
            reg_with_sidebar("argocd", "Applications", "/argocd/apps").1,
        );
        store.upsert("istio", reg_with_sidebar("istio", "Mesh", "/istio/mesh").1);
        let plugins = plugin_sections(&store);
        assert_eq!(section_variant(&plugins[0]), Some("argo"));
        assert_eq!(section_variant(&plugins[1]), None);
        // The display label normalises to the same variant as the plugin name.
        assert_eq!(plugin_section_variant("Argo CD"), Some("argo"));
        assert_eq!(plugin_section_variant("argo-cd"), Some("argo"));
        assert_eq!(plugin_section_variant("istio"), None);
        // Core sections never carry a plugin variant.
        assert!(core_sections()
            .iter()
            .all(|section| section_variant(section).is_none()));
    }

    #[test]
    fn status_dot_color_maps_keywords_and_plain_colors() {
        assert_eq!(status_dot_color("green"), "var(--success)");
        assert_eq!(status_dot_color(" Healthy "), "var(--success)");
        assert_eq!(status_dot_color("error"), "var(--danger)");
        assert_eq!(status_dot_color("BLUE"), "var(--accent)");
        assert_eq!(status_dot_color("#0d9488"), "#0d9488");
        assert_eq!(status_dot_color("rgb(13,148,136)"), "rgb(13,148,136)");
        assert_eq!(status_dot_color("oklch(66% 0.1 180)"), "oklch(66% 0.1 180)");
    }

    #[test]
    fn status_dot_color_rejects_non_color_strings() {
        assert_eq!(status_dot_color(""), "var(--subtle)");
        assert_eq!(status_dot_color("orange"), "var(--subtle)");
        assert_eq!(status_dot_color("red;} *{display:none"), "var(--subtle)");
        assert_eq!(
            status_dot_color("url(https://evil.test/x)"),
            "var(--subtle)"
        );
        assert_eq!(
            status_dot_color("var(--success)\",background:url(a)"),
            "var(--subtle)"
        );
    }

    #[test]
    fn breadcrumbs_name_the_cluster_the_section_and_the_route() {
        let sections = core_sections();
        let crumbs = breadcrumbs("/workloads", Some("prod"), &sections);
        let labels: Vec<&str> = crumbs.iter().map(|crumb| crumb.label.as_str()).collect();
        assert_eq!(labels, vec!["prod", "Workloads", "Pods"]);
        assert_eq!(
            crumbs.iter().filter(|crumb| crumb.current).count(),
            1,
            "exactly the route is current"
        );
        assert!(crumbs.last().expect("a route crumb").current);
    }

    #[test]
    fn breadcrumbs_fall_back_to_the_path_and_skip_what_they_cannot_name() {
        let sections = core_sections();
        // A plugin route the model has not registered yet still reads honestly.
        let crumbs = breadcrumbs("/argocd/apps", None, &sections);
        assert_eq!(crumbs.len(), 1);
        assert_eq!(crumbs[0].label, "argocd/apps");
        assert!(crumbs[0].current);

        // No route, no cluster: nothing to say, so nothing renders.
        assert!(breadcrumbs("", None, &sections).is_empty());
    }

    #[test]
    fn initials_are_the_first_letters_of_the_first_two_words() {
        assert_eq!(initials("Eda Kite"), "EK");
        assert_eq!(initials("eda kite operator"), "EK");
        assert_eq!(initials("hermes"), "H");
        assert_eq!(initials("   "), "");
    }
}
