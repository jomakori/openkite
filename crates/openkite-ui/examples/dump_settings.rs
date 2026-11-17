//! Dumps the shell with the settings sheet open, for the design screenshots.
//!
//! Usage: `cargo run -p openkite-ui --example dump_settings -- <out.html> [frost-dark|frost-light|mocha|latte|pinned-light|compact|import]`
//!
//! The mode picks both the theme and what the platform reports, so the
//! `System` preference can be shown resolving either way.

use dioxus::prelude::*;
use openkite_ui::settings::{
    builtin_themes, parse_zed_theme, Appearance, Density, Preference, Settings,
};
use openkite_ui::settings_view::{SettingsSheet, ThemeScope};
use openkite_ui::shell::{AppShell, Connection, Counts};

const IMPORTED: &str = r##"{"themes":[{"name":"Nord Deep","style":{
  "background":"#1b1f27","element":{"background":"#222834"},
  "border":"#2e3644","text":"#d8dee9","accents":[{"color":"#88c0d0"}]}}]}"##;

/// The theme, the appearance the platform reports, the import state.
fn settings_for(mode: &str) -> (Settings, Appearance, String, Option<String>) {
    let mut settings = Settings {
        kubeconfig_paths: vec![
            "~/.kube/config".to_string(),
            "~/kube/prod-eu-west-1.yaml".to_string(),
            "~/kube/staging.yaml".to_string(),
        ],
        ..Settings::default()
    };
    let mut import_text = String::new();
    let mut import_error = None;
    let mut system = Appearance::Dark;
    match mode {
        "frost-light" => system = Appearance::Light,
        "mocha" => settings.set_theme("catppuccin-mocha"),
        "latte" => {
            settings.set_theme("catppuccin-latte");
            system = Appearance::Light;
        }
        "pinned-light" => {
            settings.set_theme("tokyo-night");
            settings.preference = Preference::Light;
            system = Appearance::Dark;
        }
        "compact" => {
            settings.set_theme("gruvbox-dark");
            settings.density = Density::Compact;
        }
        "import" => {
            settings.use_theme(parse_zed_theme(IMPORTED).expect("theme"));
            import_text = IMPORTED.to_string();
            import_error = Some("Accent 1 missing, using the border".to_string());
        }
        _ => {}
    }
    (settings, system, import_text, import_error)
}

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    settings: Settings,
    system: Appearance,
    import_text: String,
    import_error: Option<String>,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_theme = use_callback(|_: String| {});
    let on_preference = use_callback(|_: Preference| {});
    let on_density = use_callback(|_: Density| {});
    let on_import_text = use_callback(|_: String| {});
    let on_import = use_callback(|_: ()| {});
    let on_add_kubeconfig = use_callback(|_: String| {});
    let on_remove_kubeconfig = use_callback(|_: String| {});
    let on_close = use_callback(|_: ()| {});
    rsx! {
        ThemeScope { settings: props.settings.clone(), system: props.system,
            AppShell {
                counts: Counts::new()
                    .with("pods", 34)
                    .with("deployments", 12)
                    .with("nodes", 6)
                    .with("events", 128),
                connection: Connection::Live,
                active_path: "/pods".to_string(),
                cluster: "prod-eu-west-1".to_string(),
                SettingsSheet {
                    open: true,
                    settings: props.settings.clone(),
                    system: props.system,
                    themes: builtin_themes(),
                    import_text: props.import_text.clone(),
                    import_error: props.import_error.clone(),
                    on_theme,
                    on_preference,
                    on_density,
                    on_import_text,
                    on_import,
                    on_add_kubeconfig,
                    on_remove_kubeconfig,
                    on_close,
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "settings.html".to_string());
    let mode = args.get(2).map(String::as_str).unwrap_or("frost-dark");
    let (settings, system, import_text, import_error) = settings_for(mode);

    let mut dom = VirtualDom::new_with_props(
        Harness,
        HarnessProps {
            settings,
            system,
            import_text,
            import_error,
        },
    );
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    let sprite = include_str!("../assets/icons.svg");
    let page = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>openkite · settings</title>\
         <link rel=\"stylesheet\" href=\"{}/assets/console.css\"></head>\
         <body class=\"console\">{sprite}{html}</body></html>",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::write(&out, page).expect("write dump");
    println!("{out}");
}
