/// Icon for a plugin / sidebar entry: a built-in name or inline SVG.
#[derive(Debug, Clone, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_round_trip() {
        let meta = PluginMeta {
            name: "argocd".into(),
            display_name: "Argo CD".into(),
            version: "0.1.0".into(),
            author: "OpenKite".into(),
            icon: PluginIcon::BuiltIn("argocd"),
            accent_color: Some("#ef7b4d".into()),
        };
        let cloned = meta.clone();
        assert_eq!(cloned.name, "argocd");
        assert_eq!(cloned.display_name, "Argo CD");
        assert_eq!(cloned.version, "0.1.0");
        assert_eq!(cloned.author, "OpenKite");
        assert_eq!(cloned.icon, PluginIcon::BuiltIn("argocd"));
        assert_eq!(cloned.accent_color.as_deref(), Some("#ef7b4d"));
    }

    #[test]
    fn icon_variants() {
        assert_eq!(PluginIcon::BuiltIn("cube"), PluginIcon::BuiltIn("cube"));
        assert_eq!(PluginIcon::Svg("<svg/>".into()), PluginIcon::Svg("<svg/>".into()));
        assert_ne!(PluginIcon::BuiltIn("a"), PluginIcon::BuiltIn("b"));
    }
}
