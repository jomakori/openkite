//! Integration tests for the theme engine.

use openkite::theme::{builtins, gpui_dark, import_zed, resolve, Theme, CSS_VARS};

#[test]
fn builtins_include_five_themes() {
    assert_eq!(builtins().len(), 5);
}

#[test]
fn every_builtin_covers_the_full_contract() {
    for (name, theme) in builtins() {
        for var in CSS_VARS {
            assert!(theme.get(var).is_some(), "{name} missing {var}");
        }
    }
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
fn import_zed_maps_flat_keys() {
    let json = r##"{"background": "#1e1e1e", "foreground": "#f5f5f0", "accent": "#0070f3"}"##;
    let theme = import_zed(json).unwrap();
    assert_eq!(theme.get("--bg-0"), Some("#1e1e1e"));
    assert_eq!(theme.get("--fg-0"), Some("#f5f5f0"));
    assert_eq!(theme.get("--accent"), Some("#0070f3"));
}

#[test]
fn import_zed_maps_nested_terminal_ansi() {
    let json = r##"{"terminal": {"ansi": {"black": "#000000", "white": "#ffffff"}}}"##;
    let theme = import_zed(json).unwrap();
    assert_eq!(theme.get("--term-black"), Some("#000000"));
    assert_eq!(theme.get("--term-white"), Some("#ffffff"));
}

#[test]
fn import_zed_rejects_unknown_key() {
    assert!(import_zed(r##"{"background": "#1e1e1e", "not_a_real_key": "#fff"}"##).is_err());
}

#[test]
fn import_zed_rejects_malformed_json() {
    assert!(import_zed("{ not json").is_err());
    assert!(import_zed("[1, 2, 3]").is_err());
}

#[test]
fn theme_roundtrips_through_json() {
    let theme = builtins().into_iter().next().unwrap().1;
    let path =
        std::env::temp_dir().join(format!("openkite-theme-test-{}.json", std::process::id()));
    theme.save(&path).unwrap();
    let loaded = Theme::load(&path).unwrap();
    assert_eq!(theme, loaded);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn resolve_returns_named_theme() {
    let t = resolve(Some("Tokyo Night"));
    assert_eq!(t.get("--bg-0"), Some("#1a1b26"));
}

#[test]
fn resolve_unknown_name_falls_back_to_default() {
    assert_eq!(resolve(Some("nope")), gpui_dark());
}

#[test]
fn resolve_none_uses_default() {
    assert_eq!(resolve(None), gpui_dark());
}
