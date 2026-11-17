//! The settings sheet: it renders the picker, applies density, and a switch
//! changes the tokens the surfaces below it paint with — without a reload.

use dioxus::prelude::*;
use openkite_ui::settings::{
    builtin_themes, parse_zed_theme, Appearance, Density, Preference, Settings,
};
use openkite_ui::settings_view::{SettingsSheet, ThemeScope};

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    settings: Settings,
    system: Appearance,
    open: bool,
    import_text: String,
    import_error: Option<String>,
}

#[component]
fn Harness(props: HarnessProps) -> Element {
    rsx! {
        ThemeScope { settings: props.settings.clone(), system: props.system,
            SettingsSheet {
                open: props.open,
                settings: props.settings.clone(),
                system: props.system,
                themes: builtin_themes(),
                import_text: props.import_text.clone(),
                import_error: props.import_error.clone(),
                on_theme: move |_id: String| {},
                on_preference: move |_preference: Preference| {},
                on_density: move |_density: Density| {},
                on_import_text: move |_text: String| {},
                on_import: move |_| {},
                on_add_kubeconfig: move |_path: String| {},
                on_remove_kubeconfig: move |_path: String| {},
                on_close: move |_| {},
            }
        }
    }
}

fn render(settings: Settings, open: bool) -> String {
    render_with(settings, Appearance::Dark, open)
}

