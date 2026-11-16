//! Catalogue-driven navigation: the sidebar tree derives from the resource catalogue.

/// One resource kind as the navigation tree sees it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CatalogueKind {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub icon: &'static str,
}

/// Groups in their curated order; the reference prototype's nine.
pub const GROUP_ORDER: &[&str] = &[
    "Cluster",
    "Workloads",
    "Config",
    "Network",
    "Storage",
    "Access Control",
    "Helm",
    "Custom Resources",
    "Argo CD",
];

/// Landing place for any kind the catalogue adds without a curated group.
pub const OTHER_GROUP: &str = "Other";

/// The resource catalogue, in curated order.
pub const CATALOGUE: &[CatalogueKind] = &[
    CatalogueKind {
        key: "nodes",
        label: "Nodes",
        group: "Cluster",
        icon: "node",
    },
    CatalogueKind {
        key: "namespaces",
        label: "Namespaces",
        group: "Cluster",
        icon: "cluster",
    },
    CatalogueKind {
        key: "events",
        label: "Events",
        group: "Cluster",
        icon: "warn",
    },
    CatalogueKind {
        key: "pods",
        label: "Pods",
        group: "Workloads",
        icon: "pods",
    },
    CatalogueKind {
        key: "deployments",
        label: "Deployments",
        group: "Workloads",
        icon: "deploy",
    },
    CatalogueKind {
        key: "daemonsets",
        label: "DaemonSets",
        group: "Workloads",
        icon: "node",
    },
    CatalogueKind {
        key: "statefulsets",
        label: "StatefulSets",
        group: "Workloads",
        icon: "storage",
    },
    CatalogueKind {
        key: "replicasets",
        label: "ReplicaSets",
        group: "Workloads",
        icon: "deploy",
    },
    CatalogueKind {
        key: "jobs",
        label: "Jobs",
        group: "Workloads",
        icon: "check",
    },
    CatalogueKind {
        key: "cronjobs",
        label: "CronJobs",
        group: "Workloads",
        icon: "clock",
    },
    CatalogueKind {
        key: "configmaps",
        label: "ConfigMaps",
        group: "Config",
        icon: "config",
    },
    CatalogueKind {
        key: "secrets",
        label: "Secrets",
        group: "Config",
        icon: "config",
    },
    CatalogueKind {
        key: "hpas",
        label: "HPA",
        group: "Config",
        icon: "scale",
    },
    CatalogueKind {
        key: "services",
        label: "Services",
        group: "Network",
        icon: "services",
    },
    CatalogueKind {
        key: "ingresses",
        label: "Ingresses",
        group: "Network",
        icon: "network",
    },
    CatalogueKind {
        key: "networkpolicies",
        label: "NetworkPolicies",
        group: "Network",
        icon: "network",
    },
    CatalogueKind {
        key: "pvcs",
        label: "PersistentVolumeClaims",
        group: "Storage",
        icon: "storage",
    },
    CatalogueKind {
        key: "pvs",
        label: "PersistentVolumes",
        group: "Storage",
        icon: "storage",
    },
    CatalogueKind {
        key: "storageclasses",
        label: "StorageClasses",
        group: "Storage",
        icon: "storage",
    },
    CatalogueKind {
        key: "serviceaccounts",
        label: "ServiceAccounts",
        group: "Access Control",
        icon: "projects",
    },
    CatalogueKind {
        key: "roles",
        label: "Roles",
        group: "Access Control",
        icon: "projects",
    },
    CatalogueKind {
        key: "clusterroles",
        label: "ClusterRoles",
        group: "Access Control",
        icon: "projects",
    },
    CatalogueKind {
        key: "rolebindings",
        label: "RoleBindings",
        group: "Access Control",
        icon: "projects",
    },
    CatalogueKind {
        key: "helm",
        label: "Releases",
        group: "Helm",
        icon: "repo",
    },
    CatalogueKind {
        key: "crds",
        label: "Definitions",
        group: "Custom Resources",
        icon: "projects",
    },
    CatalogueKind {
        key: "certificates",
        label: "Certificates",
        group: "Custom Resources",
        icon: "check",
    },
    CatalogueKind {
        key: "servicemonitors",
        label: "ServiceMonitors",
        group: "Custom Resources",
        icon: "network",
    },
];

