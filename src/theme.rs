//! Theme engine: CSS variable contract + opaline-backed resolution.
//!
//! Theming is provided by the [`opaline`] token engine (OKT-30): its 39
//! builtin themes are mapped onto the CSS variable contract below via
//! [`crate::theme_opaline`]. [`Theme`] is the resolved output surface the UI
//! consumes — switching themes is an instant variable swap, no re-render.
//! The glass/frost chrome (blur, translucency, elevation) lives in the design
//! system (`assets/main.css`) on top of these variables.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The OpenKite CSS variable contract, in declaration order.
pub const CSS_VARS: &[&str] = &[
    "--bg-0",
    "--bg-1",
    "--bg-2",
    "--border",
    "--fg-0",
    "--fg-1",
    "--fg-2",
    "--accent",
    "--green",
    "--yellow",
    "--red",
    "--violet",
    "--term-black",
    "--term-red",
    "--term-green",
    "--term-yellow",
    "--term-blue",
    "--term-magenta",
    "--term-cyan",
    "--term-white",
    "--term-bright-black",
    "--term-bright-red",
    "--term-bright-green",
    "--term-bright-yellow",
    "--term-bright-blue",
    "--term-bright-magenta",
    "--term-bright-cyan",
    "--term-bright-white",
];

/// Errors produced while saving or loading a theme.
#[derive(Debug)]
pub enum ThemeError {
    /// The input was not valid JSON (or not a JSON object of strings).
    MalformedJson,
    /// A filesystem error while saving or loading.
    Io(String),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::MalformedJson => write!(f, "theme input is not valid JSON"),
            ThemeError::Io(err) => write!(f, "theme I/O error: {err}"),
        }
    }
}

impl std::error::Error for ThemeError {}

/// A resolved theme: an ordered map of CSS variable name → value.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Theme {
    vars: BTreeMap<String, String>,
}

impl Theme {
    /// An empty theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a CSS variable, returning `self` for chaining.
    pub fn with_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.vars.insert(key.into(), value.into());
        self
    }

    /// Read a CSS variable, if set.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(String::as_str)
    }

    /// All variables, ordered by name.
    pub fn vars(&self) -> &BTreeMap<String, String> {
        &self.vars
    }

    /// Serialize to a `--var: value;` block, one declaration per line.
    pub fn to_css_vars(&self) -> String {
        self.vars
            .iter()
            .map(|(key, value)| format!("{key}: {value};"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Serialize to JSON and write to `path`.
    pub fn save(&self, path: &Path) -> Result<(), ThemeError> {
        let json = serde_json::to_string_pretty(self).map_err(|e| ThemeError::Io(e.to_string()))?;
        fs::write(path, json).map_err(|e| ThemeError::Io(e.to_string()))
    }

    /// Load a theme from JSON at `path`.
    pub fn load(path: &Path) -> Result<Self, ThemeError> {
        let json = fs::read_to_string(path).map_err(|e| ThemeError::Io(e.to_string()))?;
        serde_json::from_str(&json).map_err(|_| ThemeError::MalformedJson)
    }
}

/// Resolve the active theme from an opaline theme id (kebab-case, e.g.
/// `"catppuccin-mocha"`). Unknown ids and `None` fall back to the default —
/// the theme picker is responsible for offering valid ids.
pub fn resolve(name: Option<&str>) -> Theme {
    match name {
        Some(id) => crate::theme_opaline::load_and_map(id).unwrap_or_else(|_| default_theme()),
        None => default_theme(),
    }
}

/// The default theme: opaline's `"default"` alias (SilkCircuit Neon).
pub fn default_theme() -> Theme {
    crate::theme_opaline::load_and_map("default").expect("opaline default theme is a builtin")
}
