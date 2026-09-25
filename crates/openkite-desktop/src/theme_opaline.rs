//! Opaline theme source.
//!
//! Adopts the [`opaline`](https://crates.io/crates/opaline) token-based theme
//! engine as a theme source: its 39 builtin themes (Catppuccin, Rose Pine,
//! Tokyo Night, Nord, …) are mapped onto the OpenKite CSS variable contract
//! ([`crate::theme::CSS_VARS`]). The glass/frost chrome (blur, translucency,
//! elevation) lives in the design system on top of these tokens — opaline
//! supplies the colors, OpenKite supplies the surface treatment.
//!
//! Mapping strategy: each OpenKite var resolves from an opaline semantic
//! token (`bg.base`, `text.primary`, `accent.primary`, `success`, …), falling
//! back to a palette entry, then to a neutral default. Opaline themes do not
//! carry ANSI brights, so `--term-bright-*` is derived by lightening the base
//! hue. See [`theme_from_opaline`].

use crate::theme::{Theme, CSS_VARS};

/// OpenKite var → (opaline token, palette fallback, neutral default).
const MAPPING: &[(&str, &str, &str, &str)] = &[
    ("--bg-0", "bg.base", "base", "#f5f7fa"),
    ("--bg-1", "bg.panel", "panel", "#ffffff"),
    ("--bg-2", "bg.elevated", "surface1", "#f3f4f6"),
    ("--border", "border.unfocused", "overlay0", "#e2e4e9"),
    ("--fg-0", "text.primary", "text", "#1e2024"),
    ("--fg-1", "text.secondary", "subtext1", "#383a42"),
    ("--fg-2", "text.muted", "subtext0", "#6b7280"),
    ("--accent", "accent.primary", "blue", "#4d8ce8"),
    ("--green", "success", "green", "#4d9a5e"),
    ("--yellow", "warning", "yellow", "#c4841d"),
    ("--red", "error", "red", "#e05252"),
    ("--violet", "accent.tertiary", "mauve", "#8b5cf6"),
    // Terminal normal (ANSI hues from palette).
    ("--term-black", "bg.base", "black", "#1e2024"),
    ("--term-red", "error", "red", "#e05252"),
    ("--term-green", "success", "green", "#4d9a5e"),
    ("--term-yellow", "warning", "yellow", "#c4841d"),
    ("--term-blue", "accent.secondary", "blue", "#4d8ce8"),
    ("--term-magenta", "accent.primary", "mauve", "#8b5cf6"),
    ("--term-cyan", "info", "teal", "#2dd4bf"),
    ("--term-white", "text.primary", "text", "#abb2bf"),
];

/// Resolve one var through token → palette → default.
fn resolve(theme: &opaline::Theme, var: &str) -> String {
    let (_, token, fallback, default) = MAPPING
        .iter()
        .find(|row| row.0 == var)
        .expect("mapping covers every non-bright var");
    theme
        .try_color(token)
        .or_else(|| theme.try_color(fallback))
        .map(|c| c.to_hex())
        .unwrap_or_else(|| (*default).to_string())
}

/// Bright terminal variant: lighten the base hue toward white.
fn bright(theme: &opaline::Theme, var: &str) -> String {
    let base_var = var.replacen("--term-bright-", "--term-", 1);
    let (_, token, fallback, default) = MAPPING
        .iter()
        .find(|row| row.0 == base_var)
        .expect("bright var maps from its normal counterpart");
    theme
        .try_color(token)
        .or_else(|| theme.try_color(fallback))
        .map(|c| c.lighten(0.25).to_hex())
        .unwrap_or_else(|| (*default).to_string())
}

/// Build an OpenKite [`Theme`] from an opaline theme, covering the full
/// `CSS_VARS` contract (all 28 vars, including derived terminal brights).
pub fn theme_from_opaline(theme: &opaline::Theme) -> Theme {
    let mut out = Theme::new();
    for var in CSS_VARS {
        let value = if var.starts_with("--term-bright-") {
            bright(theme, var)
        } else {
            resolve(theme, var)
        };
        out = out.with_var(*var, value);
    }
    out
}

/// A listable opaline theme: `(kebab id, display name, variant)`.
pub struct OpalineThemeInfo {
    pub id: String,
    pub display_name: String,
    pub variant: String,
}

/// List available opaline themes (builtins; user themes once discovery is on).
pub fn list_opaline_themes() -> Vec<OpalineThemeInfo> {
    opaline::list_available_themes()
        .into_iter()
        .map(|info| {
            let variant = match info.variant {
                opaline::ThemeVariant::Dark => "dark",
                _ => "light",
            };
            OpalineThemeInfo {
                id: info.name,
                display_name: info.display_name,
                variant: variant.into(),
            }
        })
        .collect()
}

/// Load an opaline builtin by kebab-case id (e.g. `"catppuccin-mocha"`;
/// `"default"` aliases `"silkcircuit-neon"`).
pub fn load_opaline(name: &str) -> Result<opaline::Theme, String> {
    opaline::load_by_name(name).ok_or_else(|| format!("unknown opaline theme: {name}"))
}

/// Convenience: load an opaline theme by id and map it onto our contract.
pub fn load_and_map(name: &str) -> Result<Theme, String> {
    load_opaline(name).map(|t| theme_from_opaline(&t))
}
