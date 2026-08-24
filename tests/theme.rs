//! Integration tests for the opaline-backed theme engine.

use openkite::theme::{resolve, Theme, CSS_VARS};

#[test]
fn default_theme_covers_the_full_contract() {
    let theme = resolve(None);
    for var in CSS_VARS {
        assert!(
            theme.get(var).is_some() && !theme.get(var).unwrap().is_empty(),
            "default missing {var}"
        );
    }
}

#[test]
fn resolve_named_opaline_theme() {
    let theme = resolve(Some("catppuccin-mocha"));
    assert_eq!(theme.get("--bg-0"), Some("#11111b"));
    assert_eq!(theme.get("--accent"), Some("#cba6f7"));
}

#[test]
fn resolve_unknown_name_falls_back_to_default() {
    let unknown = resolve(Some("not-a-theme"));
    let default = resolve(None);
    assert_eq!(unknown.vars(), default.vars());
}

#[test]
fn resolve_none_uses_the_default_alias() {
    let none = resolve(None);
    let alias = resolve(Some("default"));
    assert_eq!(none.vars(), alias.vars());
}

#[test]
fn to_css_vars_serializes_declarations() {
    let theme = Theme::new()
        .with_var("--bg-0", "#000000")
        .with_var("--fg-0", "#ffffff");
    let css = theme.to_css_vars();
    assert!(css.contains("--bg-0: #000000;"));
    assert!(css.contains("--fg-0: #ffffff;"));
}

#[test]
fn theme_roundtrips_through_json() {
    let theme = resolve(Some("tokyo-night"));
    let path =
        std::env::temp_dir().join(format!("openkite-theme-test-{}.json", std::process::id()));
    theme.save(&path).unwrap();
    let loaded = Theme::load(&path).unwrap();
    assert_eq!(theme, loaded);
    let _ = std::fs::remove_file(&path);
}
