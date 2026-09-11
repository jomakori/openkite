//! Local app configuration persisted at `~/.openkite/config.toml`.

use openkite_plugin_sdk::anyhow;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn default_true() -> bool {
    true
}

/// Whether the OS window menu bar is shown (OKT-99).
///
/// Persisted in the same `OpenKiteConfig` store under the `menuBar` key
/// (camelCase, matching the setting's UI name) as `"show"` or `"hide"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MenuBarVisibility {
    /// Show the platform menu bar (the default).
    #[default]
    Show,
    /// Hide the menu bar where the platform supports it (Linux/Windows).
    Hide,
}

impl MenuBarVisibility {
    /// The other visibility — what a palette toggle flips to.
    pub fn toggled(self) -> Self {
        match self {
            Self::Show => Self::Hide,
            Self::Hide => Self::Show,
        }
    }

    /// Whether the menu bar should be visible.
    pub fn is_visible(self) -> bool {
        matches!(self, Self::Show)
    }
}

/// Which OS window decoration (title bar / chrome) theme to request (OKT-100).
///
/// Persisted in the same `OpenKiteConfig` store under the `titleBarTheme` key
/// (camelCase, matching the setting's UI name) as `"system"`, `"light"`, or
/// `"dark"`. tao's window `Theme` has no "system" variant, so this is a
/// tri-state: [`Self::System`] maps to `None` (follow the OS) and the other
/// two force the decoration theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TitleBarTheme {
    /// Follow the OS decoration theme (the default).
    #[default]
    System,
    /// Force the light decoration theme.
    Light,
    /// Force the dark decoration theme.
    Dark,
}

impl TitleBarTheme {
    /// The lowercase persisted value (`titleBarTheme = "system" | ...`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// Title-cased palette label for the choice.
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
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
    /// Selected theme name (an opaline theme id, kebab-case). `None` = default theme.
    #[serde(default)]
    pub theme: Option<String>,
    /// UI font size in pixels. `None` = default.
    #[serde(default)]
    pub font_size: Option<u16>,
    /// Whether metrics columns render by default.
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,
    /// Whether the OS window menu bar is shown (OKT-99). Persisted as
    /// `menuBar = "show" | "hide"`; missing in older files → `show`.
    #[serde(default, rename = "menuBar")]
    pub menu_bar: MenuBarVisibility,
    /// OS window decoration (title bar) theme override (OKT-100). Persisted
    /// as `titleBarTheme = "system" | "light" | "dark"`; missing in older
    /// files → `system` (follow the OS). Independent of the in-page app theme
    /// (`theme`).
    #[serde(default, rename = "titleBarTheme")]
    pub title_bar_theme: TitleBarTheme,
}

impl Default for OpenKiteConfig {
    fn default() -> Self {
        Self {
            enabled_plugins: Vec::new(),
            disabled_plugins: Vec::new(),
            theme: None,
            font_size: None,
            metrics_enabled: true,
            menu_bar: MenuBarVisibility::Show,
            title_bar_theme: TitleBarTheme::System,
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
    /// Consumed by the settings UI.
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
