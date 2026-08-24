//! Local app configuration persisted at `~/.openkite/config.toml`.

use openkite_plugin_sdk::anyhow;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn default_true() -> bool {
    true
}

/// Local OpenKite configuration: plugin enable/disable state plus appearance
/// and metrics settings. New fields are `#[serde(default)]`ed so config files
/// written by older builds keep loading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenKiteConfig {
    /// Explicitly enabled plugin names. Empty = all plugins enabled.
    pub enabled_plugins: Vec<String>,
    /// Explicitly disabled plugin names (wins over `enabled_plugins`).
    pub disabled_plugins: Vec<String>,
    /// Selected theme name (a `theme::builtins` key). `None` = default theme.
    #[serde(default)]
    pub theme: Option<String>,
    /// UI font size in pixels. `None` = default.
    #[serde(default)]
    pub font_size: Option<u16>,
    /// Whether metrics columns render by default.
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,
}

impl Default for OpenKiteConfig {
    fn default() -> Self {
        Self {
            enabled_plugins: Vec::new(),
            disabled_plugins: Vec::new(),
            theme: None,
            font_size: None,
            metrics_enabled: true,
        }
    }
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
    /// Consumed by the settings UI (OKT-19).
    #[allow(dead_code)]
    pub fn save(&self) -> anyhow::Result<()> {
        self.save_to(&Self::path())
    }

    /// Persist to a specific file (testable).
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn is_enabled(&self, name: &str) -> bool {
        if self.disabled_plugins.iter().any(|n| n.as_str() == name) {
            return false;
        }
        self.enabled_plugins.is_empty() || self.enabled_plugins.iter().any(|n| n.as_str() == name)
    }

    fn path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".openkite")
            .join("config.toml")
    }
}
