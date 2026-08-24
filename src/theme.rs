//! Theme engine: CSS variable contract, built-in palettes, and Zed import.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// Errors produced while importing or loading a theme.
#[derive(Debug)]
pub enum ThemeError {
    /// The input was not valid JSON (or not a JSON object of strings).
    MalformedJson,
    /// The input contained a Zed key with no OpenKite mapping.
    UnknownKey(String),
    /// A filesystem error while saving or loading.
    Io(String),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::MalformedJson => write!(f, "theme input is not valid JSON"),
            ThemeError::UnknownKey(key) => write!(f, "unknown Zed theme key: {key}"),
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

/// Build a [`Theme`] from a list of `key => value` pairs.
macro_rules! theme {
    ($($key:expr => $value:expr),* $(,)?) => {{
        let mut theme = Theme::new();
        $( theme = theme.with_var($key, $value); )*
        theme
    }};
}

/// A standard 16-color xterm palette (One Dark-inspired).
const TERM_PALETTE: [(&str, &str); 16] = [
    ("--term-black", "#282c34"),
    ("--term-red", "#e06c75"),
    ("--term-green", "#98c379"),
    ("--term-yellow", "#e5c07b"),
    ("--term-blue", "#61afef"),
    ("--term-magenta", "#c678dd"),
    ("--term-cyan", "#56b6c2"),
    ("--term-white", "#abb2bf"),
    ("--term-bright-black", "#5c6370"),
    ("--term-bright-red", "#e06c75"),
    ("--term-bright-green", "#98c379"),
    ("--term-bright-yellow", "#e5c07b"),
    ("--term-bright-blue", "#61afef"),
    ("--term-bright-magenta", "#c678dd"),
    ("--term-bright-cyan", "#56b6c2"),
    ("--term-bright-white", "#ffffff"),
];

/// Apply the shared terminal palette to a theme.
fn with_term(mut theme: Theme) -> Theme {
    for (key, value) in TERM_PALETTE {
        theme = theme.with_var(key, value);
    }
    theme
}

/// GPUI Light — a bright, low-contrast theme.
pub fn gpui_light() -> Theme {
    with_term(theme! {
        "--bg-0" => "#f5f7fa",
        "--bg-1" => "#ffffff",
        "--bg-2" => "#f3f4f6",
        "--border" => "#e2e4e9",
        "--fg-0" => "#1e2024",
        "--fg-1" => "#383a42",
        "--fg-2" => "#6b7280",
        "--accent" => "#4d8ce8",
        "--green" => "#4d9a5e",
        "--yellow" => "#c4841d",
        "--red" => "#e05252",
        "--violet" => "#8b5cf6",
    })
}

/// GPUI Dark — a neutral dark theme.
pub fn gpui_dark() -> Theme {
    with_term(theme! {
        "--bg-0" => "#1e2024",
        "--bg-1" => "#25272c",
        "--bg-2" => "#2d3036",
        "--border" => "#3a3d44",
        "--fg-0" => "#f5f7fa",
        "--fg-1" => "#c8ccd4",
        "--fg-2" => "#8b909a",
        "--accent" => "#5b9bf0",
        "--green" => "#4d9a5e",
        "--yellow" => "#d4a017",
        "--red" => "#e05252",
        "--violet" => "#9d7bf6",
    })
}

/// Catppuccin Mocha — a warm pastel dark theme.
pub fn catppuccin_mocha() -> Theme {
    with_term(theme! {
        "--bg-0" => "#1e1e2e",
        "--bg-1" => "#181825",
        "--bg-2" => "#313244",
        "--border" => "#45475a",
        "--fg-0" => "#cdd6f4",
        "--fg-1" => "#a6adc8",
        "--fg-2" => "#6c7086",
        "--accent" => "#89b4fa",
        "--green" => "#a6e3a1",
        "--yellow" => "#f9e2af",
        "--red" => "#f38ba8",
        "--violet" => "#cba6f7",
    })
}

/// Tokyo Night — a cool blue-dark theme.
pub fn tokyo_night() -> Theme {
    with_term(theme! {
        "--bg-0" => "#1a1b26",
        "--bg-1" => "#16161e",
        "--bg-2" => "#24283b",
        "--border" => "#414868",
        "--fg-0" => "#c0caf5",
        "--fg-1" => "#a9b1d6",
        "--fg-2" => "#565f89",
        "--accent" => "#7aa2f7",
        "--green" => "#9ece6a",
        "--yellow" => "#e0af68",
        "--red" => "#f7768e",
        "--violet" => "#bb9af7",
    })
}

/// Rosé Pine — a muted rose-tinted dark theme.
pub fn rose_pine() -> Theme {
    with_term(theme! {
        "--bg-0" => "#191724",
        "--bg-1" => "#1f1d2e",
        "--bg-2" => "#26233a",
        "--border" => "#403d52",
        "--fg-0" => "#e0def4",
        "--fg-1" => "#908caa",
        "--fg-2" => "#6e6a86",
        "--accent" => "#c4a7e7",
        "--green" => "#31748f",
        "--yellow" => "#f6c177",
        "--red" => "#eb6f92",
        "--violet" => "#c4a7e7",
    })
}

/// All built-in themes, ordered by display name.
pub fn builtins() -> Vec<(&'static str, Theme)> {
    vec![
        ("GPUI Light", gpui_light()),
        ("GPUI Dark", gpui_dark()),
        ("Catppuccin Mocha", catppuccin_mocha()),
        ("Tokyo Night", tokyo_night()),
        ("Rosé Pine", rose_pine()),
    ]
}

/// Resolve a theme by display name, falling back to the default (GPUI Dark).
pub fn resolve(name: Option<&str>) -> Theme {
    match name {
        Some(name) => builtins()
            .into_iter()
            .find(|(display, _)| *display == name)
            .map(|(_, theme)| theme)
            .unwrap_or_else(gpui_dark),
        None => gpui_dark(),
    }
}

/// Map a flattened Zed theme key to its OpenKite CSS variable.
fn zed_key(key: &str) -> Option<&'static str> {
    Some(match key {
        "background" => "--bg-0",
        "foreground" => "--fg-0",
        "border" => "--border",
        "accent" => "--accent",
        "terminal.ansi.black" => "--term-black",
        "terminal.ansi.red" => "--term-red",
        "terminal.ansi.green" => "--term-green",
        "terminal.ansi.yellow" => "--term-yellow",
        "terminal.ansi.blue" => "--term-blue",
        "terminal.ansi.magenta" => "--term-magenta",
        "terminal.ansi.cyan" => "--term-cyan",
        "terminal.ansi.white" => "--term-white",
        "terminal.ansi.bright_black" => "--term-bright-black",
        "terminal.ansi.bright_red" => "--term-bright-red",
        "terminal.ansi.bright_green" => "--term-bright-green",
        "terminal.ansi.bright_yellow" => "--term-bright-yellow",
        "terminal.ansi.bright_blue" => "--term-bright-blue",
        "terminal.ansi.bright_magenta" => "--term-bright-magenta",
        "terminal.ansi.bright_cyan" => "--term-bright-cyan",
        "terminal.ansi.bright_white" => "--term-bright-white",
        _ => return None,
    })
}