/// A non-kind entry that still appears in the nav.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CatalogueView {
    pub label: &'static str,
    pub icon: &'static str,
    pub path: &'static str,
    pub group: &'static str,
    pub enabled: bool,
}

/// Views the reference shows beside the kinds; disabled ones are product placeholders.
pub const VIEWS: &[CatalogueView] = &[
    CatalogueView {
        label: "Overview",
        icon: "cluster",
        path: "/overview",
        group: "Cluster",
        enabled: true,
    },
    CatalogueView {
        label: "Applications",
        icon: "argo",
        path: "/argocd",
        group: "Argo CD",
        enabled: true,
    },
    CatalogueView {
        label: "Projects",
        icon: "projects",
        path: "/argocd/projects",
        group: "Argo CD",
        enabled: false,
    },
    CatalogueView {
        label: "Repositories",
        icon: "repo",
        path: "/argocd/repositories",
        group: "Argo CD",
        enabled: false,
    },
];

/// One row of the navigation tree.
#[derive(Clone, PartialEq, Debug)]
pub struct NavEntry {
    pub label: &'static str,
    pub icon: &'static str,
    pub path: String,
    /// Set for resource kinds; the key a counts frame is looked up by.
    pub badge_kind: Option<&'static str>,
    pub enabled: bool,
}

/// A titled group of rows.
#[derive(Clone, PartialEq, Debug)]
pub struct NavGroup {
    pub title: &'static str,
    pub entries: Vec<NavEntry>,
}

/// Lower-case, dash-separated form of a group title, used as the first path segment.
pub fn group_slug(group: &str) -> String {
    let slug: String = group
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    slug.split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn entry_path(group: &str, key: &str) -> String {
    format!("/{}/{}", group_slug(group), key)
}

/// Builds the tree: curated group order, catalogue order inside a group, unknown groups last.
pub fn build_nav(kinds: &[CatalogueKind], views: &[CatalogueView]) -> Vec<NavGroup> {
    let mut groups: Vec<NavGroup> = Vec::new();
    for title in GROUP_ORDER {
        let mut entries: Vec<NavEntry> = views
            .iter()
            .filter(|v| v.group == *title)
            .map(|v| NavEntry {
                label: v.label,
                icon: v.icon,
                path: v.path.to_string(),
                badge_kind: None,
                enabled: v.enabled,
            })
            .collect();
        entries.extend(
            kinds
                .iter()
                .filter(|k| k.group == *title)
                .map(|k| NavEntry {
                    label: k.label,
                    icon: k.icon,
                    path: entry_path(title, k.key),
                    badge_kind: Some(k.key),
                    enabled: true,
                }),
        );
        if !entries.is_empty() {
            groups.push(NavGroup { title, entries });
        }
    }
    let other: Vec<NavEntry> = kinds
        .iter()
        .filter(|k| !GROUP_ORDER.contains(&k.group))
        .map(|k| NavEntry {
            label: k.label,
            icon: k.icon,
            path: entry_path(OTHER_GROUP, k.key),
            badge_kind: Some(k.key),
            enabled: true,
        })
        .collect();
    if !other.is_empty() {
        groups.push(NavGroup {
            title: OTHER_GROUP,
            entries: other,
        });
    }
    groups
}

/// Kinds the tree failed to route. Empty means nav coverage holds.
pub fn coverage_gaps(kinds: &[CatalogueKind], groups: &[NavGroup]) -> Vec<&'static str> {
    let routed: std::collections::BTreeSet<&str> = groups
        .iter()
        .flat_map(|g| g.entries.iter())
        .filter_map(|e| e.badge_kind)
        .collect();
    kinds
        .iter()
        .map(|k| k.key)
        .filter(|k| !routed.contains(k))
        .collect()
}

