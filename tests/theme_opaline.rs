//! Integration tests for the opaline theme source (OKT-30).

use openkite::theme::CSS_VARS;
use openkite::theme_opaline::{
    list_opaline_themes, load_and_map, load_opaline, theme_from_opaline,
};

#[test]
fn lists_the_full_builtin_collection() {
    let themes = list_opaline_themes();
    assert!(
        themes.len() >= 39,
        "expected 39+ builtins, got {}",
        themes.len()
    );
    assert!(themes.iter().any(|t| t.id == "catppuccin-mocha"));
    assert!(themes.iter().any(|t| t.id == "rose-pine"));
    for t in themes {
        assert!(
            t.variant == "dark" || t.variant == "light",
            "{}: unexpected variant {}",
            t.id,
            t.variant
        );
    }
}

#[test]
fn loads_builtins_by_kebab_id_and_default_alias() {
    assert!(load_opaline("catppuccin-mocha").is_ok());
    assert!(load_opaline("default").is_ok());
    assert!(load_opaline("silkcircuit-neon").is_ok());
    assert!(load_opaline("no-such-theme").is_err());
}

#[test]
fn mapped_theme_covers_the_full_contract() {
    for id in [
        "catppuccin-mocha",
        "github-light",
        "tokyo-night",
        "one-dark",
    ] {
        let theme = load_and_map(id).unwrap();
        for var in CSS_VARS {
            assert!(
                theme.get(var).is_some() && !theme.get(var).unwrap().is_empty(),
                "{id} missing {var}"
            );
        }
    }
}

#[test]
fn catppuccin_mocha_resolves_known_hexes() {
    let theme = load_and_map("catppuccin-mocha").unwrap();
    assert_eq!(theme.get("--bg-0"), Some("#11111b")); // crust
    assert_eq!(theme.get("--accent"), Some("#cba6f7")); // mauve
    assert_eq!(theme.get("--green"), Some("#a6e3a1"));
    assert_eq!(theme.get("--red"), Some("#f38ba8"));
    assert_eq!(theme.get("--fg-0"), Some("#cdd6f4")); // text
}

#[test]
fn bright_terminal_variants_are_lightened() {
    let theme = load_and_map("catppuccin-mocha").unwrap();
    let red = theme.get("--term-red").unwrap().to_string();
    let bright_red = theme.get("--term-bright-red").unwrap().to_string();
    assert_ne!(red, bright_red, "bright red must differ from red");
    // Lightening toward white strictly increases channel values for mocha's red.
    let r: Vec<u8> = red[1..]
        .chars()
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|c| u8::from_str_radix(&c.iter().collect::<String>(), 16).unwrap())
        .collect();
    let br: Vec<u8> = bright_red[1..]
        .chars()
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|c| u8::from_str_radix(&c.iter().collect::<String>(), 16).unwrap())
        .collect();
    assert!(br.iter().zip(&r).all(|(b, a)| b >= a));
}

#[test]
fn light_themes_produce_light_backgrounds() {
    let dark = load_and_map("catppuccin-mocha").unwrap();
    let light = load_and_map("github-light").unwrap();
    let dark_bg = hex_luminance(dark.get("--bg-0").unwrap());
    let light_bg = hex_luminance(light.get("--bg-0").unwrap());
    assert!(light_bg > dark_bg, "light theme bg must be lighter");
}

fn hex_luminance(hex: &str) -> u32 {
    let v = u32::from_str_radix(&hex[1..], 16).unwrap();
    // Rough perceptual-ish sum of channels.
    (v >> 16 & 0xff) + (v >> 8 & 0xff) + (v & 0xff)
}

#[test]
fn to_css_vars_serializes_opaline_mapping() {
    let theme = theme_from_opaline(&load_opaline("catppuccin-mocha").unwrap());
    let css = theme.to_css_vars();
    assert!(css.contains("--bg-0: #11111b;"));
    assert!(css.contains("--accent: #cba6f7;"));
}
