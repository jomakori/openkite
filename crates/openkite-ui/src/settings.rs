//! Console settings: themes, density, shortcuts and kubeconfig paths.
//!
//! A theme may only override the token names the console already paints with,
//! so every surface keeps reading `var(--…)` and a switch is a repaint, not a
//! reload.

use serde::Deserialize;

/// Every custom property a theme is allowed to set.
pub const TOKENS: [&str; 11] = [
    "bg",
    "surface",
    "surface-solid",
    "fg",
    "muted",
    "subtle",
    "border",
    "accent",
    "success",
    "warn",
    "danger",
];

/// Whether a theme paints light or dark, so the platform scrollbars match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

/// Row height and padding of dense surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Density {
    Comfortable,
    Compact,
}

impl Density {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Compact => "compact",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Comfortable => "Comfortable",
            Self::Compact => "Compact",
        }
    }
}

/// A set of token overrides, plus the name it shows up as in the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub appearance: Appearance,
    tokens: Vec<(String, String)>,
}

impl Theme {
    pub fn tokens(&self) -> &[(String, String)] {
        &self.tokens
    }

    /// The override block as an inline `style`, for themes that are not compiled in.
    pub fn css_vars(&self) -> String {
        self.tokens
            .iter()
            .map(|(name, value)| format!("--{name}:{value}"))
            .collect::<Vec<_>>()
            .join(";")
    }

    fn builtin(id: &str, name: &str, appearance: Appearance, tokens: &[(&str, &str)]) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            appearance,
            tokens: tokens
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        }
    }
}

/// The five themes that ship with the console. `frost` is the base sheet, so it
/// carries no overrides of its own.
pub fn builtin_themes() -> Vec<Theme> {
    vec![
        Theme::builtin("frost", "Frost", Appearance::Light, &[]),
        Theme::builtin(
            "dusk",
            "Dusk",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.185 0.012 262)"),
                ("surface", "oklch(0.255 0.014 262 / 0.82)"),
                ("surface-solid", "oklch(0.255 0.014 262 / 0.97)"),
                ("fg", "oklch(0.94 0.006 262)"),
                ("muted", "oklch(0.76 0.014 262)"),
                ("subtle", "oklch(0.62 0.014 262)"),
                ("border", "oklch(0.37 0.014 262)"),
                ("accent", "oklch(0.7 0.14 258)"),
                ("success", "oklch(0.72 0.13 150)"),
                ("warn", "oklch(0.78 0.13 78)"),
                ("danger", "oklch(0.7 0.17 22)"),
            ],
        ),
        Theme::builtin(
            "midnight",
            "Midnight",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.145 0.014 268)"),
                ("surface", "oklch(0.205 0.016 268 / 0.86)"),
                ("surface-solid", "oklch(0.205 0.016 268 / 0.97)"),
                ("fg", "oklch(0.91 0.008 268)"),
                ("muted", "oklch(0.7 0.016 268)"),
                ("subtle", "oklch(0.55 0.016 268)"),
                ("border", "oklch(0.315 0.02 268)"),
                ("accent", "oklch(0.66 0.16 272)"),
                ("success", "oklch(0.7 0.12 152)"),
                ("warn", "oklch(0.76 0.12 82)"),
                ("danger", "oklch(0.68 0.16 24)"),
            ],
        ),
        Theme::builtin(
            "graphite",
            "Graphite",
            Appearance::Dark,
            &[
                ("bg", "oklch(0.17 0 0)"),
                ("surface", "oklch(0.235 0 0 / 0.88)"),
                ("surface-solid", "oklch(0.235 0 0 / 0.97)"),
                ("fg", "oklch(0.97 0 0)"),
                ("muted", "oklch(0.78 0 0)"),
                ("subtle", "oklch(0.6 0 0)"),
                ("border", "oklch(0.42 0 0)"),
                ("accent", "oklch(0.78 0.02 250)"),
                ("success", "oklch(0.76 0.1 150)"),
                ("warn", "oklch(0.8 0.1 80)"),
                ("danger", "oklch(0.72 0.14 24)"),
            ],
        ),
        Theme::builtin(
            "paper",
            "Paper",
            Appearance::Light,
            &[
                ("bg", "oklch(0.958 0.014 88)"),
                ("surface", "oklch(0.995 0.006 90 / 0.9)"),
                ("surface-solid", "oklch(0.995 0.006 90 / 0.98)"),
                ("fg", "oklch(0.27 0.02 60)"),
                ("muted", "oklch(0.52 0.022 62)"),
                ("subtle", "oklch(0.66 0.018 64)"),
                ("border", "oklch(0.885 0.014 82)"),
                ("accent", "oklch(0.56 0.13 40)"),
                ("success", "oklch(0.6 0.11 148)"),
                ("warn", "oklch(0.68 0.13 70)"),
                ("danger", "oklch(0.6 0.17 26)"),
            ],
        ),
    ]
}

