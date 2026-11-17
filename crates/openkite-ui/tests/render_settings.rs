//! The settings sheet: it renders the picker, applies density, and a switch
//! changes the tokens the surfaces below it paint with — without a reload.

use dioxus::prelude::*;
use openkite_ui::settings::{builtin_themes, parse_zed_theme, Density, Settings};
use openkite_ui::settings_view::{SettingsSheet, ThemeScope};

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    settings: Settings,
    open: bool,
    import_text: String,
    import_error: Option<String>,
}

#[component]
fn Harness(props: HarnessProps) -> Element {
    rsx! {
        ThemeScope { settings: props.settings.clone(),
            SettingsSheet {
                open: props.open,
                settings: props.settings.clone(),
                themes: builtin_themes(),
                import_text: props.import_text.clone(),
                import_error: props.import_error.clone(),
                on_theme: move |_id: String| {},
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
    let props = HarnessProps {
        settings,
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
    assert!(!html.contains("modal-body"), "{html}");
    assert!(!html.contains("Settings"), "{html}");
}

#[test]
fn the_sheet_is_a_modal_dialog_with_a_title() {
    let html = render(Settings::default(), true);
    assert!(html.contains(r#"class="dlg wide modal""#), "{html}");
    assert!(html.contains(r#"role="dialog""#), "{html}");
    assert!(html.contains(r#"aria-modal="true""#), "{html}");
    assert!(html.contains(r#"aria-label="Settings""#), "{html}");
    assert!(html.contains("modal-head"), "{html}");
    assert!(html.contains("modal-body"), "{html}");
    assert!(html.contains("modal-actions"), "{html}");
}

#[test]
fn all_five_themes_are_offered_and_one_is_marked() {
    let html = render(Settings::default(), true);
    for id in ["frost", "dusk", "midnight", "graphite", "paper"] {
        assert!(
            html.contains(&format!(r#"data-theme-id="{id}""#)),
            "{id}: {html}"
        );
    }
    assert_eq!(
        html.matches(r#"aria-pressed="true""#).count(),
        2,
        "one theme, one density: {html}"
    );
    assert!(html.contains(r#"class="theme-tile on""#), "{html}");
}

#[test]
fn the_default_theme_paints_the_default_tokens() {
    let html = render(Settings::default(), true);
    assert!(html.contains(r#"data-theme="frost""#), "{html}");
    assert!(
        !html.contains("--bg:"),
        "the base sheet needs no overrides: {html}"
    );
}

#[test]
fn switching_theme_changes_the_tokens_without_a_reload() {
    let settings = Settings {
        theme: "midnight".to_string(),
        ..Settings::default()
    };
    let html = render(settings, true);
    assert!(html.contains(r#"data-theme="midnight""#), "{html}");
    assert!(html.contains("--bg:oklch(0.145 0.014 268)"), "{html}");
    assert!(html.contains("--accent:oklch(0.66 0.16 272)"), "{html}");
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
fn every_theme_tile_carries_its_own_preview() {
    let html = render(Settings::default(), true);
    assert_eq!(html.matches("theme-swatch").count(), 5, "{html}");
    assert!(html.contains("data-appearance=\"light\""), "{html}");
    assert!(html.contains("data-appearance=\"dark\""), "{html}");
    // Midnight's tile paints from its own tokens, not the active theme's.
    assert!(html.contains("background:oklch(0.145 0.014 268)"), "{html}");
}

#[test]
fn the_footer_says_changes_apply_immediately() {
    let html = render(Settings::default(), true);
    assert!(html.contains("Changes apply immediately"), "{html}");
    assert!(html.contains("Done"), "{html}");
}