fn render_with(settings: Settings, system: Appearance, open: bool) -> String {
    let props = HarnessProps {
        settings,
        system,
        open,
        import_text: String::new(),
        import_error: None,
    };
    let mut dom = VirtualDom::new_with_props(Harness, props);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn a_closed_sheet_renders_nothing() {
    let html = render(Settings::default(), false);
    assert!(!html.contains("pane-body"), "{html}");
    assert!(!html.contains("Settings"), "{html}");
}

#[test]
fn the_sheet_is_a_pane_on_the_right_edge() {
    let html = render(Settings::default(), true);
    assert!(html.contains(r#"class="set-pane""#), "{html}");
    assert!(html.contains(r#"role="dialog""#), "{html}");
    assert!(html.contains(r#"aria-modal="true""#), "{html}");
    assert!(html.contains(r#"aria-label="Settings""#), "{html}");
    assert!(html.contains("pane-head"), "{html}");
    assert!(html.contains("pane-body"), "{html}");
    assert!(html.contains("pane-foot"), "{html}");
    // The centred modal chrome is gone with it.
    assert!(!html.contains("dlg wide modal"), "{html}");
    assert!(!html.contains("modal-actions"), "{html}");
}

#[test]
fn all_twenty_four_themes_are_offered_in_one_list() {
    let html = render(Settings::default(), true);
    for id in [
        "frost-dark",
        "catppuccin-mocha",
        "one-dark",
        "dracula",
        "nord-dark",
        "gruvbox-dark",
        "tokyo-night",
        "solarized-dark",
        "monokai",
        "ayu-dark",
        "kanagawa-wave",
        "rose-pine",
    ] {
        assert!(
            html.contains(&format!(r#"data-theme-id="{id}""#)),
            "{id}: {html}"
        );
    }
    for id in [
        "frost",
        "catppuccin-latte",
        "one-light",
        "solarized-light",
        "gruvbox-light",
        "github-light",
        "ayu-light",
        "rose-pine-dawn",
        "kanagawa-lotus",
        "everforest-light",
        "quiet-light",
        "xcode-light",
    ] {
        assert!(
            html.contains(&format!(r#"data-theme-id="{id}""#)),
            "{id}: {html}"
        );
    }
    assert_eq!(html.matches("theme-row").count(), 24, "{html}");
    assert!(!html.contains("theme-group"), "one flat list, no groups");
    // Appearance, one theme, one density.
    assert_eq!(html.matches(r#"aria-pressed="true""#).count(), 3, "{html}");
    assert!(html.contains(r#"class="theme-row on""#), "{html}");
}

#[test]
fn the_appearance_switch_offers_system_light_and_dark() {
    let html = render(Settings::default(), true);
    for option in ["system", "light", "dark"] {
        assert!(
            html.contains(&format!(r#"data-preference="{option}""#)),
            "{option}: {html}"
        );
    }
    assert!(
        html.contains(r#"data-preference="system" class="seg on""#)
            || html.contains(r#"class="seg on" data-preference="system""#),
        "{html}"
    );
}

#[test]
fn the_default_follows_the_system_and_paints_that_half_of_the_pair() {
    let dark = render_with(Settings::default(), Appearance::Dark, true);
    assert!(dark.contains(r#"data-theme="frost-dark""#), "{dark}");
    assert!(dark.contains(r#"data-appearance="dark""#), "{dark}");
    assert!(dark.contains("--bg:oklch(0.173 0.009 264)"), "{dark}");

    let light = render_with(Settings::default(), Appearance::Light, true);
    assert!(light.contains(r#"data-theme="frost""#), "{light}");
    assert!(light.contains(r#"data-appearance="light""#), "{light}");
    assert!(light.contains("--bg:oklch(0.973 0.006 265)"), "{light}");
}

#[test]
fn pinning_an_appearance_overrides_the_system_report() {
    let settings = Settings {
        preference: Preference::Light,
        ..Settings::default()
    };
    let html = render_with(settings, Appearance::Dark, true);
    assert!(html.contains(r#"data-theme="frost""#), "{html}");
    assert!(!html.contains(r#"data-theme="frost-dark""#), "{html}");
}

#[test]
fn switching_theme_changes_the_tokens_without_a_reload() {
    let mut settings = Settings::default();
    settings.set_theme("catppuccin-mocha");
    let html = render(settings, true);
    assert!(html.contains(r#"data-theme="catppuccin-mocha""#), "{html}");
    assert!(html.contains("--bg:oklch(0.243 0.030 284)"), "{html}");
    assert!(html.contains("--accent:oklch(0.766 0.111 260)"), "{html}");
}

#[test]
fn an_imported_theme_paints_its_own_colours() {
    let json =
        r##"{"themes":[{"name":"Imported","style":{"background":"#010203","text":"#fefefe"}}]}"##;
    let mut settings = Settings::default();
    settings.use_theme(parse_zed_theme(json).expect("theme"));
    let html = render(settings, true);
    assert!(html.contains(r#"data-theme="zed-imported""#), "{html}");
    assert!(html.contains("--bg:#010203"), "{html}");
    assert!(html.contains("--fg:#fefefe"), "{html}");
}

#[test]
fn compact_density_is_carried_on_the_scope() {
    let settings = Settings {
        density: Density::Compact,
        ..Settings::default()
    };
    let html = render(settings, true);
    assert!(html.contains(r#"data-density="compact""#), "{html}");
}

#[test]
fn shortcuts_are_listed_with_their_keys() {
    let html = render(Settings::default(), true);
    for action in [
        "Search and commands",
        "Toggle the log dock",
        "Refresh the view",
        "Close the overlay",
    ] {
        assert!(html.contains(action), "{action}: {html}");
    }
    assert!(html.contains("<kbd>mod+k</kbd>"), "{html}");
    assert!(html.contains("<kbd>esc</kbd>"), "{html}");
}

#[test]
fn kubeconfig_paths_are_listed_and_removable() {
    let settings = Settings {
        kubeconfig_paths: vec!["~/.kube/config".to_string(), "~/kube/prod.yaml".to_string()],
        ..Settings::default()
    };
    let html = render(settings, true);
    assert!(html.contains("~/.kube/config"), "{html}");
    assert!(html.contains("~/kube/prod.yaml"), "{html}");
    assert!(html.contains(r#"data-remove="~/kube/prod.yaml""#), "{html}");
    assert!(
        html.contains(r#"aria-label="Remove ~/kube/prod.yaml""#),
        "{html}"
    );
    assert!(
        html.contains(r#"placeholder="~/.kube/prod.yaml""#),
        "{html}"
    );
}

#[test]
fn the_import_field_and_error_are_rendered() {
    let props = HarnessProps {
        settings: Settings::default(),
        system: Appearance::Dark,
        open: true,
        import_text: "{\"themes\":[]}".to_string(),
        import_error: Some("That file carries no themes".to_string()),
    };
    let mut dom = VirtualDom::new_with_props(Harness, props);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    assert!(html.contains(r#"class="input mono set-import""#), "{html}");
    assert!(html.contains("Paste a Zed theme JSON file"), "{html}");
    assert!(html.contains("<h3>Zed theme</h3>"), "{html}");
    assert!(html.contains(r#"role="alert""#), "{html}");
    assert!(html.contains("That file carries no themes"), "{html}");
}

#[test]
fn every_row_previews_its_own_four_colours() {
    let html = render(Settings::default(), true);
    // Four dots per row: canvas, raised surface, ink, accent.
    assert_eq!(html.matches("class=\"theme-dot\"").count(), 96, "{html}");
    // Tokyo Night's row paints from its own tokens, not the active theme's.
    assert!(html.contains("background:oklch(0.226 0.021 280)"), "{html}");
    // The slotted presets say which slot they fill; the rest leave it blank.
    assert!(
        html.contains(r#"<span class="theme-slot">D</span>"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<span class="theme-slot">L</span>"#),
        "{html}"
    );
}

#[test]
fn pinning_an_appearance_narrows_the_list_to_that_half() {
    let light = Settings {
        preference: Preference::Light,
        ..Settings::default()
    };
    let html = render(light, true);
    // Every dark preset is still listed, but dimmed and refused.
    assert!(html.contains(r#"class="theme-row off""#), "{html}");
    assert!(html.contains(r#"data-theme-mode="dark""#), "{html}");
    assert!(html.contains("disabled"), "{html}");

    let system = render(Settings::default(), true);
    assert!(!system.contains("theme-row off"), "{system}");
}

#[test]
fn the_footer_says_changes_apply_immediately() {
    let html = render(Settings::default(), true);
    assert!(html.contains("Changes apply immediately"), "{html}");
    assert!(html.contains("Done"), "{html}");
}