pub fn theme_by_id(id: &str) -> Theme {
    builtin_themes()
        .into_iter()
        .find(|theme| theme.id == id)
        .unwrap_or_else(|| builtin_themes().remove(0))
}

/// A Zed theme file, reduced to the tokens the console can honour.
#[derive(Debug, Deserialize)]
struct ZedFile {
    #[serde(default)]
    themes: Vec<ZedTheme>,
}

#[derive(Debug, Deserialize)]
struct ZedTheme {
    name: String,
    #[serde(default)]
    appearance: Option<String>,
    #[serde(default)]
    style: ZedStyle,
}

#[derive(Debug, Default, Deserialize)]
struct ZedStyle {
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    border: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    surface: Option<String>,
    #[serde(default)]
    element: Option<ZedElement>,
    #[serde(default)]
    accents: Vec<ZedAccent>,
    #[serde(default)]
    players: Vec<ZedAccent>,
}

#[derive(Debug, Default, Deserialize)]
struct ZedElement {
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    border: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ZedAccent {
    #[serde(default)]
    color: Option<String>,
}

/// What went wrong reading a Zed theme file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeError {
    /// The file is not JSON, or not shaped like a Zed theme.
    Malformed(String),
    /// The file parsed but carried no theme to import.
    Empty,
    /// The only theme in the file sets no colour we can map.
    NoTokens,
}

impl ThemeError {
    pub fn message(&self) -> String {
        match self {
            Self::Malformed(why) => format!("Could not read that theme: {why}"),
            Self::Empty => "That file carries no themes".to_string(),
            Self::NoTokens => "That theme sets no colours the console can use".to_string(),
        }
    }
}

/// Reads the first theme of a Zed theme file into the tokens the console paints with.
pub fn parse_zed_theme(json: &str) -> Result<Theme, ThemeError> {
    let file: ZedFile =
        serde_json::from_str(json).map_err(|err| ThemeError::Malformed(err.to_string()))?;
    let theme = file.themes.into_iter().next().ok_or(ThemeError::Empty)?;
    let style = theme.style;
    let mut tokens: Vec<(String, String)> = Vec::new();
    let mut push = |name: &str, value: Option<String>| {
        if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
            tokens.push((name.to_string(), value));
        }
    };
    push("bg", style.background);
    let element = style.element.unwrap_or_default();
    push(
        "surface",
        style.surface.or_else(|| element.background.clone()),
    );
    push("surface-solid", element.background);
    push("border", style.border.or(element.border));
    push("fg", style.text.or(element.text));
    let accent = style
        .accents
        .first()
        .or(style.players.first())
        .and_then(|accent| accent.color.clone());
    push("accent", accent);
    if tokens.is_empty() {
        return Err(ThemeError::NoTokens);
    }
    // A theme read from a file cannot live in the compiled picker, so its id is
    // derived from its name and it is addressed by the inline block instead.
    let id = format!(
        "zed-{}",
        theme
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
    );
    Ok(Theme {
        id,
        name: theme.name,
        appearance: match theme.appearance.as_deref() {
            Some("light") => Appearance::Light,
            _ => Appearance::Dark,
        },
        tokens,
    })
}

/// One row of the shortcut sheet.
pub struct Shortcut {
    pub action: &'static str,
    pub keys: &'static str,
}

/// The bindings the console answers to.
pub const SHORTCUTS: [Shortcut; 6] = [
    Shortcut {
        action: "Search and commands",
        keys: "mod+k",
    },
    Shortcut {
        action: "Toggle the log dock",
        keys: "mod+j",
    },
    Shortcut {
        action: "Next namespace",
        keys: "mod+]",
    },
    Shortcut {
        action: "Refresh the view",
        keys: "mod+r",
    },
    Shortcut {
        action: "Close the overlay",
        keys: "esc",
    },
    Shortcut {
        action: "Move through results",
        keys: "↑ ↓",
    },
];

/// The settings the console holds for this profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub theme: String,
    /// The theme read from a Zed file, if the active theme came from one.
    pub imported: Option<Theme>,
    pub density: Density,
    pub kubeconfig_paths: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "frost".to_string(),
            imported: None,
            density: Density::Comfortable,
            kubeconfig_paths: vec!["~/.kube/config".to_string()],
        }
    }
}