/// Recursively flatten a JSON object into dotted `key → value` pairs.
fn flatten(
    prefix: &str,
    obj: &serde_json::Map<String, Value>,
    out: &mut Vec<(String, String)>,
) -> Result<(), ThemeError> {
    for (key, value) in obj {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            Value::String(text) => out.push((path, text.clone())),
            Value::Object(child) => flatten(&path, child, out)?,
            _ => return Err(ThemeError::MalformedJson),
        }
    }
    Ok(())
}

/// Import a Zed theme JSON document into an OpenKite [`Theme`].
///
/// Accepts a JSON object (flat or nested, e.g. `terminal.ansi.black`) and
/// rejects any key without a known mapping so typos fail loudly rather than
/// silently producing a broken theme.
pub fn import_zed(json: &str) -> Result<Theme, ThemeError> {
    let value: Value = serde_json::from_str(json).map_err(|_| ThemeError::MalformedJson)?;
    let obj = value.as_object().ok_or(ThemeError::MalformedJson)?;

    let mut flat = Vec::new();
    flatten("", obj, &mut flat)?;

    let mut theme = Theme::new();
    for (key, text) in flat {
        let Some(css) = zed_key(&key) else {
            return Err(ThemeError::UnknownKey(key));
        };
        theme = theme.with_var(css, text);
    }
    Ok(theme)
}