/// The group and row a path resolves to, for breadcrumbs and the active state.
pub fn entry_for_path<'a>(
    groups: &'a [NavGroup],
    path: &str,
) -> Option<(&'a NavGroup, &'a NavEntry)> {
    groups
        .iter()
        .find_map(|g| g.entries.iter().find(|e| e.path == path).map(|e| (g, e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn other_kind() -> CatalogueKind {
        CatalogueKind {
            key: "widgets",
            label: "Widgets",
            group: "Widgets",
            icon: "cube",
        }
    }

    #[test]
    fn every_catalogue_kind_is_routed_exactly_once() {
        let groups = build_nav(CATALOGUE, VIEWS);
        assert!(
            coverage_gaps(CATALOGUE, &groups).is_empty(),
            "unrouted kinds: {:?}",
            coverage_gaps(CATALOGUE, &groups)
        );
        let keys: Vec<&str> = groups
            .iter()
            .flat_map(|g| g.entries.iter())
            .filter_map(|e| e.badge_kind)
            .collect();
        assert_eq!(keys.len(), CATALOGUE.len());
    }

    #[test]
    fn paths_are_unique() {
        let groups = build_nav(CATALOGUE, VIEWS);
        let mut seen = std::collections::BTreeSet::new();
        for entry in groups.iter().flat_map(|g| g.entries.iter()) {
            assert!(
                seen.insert(entry.path.clone()),
                "duplicate path {}",
                entry.path
            );
            assert!(
                entry.path.starts_with('/'),
                "path must be absolute: {}",
                entry.path
            );
        }
    }

    #[test]
    fn groups_follow_the_curated_order() {
        let groups = build_nav(CATALOGUE, VIEWS);
        let titles: Vec<&str> = groups.iter().map(|g| g.title).collect();
        assert_eq!(titles, GROUP_ORDER);
    }

    #[test]
    fn kinds_keep_catalogue_order_inside_a_group() {
        let groups = build_nav(CATALOGUE, VIEWS);
        let workloads = groups
            .iter()
            .find(|g| g.title == "Workloads")
            .expect("workloads group");
        let keys: Vec<&str> = workloads
            .entries
            .iter()
            .filter_map(|e| e.badge_kind)
            .collect();
        let expected: Vec<&str> = CATALOGUE
            .iter()
            .filter(|k| k.group == "Workloads")
            .map(|k| k.key)
            .collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn ungrouped_kind_lands_in_other_and_still_gets_a_route() {
        let mut kinds = CATALOGUE.to_vec();
        kinds.push(other_kind());
        let groups = build_nav(&kinds, VIEWS);
        assert_eq!(groups.last().map(|g| g.title), Some(OTHER_GROUP));
        assert!(coverage_gaps(&kinds, &groups).is_empty());
        let (group, entry) = entry_for_path(&groups, "/other/widgets").expect("routed widget");
        assert_eq!(group.title, OTHER_GROUP);
        assert_eq!(entry.label, "Widgets");
    }

    #[test]
    fn views_render_before_kinds_in_their_group() {
        let groups = build_nav(CATALOGUE, VIEWS);
        let cluster = groups
            .iter()
            .find(|g| g.title == "Cluster")
            .expect("cluster group");
        assert_eq!(cluster.entries.first().map(|e| e.label), Some("Overview"));
        assert_eq!(cluster.entries.first().and_then(|e| e.badge_kind), None);
        let argo = groups
            .iter()
            .find(|g| g.title == "Argo CD")
            .expect("argo group");
        assert_eq!(argo.entries.first().map(|e| e.label), Some("Applications"));
        assert!(
            argo.entries.iter().any(|e| !e.enabled),
            "placeholder rows keep enabled=false"
        );
    }

    #[test]
    fn group_slug_is_path_safe() {
        assert_eq!(group_slug("Access Control"), "access-control");
        assert_eq!(group_slug("Argo CD"), "argo-cd");
    }

    #[test]
    fn every_nav_icon_exists_in_the_sprite() {
        let sprite =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons.svg"))
                .expect("icons sprite");
        for kind in CATALOGUE {
            assert!(
                sprite.contains(&format!("id=\"i-{}\"", kind.icon)),
                "missing kind icon i-{}",
                kind.icon
            );
        }
        for view in VIEWS {
            assert!(
                sprite.contains(&format!("id=\"i-{}\"", view.icon)),
                "missing view icon i-{}",
                view.icon
            );
        }
        for chrome in ["menu", "search", "chevron", "cluster"] {
            assert!(
                sprite.contains(&format!("id=\"i-{chrome}\"")),
                "missing chrome icon i-{chrome}"
            );
        }
    }
}
