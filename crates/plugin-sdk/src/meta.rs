/// Icon for a plugin / sidebar entry: a built-in name or inline SVG.
#[derive(Debug, Clone)]
pub enum PluginIcon {
    BuiltIn(&'static str),
    Svg(String),
}

/// Plugin identity, shown in the plugin manager and sidebar.
#[derive(Debug, Clone)]
pub struct PluginMeta {
    /// Machine name, e.g. "argocd".
    pub name: String,
    /// Human name, e.g. "Argo CD".
    pub display_name: String,
    pub version: String,
    pub author: String,
    pub icon: PluginIcon,
    /// e.g. "#ef7b4d" for ArgoCD orange.
    pub accent_color: Option<String>,
}
