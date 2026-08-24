//! Theme catalog (OKT-42): curated defaults + the full opaline store.
//!
//! The picker (and future command palette) lists the catalog: the 5 curated
//! defaults the app starts with, followed by the remaining opaline builtins
//! as the store (39 themes total). `swatches` gives picker rows a live
//! preview — omarchy-style instant switching — by resolving each theme's key
//! tokens through the standard opaline mapping.

use crate::theme_opaline::{list_opaline_themes, load_and_map};

/// The 5 curated defaults (the original defaults, mapped to opaline ids).
pub const DEFAULT_THEME_IDS: &[&str] = &[
    "one-light", // GPUI Light family
    "one-dark",  // GPUI Dark family
    "catppuccin-mocha",
    "tokyo-night",
    "rose-pine",
];

/// One entry in the theme catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeEntry {
    pub id: String,
    pub display_name: String,
    pub variant: String,
    /// Curated default (sectioned first) vs store.
    pub is_default: bool,
}

/// Preview swatches for a theme row: `(--bg-0, --fg-0, --accent)` hexes.
pub fn swatches(id: &str) -> (String, String, String) {
    match load_and_map(id) {
        Ok(theme) => (
            theme.get("--bg-0").unwrap_or("#888888").to_string(),
            theme.get("--fg-0").unwrap_or("#888888").to_string(),
            theme.get("--accent").unwrap_or("#888888").to_string(),
        ),
        Err(_) => ("#888888".into(), "#888888".into(), "#888888".into()),
    }
}

/// True when `entry_id` is the currently selected theme. The boot default is
/// opaline's `"default"` alias (SilkCircuit Neon), which is not a catalog id —
/// so `"default"` matches `"silkcircuit-neon"`.
pub fn matches_current(entry_id: &str, current: &str) -> bool {
    entry_id == current || (current == "default" && entry_id == "silkcircuit-neon")
}

/// The boot default id (theme.rs `resolve(None)` → SilkCircuit Neon).
pub fn default_id() -> &'static str {
    "default"
}

fn default_position(entry: &ThemeEntry) -> usize {
    DEFAULT_THEME_IDS
        .iter()
        .position(|id| *id == entry.id)
        .unwrap_or(usize::MAX)
}

/// The full catalog: the 5 curated defaults (in declared order) first, then
/// the remaining store entries, each loadable through the opaline engine.
pub fn catalog() -> Vec<ThemeEntry> {
    let mut defaults = Vec::new();
    let mut store = Vec::new();
    for info in list_opaline_themes() {
        let is_default = DEFAULT_THEME_IDS.contains(&info.id.as_str());
        let entry = ThemeEntry {
            id: info.id,
            display_name: info.display_name,
            variant: info.variant,
            is_default,
        };
        if is_default {
            defaults.push(entry);
        } else {
            store.push(entry);
        }
    }
    defaults.sort_by_key(default_position);
    defaults.extend(store);
    defaults
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::CSS_VARS;

    #[test]
    fn catalog_returns_39_entries_with_5_curated_defaults_first() {
        let catalog = catalog();
        assert_eq!(catalog.len(), 39);
        let defaults: Vec<&ThemeEntry> = catalog.iter().filter(|e| e.is_default).collect();
        assert_eq!(defaults.len(), 5);
        // Defaults are the first 5 entries, in declared order.
        for (i, id) in DEFAULT_THEME_IDS.iter().enumerate() {
            assert_eq!(catalog[i].id, *id);
        }
    }

    #[test]
    fn every_default_theme_loads_and_covers_the_contract() {
        for id in DEFAULT_THEME_IDS {
            let theme = load_and_map(id).unwrap_or_else(|_| panic!("{id} must load"));
            for var in CSS_VARS {
                assert!(
                    theme.get(var).is_some() && !theme.get(var).unwrap().is_empty(),
                    "{id} missing {var}"
                );
            }
        }
    }

    #[test]
    fn every_store_entry_loads() {
        for entry in catalog() {
            assert!(
                load_and_map(&entry.id).is_ok(),
                "store entry {} failed to load",
                entry.id
            );
        }
    }

    #[test]
    fn swatches_return_hex_or_fallback() {
        let (bg, fg, accent) = swatches("catppuccin-mocha");
        assert!(bg.starts_with('#') && fg.starts_with('#') && accent.starts_with('#'));
        assert_eq!(bg, "#11111b");
        assert_eq!(accent, "#cba6f7");
        let (fb, _, _) = swatches("no-such-theme");
        assert_eq!(fb, "#888888");
    }

    #[test]
    fn current_matching_aliases_boot_default() {
        assert!(matches_current("silkcircuit-neon", "default"));
        assert!(matches_current("catppuccin-mocha", "catppuccin-mocha"));
        assert!(!matches_current("catppuccin-mocha", "default"));
    }

    #[test]
    fn catalog_ids_are_unique() {
        let ids: std::collections::HashSet<&str> =
            catalog().iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids.len(), 39);
    }
}
