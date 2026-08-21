//! Local app configuration persisted at `~/.openkite/config.toml`.

use openkite_plugin_sdk::anyhow;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Local OpenKite configuration. Currently holds the plugin enable/disable
/// state; theme + settings grow this in their own tickets (OKT-17/OKT-19).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenKiteConfig {
    /// Explicitly enabled plugin names. Empty = all plugins enabled.
    pub enabled_plugins: Vec<String>,
    /// Explicitly disabled plugin names (wins over `enabled_plugins`).
    pub disabled_plugins: Vec<String>,
}

impl OpenKiteConfig {
    /// Load from `~/.openkite/config.toml`; missing/corrupt file → default.
    pub fn load() -> Self {
        Self::load_from(&Self::path())
    }

    /// Load from a specific file (testable).
    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist to `~/.openkite/config.toml`, creating the directory if needed.
    pub fn save(&self) -> anyhow::Result<()> {
        self.save_to(&Self::path())
    }

    /// Persist to a specific file (testable).
    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let raw = toml::to_string(self)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    /// Whether a plugin is enabled. Explicitly disabled wins; otherwise a
    /// non-empty `enabled_plugins` acts as an allowlist, and an empty list
    /// means "all enabled".
    pub fn is_enabled(&self, name: &str) -> bool {
        if self.disabled_plugins.iter().any(|n| n.as_str() == name) {
            return false;
        }
        self.enabled_plugins.is_empty()
            || self.enabled_plugins.iter().any(|n| n.as_str() == name)
    }

    fn path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".openkite")
            .join("config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("openkite-config-{}", std::process::id()));
        let path = dir.join("config.toml");

        let config = OpenKiteConfig {
            enabled_plugins: vec!["argocd".into()],
            disabled_plugins: vec!["legacy".into()],
        };
        config.save_to(&path).expect("save");

        let loaded = OpenKiteConfig::load_from(&path);
        assert_eq!(loaded.enabled_plugins, config.enabled_plugins);
        assert_eq!(loaded.disabled_plugins, config.disabled_plugins);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_loads_default() {
        let missing = std::env::temp_dir().join(format!("openkite-missing-{}", std::process::id()));
        let _ = std::fs::remove_file(&missing);
        assert_eq!(OpenKiteConfig::load_from(&missing), OpenKiteConfig::default());
    }

    #[test]
    fn is_enabled_semantics() {
        let empty = OpenKiteConfig::default();
        assert!(empty.is_enabled("anything")); // empty allowlist = all enabled

        let allow = OpenKiteConfig {
            enabled_plugins: vec!["argocd".into()],
            ..Default::default()
        };
        assert!(allow.is_enabled("argocd"));
        assert!(!allow.is_enabled("other"));

        let disabled_wins = OpenKiteConfig {
            enabled_plugins: vec!["argocd".into()],
            disabled_plugins: vec!["argocd".into()],
        };
        assert!(!disabled_wins.is_enabled("argocd"));
    }
}