impl Settings {
    pub fn theme(&self) -> Theme {
        match &self.imported {
            Some(theme) if theme.id == self.theme => theme.clone(),
            _ => theme_by_id(&self.theme),
        }
    }

    /// Makes an imported theme the active one.
    pub fn use_theme(&mut self, theme: Theme) {
        self.theme = theme.id.clone();
        self.imported = Some(theme);
    }

    /// Adds a kubeconfig path, ignoring blanks and duplicates.
    pub fn add_kubeconfig(&mut self, path: &str) {
        let path = path.trim();
        if path.is_empty() || self.kubeconfig_paths.iter().any(|known| known == path) {
            return;
        }
        self.kubeconfig_paths.push(path.to_string());
    }

    pub fn remove_kubeconfig(&mut self, path: &str) {
        self.kubeconfig_paths.retain(|known| known != path);
    }

    /// The token block every surface under the theme reads.
    pub fn css_vars(&self) -> String {
        self.theme().css_vars()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_theme_only_sets_known_tokens() {
        for theme in builtin_themes() {
            for (name, _) in theme.tokens() {
                assert!(
                    TOKENS.contains(&name.as_str()),
                    "{} sets unknown token --{name}",
                    theme.id
                );
            }
        }
    }

    #[test]
    fn five_themes_ship_and_the_default_is_frost() {
        let themes = builtin_themes();
        assert_eq!(themes.len(), 5, "{themes:?}");
        assert_eq!(Settings::default().theme, "frost");
        assert!(themes.iter().any(|t| t.appearance == Appearance::Light));
        assert!(themes.iter().any(|t| t.appearance == Appearance::Dark));
    }

    #[test]
    fn an_unknown_theme_falls_back_to_frost() {
        assert_eq!(theme_by_id("nope").id, "frost");
    }

    #[test]
    fn a_zed_theme_maps_its_colours_onto_console_tokens() {
        let json = r##"{
            "name": "Fixture",
            "themes": [{
                "name": "Fixture Dark",
                "appearance": "dark",
                "style": {
                    "background": "#101216",
                    "border": "#2a2f38",
                    "text": "#e6e8ec",
                    "element": { "background": "#161a20" },
                    "accents": [{ "color": "#7aa2f7" }]
                }
            }]
        }"##;
        let theme = parse_zed_theme(json).expect("theme");
        assert_eq!(theme.id, "zed-fixture-dark");
        assert_eq!(theme.appearance, Appearance::Dark);
        let vars = theme.css_vars();
        assert!(vars.contains("--bg:#101216"), "{vars}");
        assert!(vars.contains("--fg:#e6e8ec"), "{vars}");
        assert!(vars.contains("--accent:#7aa2f7"), "{vars}");
        for (name, _) in theme.tokens() {
            assert!(TOKENS.contains(&name.as_str()), "unknown token --{name}");
        }
    }

    #[test]
    fn a_zed_file_without_themes_is_rejected() {
        assert_eq!(parse_zed_theme("{\"themes\":[]}"), Err(ThemeError::Empty));
        assert!(matches!(
            parse_zed_theme("not json"),
            Err(ThemeError::Malformed(_))
        ));
        assert_eq!(
            parse_zed_theme("{\"themes\":[{\"name\":\"Bare\",\"style\":{}}]}"),
            Err(ThemeError::NoTokens)
        );
    }

    #[test]
    fn the_base_theme_carries_no_overrides() {
        assert_eq!(Settings::default().css_vars(), "");
        assert!(!theme_by_id("midnight").css_vars().is_empty());
    }

    #[test]
    fn an_imported_theme_paints_with_its_own_tokens() {
        let json = r##"{"themes":[{"name":"Imported","style":{"background":"#010203"}}]}"##;
        let mut settings = Settings::default();
        settings.use_theme(parse_zed_theme(json).expect("theme"));
        assert_eq!(settings.theme, "zed-imported");
        assert_eq!(settings.css_vars(), "--bg:#010203");
    }

    #[test]
    fn kubeconfig_paths_ignore_blanks_and_duplicates() {
        let mut settings = Settings::default();
        settings.add_kubeconfig("  ");
        settings.add_kubeconfig("~/.kube/config");
        settings.add_kubeconfig("~/kube/prod.yaml");
        assert_eq!(settings.kubeconfig_paths.len(), 2);
        settings.remove_kubeconfig("~/.kube/config");
        assert_eq!(settings.kubeconfig_paths, vec!["~/kube/prod.yaml"]);
    }
}
