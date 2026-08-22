use crate::meta::PluginIcon;

/// A named group of sidebar entries (e.g. "Argo CD").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarSection {
    pub label: String,
    pub icon: PluginIcon,
    pub accent_color: Option<String>,
    pub entries: Vec<SidebarEntry>,
}

/// A single clickable sidebar entry, mapped to a plugin route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarEntry {
    pub label: String,
    pub icon: PluginIcon,
    /// e.g. "/argocd/apps"
    pub route: String,
    /// Optional count badge (e.g. "12" for app count).
    pub badge: Option<String>,
    pub badge_color: Option<String>,
}
