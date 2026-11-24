//! The settings sheet: theme, density, shortcuts and kubeconfig paths.

use dioxus::prelude::*;

use crate::settings::{Appearance, Density, Preference, Settings, Theme, SHORTCUTS};

/// Returns focus to the topbar button that opens the sheet.
const SETTINGS_REFOCUS: &str = "document.querySelector('[data-action=\"settings\"]')?.focus();";

#[derive(Props, Clone, PartialEq)]
pub struct SettingsSheetProps {
    /// Whether the sheet is open.
    pub open: bool,
    /// The settings being edited.
    pub settings: Settings,
    /// Whether the platform is currently painting light or dark.
    pub system: Appearance,
    /// Every theme the picker offers, built-ins first.
    pub themes: Vec<Theme>,
    /// The text of the Zed import field.
    pub import_text: String,
    /// Why the last import failed, if it did.
    pub import_error: Option<String>,
    pub on_theme: EventHandler<String>,
    pub on_preference: EventHandler<Preference>,
    pub on_density: EventHandler<Density>,
    pub on_import_text: EventHandler<String>,
    pub on_import: EventHandler<()>,
    pub on_add_kubeconfig: EventHandler<String>,
    pub on_remove_kubeconfig: EventHandler<String>,
    pub on_close: EventHandler<()>,
}

#[derive(Props, Clone, PartialEq)]
pub struct ThemeScopeProps {
    pub settings: Settings,
    /// What the platform reports, so `System` has something to resolve against.
    pub system: Appearance,
    pub children: Element,
}

/// Applies a theme and density to everything below it. `display: contents` keeps
/// the wrapper out of the layout, so themes reach the real surfaces.
#[allow(non_snake_case)]
pub fn ThemeScope(props: ThemeScopeProps) -> Element {
    let settings = props.settings.clone();
    let theme = settings.theme(props.system);
    rsx! {
        div {
            class: "theme-scope",
            "data-theme": "{theme.id}",
            "data-appearance": "{theme.appearance.as_str()}",
            "data-density": "{settings.density.as_str()}",
            style: settings.css_vars(props.system),
            {props.children}
        }
    }
}

/// The full sheet, or nothing when it is closed.
#[allow(non_snake_case)]
pub fn SettingsSheet(props: SettingsSheetProps) -> Element {
    if !props.open {
        return rsx! {};
    }
    let settings = props.settings.clone();
    let active = settings.theme(props.system);
    // An imported theme is not in the built-in list, so it gets its own tile.
    let mut themes = props.themes.clone();
    for chosen in [
        settings.theme_for(Appearance::Light),
        settings.theme_for(Appearance::Dark),
    ] {
        if !themes.iter().any(|theme| theme.id == chosen.id) {
            themes.push(chosen);
        }
    }
    let mut listed: Vec<Theme> = themes.clone();
    listed.sort_by(|left, right| left.name.cmp(&right.name));
    let dark_count = listed
        .iter()
        .filter(|theme| theme.appearance == Appearance::Dark)
        .count();
    let light_count = listed.len() - dark_count;
    let active_id = active.id.clone();
    let preference = settings.preference;
    let light_id = settings.theme_light.clone();
    let dark_id = settings.theme_dark.clone();
    let density = settings.density;
    let import_error = props.import_error.clone();
    let import_text = props.import_text.clone();
    let paths = settings.kubeconfig_paths.clone();
    let mut draft = use_signal(String::new);
    let mut drag_from = use_signal(|| 0.0f64);
    let dismiss = {
        let close = props.on_close;
        move || {
            spawn(async move {
                let _ = dioxus::document::eval(SETTINGS_REFOCUS).await;
                close.call(());
            });
        }
    };
    let draft_value = draft();
    rsx! {
        div { class: "set-scrim", "data-settings": "scrim", onclick: move |_| dismiss(),
            aside {
                class: "set-pane",
                role: "dialog",
                "aria-modal": "true",
                "aria-label": "Settings",
                "data-settings": "pane",
                tabindex: "-1",
                autofocus: "true",
                onclick: move |event| event.stop_propagation(),
                onkeydown: move |event| {
                    if event.key().to_string() == "Escape" { dismiss(); }
                },
                div {
                    class: "pane-head",
                    onpointerdown: move |event| drag_from.set(event.data().client_coordinates().y),
                    onpointerup: move |event| {
                        let travelled = event.data().client_coordinates().y - drag_from();
                        drag_from.set(0.0);
                        if travelled > 60.0 { dismiss(); }
                    },
                    h2 { "Settings" }
                    button {
                        class: "btn ghost icon-only",
                        "aria-label": "Close settings",
                        onclick: move |_| dismiss(),
                        dangerous_inner_html: icon("x"),
                    }
                }
                div { class: "pane-body",
                    section { class: "set-group",
                        header { class: "set-head",
                            h3 { "Appearance" }
                            span { class: "muted", "Follow the system, or pin light or dark" }
                        }
                        div { class: "segmented", role: "group", "aria-label": "Appearance",
                            for option in [Preference::System, Preference::Light, Preference::Dark] {
                                button {
                                    key: "{option.as_str()}",
                                    class: if option == preference { "seg on" } else { "seg" },
                                    "data-preference": "{option.as_str()}",
                                    "aria-pressed": if option == preference { "true" } else { "false" },
                                    onclick: move |_| props.on_preference.call(option),
                                    "{option.label()}"
                                }
                            }
                        }
                    }

                    section { class: "set-group theme-set",
                        header { class: "set-head",
                            h3 { "Theme" }
                            span { class: "muted", "{dark_count} dark, {light_count} light — one list, pick by eye" }
                        }
                        div { class: "theme-list", role: "listbox", "aria-label": "Theme",
                            for theme in &listed {
                                {
                                    let id = theme.id.clone();
                                    let mode = theme.appearance.as_str();
                                    let pickable = preference.accepts(theme.appearance);
                                    let chosen = theme.id == active_id;
                                    let slotted = if theme.id == dark_id { "D" } else if theme.id == light_id { "L" } else { "" };
                                    rsx! {
                                        button {
                                            key: "{theme.id}",
                                            class: if chosen { "theme-row on" } else if !pickable { "theme-row off" } else { "theme-row" },
                                            role: "option",
                                            "data-theme-id": "{theme.id}",
                                            "data-theme-mode": "{mode}",
                                            "aria-selected": if chosen { "true" } else { "false" },
                                            "aria-pressed": if chosen { "true" } else { "false" },
                                            disabled: !pickable,
                                            onclick: move |_| props.on_theme.call(id.clone()),
                                            span { class: "theme-rail" }
                                            span { class: "theme-dots",
                                                for tint in palette(theme) {
                                                    span { class: "theme-dot", style: "background:{tint}" }
                                                }
                                            }
                                            span { class: "theme-name", "{theme.name}" }
                                            if !slotted.is_empty() {
                                                span { class: "theme-slot", "{slotted}" }
                                            }
                                            span { class: "theme-mode", "{mode}" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    section { class: "set-group",
                        header { class: "set-head",
                            h3 { "Zed theme" }
                            span { class: "muted", "theme JSON, mapped onto the console tokens" }
                        }
                        div { class: "set-row",
                            textarea {
                                class: "input mono set-import",
                                rows: "3",
                                placeholder: "Paste a Zed theme JSON file",
                                value: "{import_text}",
                                oninput: move |event| props.on_import_text.call(event.value()),
                            }
                            button {
                                class: "btn",
                                "data-action": "import-theme",
                                onclick: move |_| props.on_import.call(()),
                                "Import"
                            }
                        }
                        if let Some(error) = import_error.clone() {
                            p { class: "set-error", role: "alert", "{error}" }
                        }
                    }

                    section { class: "set-group",
                        header { class: "set-head", h3 { "Density" } }
                        div { class: "segmented", role: "group", "aria-label": "Density",
                            for option in [Density::Comfortable, Density::Compact] {
                                button {
                                    key: "{option.as_str()}",
                                    class: if option == density { "seg on" } else { "seg" },
                                    "data-density": "{option.as_str()}",
                                    "aria-pressed": if option == density { "true" } else { "false" },
                                    onclick: move |_| props.on_density.call(option),
                                    "{option.label()}"
                                }
                            }
                        }
                    }

                    section { class: "set-group",
                        header { class: "set-head",
                            h3 { "Shortcuts" }
                            span { class: "muted", "mod is ⌘ on macOS, ctrl elsewhere" }
                        }
                        ul { class: "set-list",
                            for shortcut in SHORTCUTS.iter() {
                                li { class: "set-item", key: "{shortcut.action}",
                                    span { "{shortcut.action}" }
                                    kbd { "{shortcut.keys}" }
                                }
                            }
                        }
                    }

                    section { class: "set-group",
                        header { class: "set-head",
                            h3 { "Kubeconfig paths" }
                            span { class: "muted", "read in order, first match wins" }
                        }
                        ul { class: "set-list",
                            for path in paths.iter() {
                                li { class: "set-item", key: "{path}",
                                    span { class: "mono", "{path}" }
                                    button {
                                        class: "btn ghost icon-only",
                                        "data-remove": "{path}",
                                        "aria-label": "Remove {path}",
                                        onclick: {
                                            let path = path.clone();
                                            move |_| props.on_remove_kubeconfig.call(path.clone())
                                        },
                                        dangerous_inner_html: icon("x"),
                                    }
                                }
                            }
                        }
                        div {
                            class: "set-row",
                            input {
                                class: "input mono",
                                placeholder: "~/.kube/prod.yaml",
                                "aria-label": "Kubeconfig path",
                                value: "{draft_value}",
                                oninput: move |event| draft.set(event.value()),
                            }
                            button {
                                class: "btn",
                                "data-action": "add-kubeconfig",
                                onclick: move |_| {
                                    props.on_add_kubeconfig.call(draft());
                                    draft.set(String::new());
                                },
                                "Add path"
                            }
                        }
                    }
                }
                div { class: "pane-foot",
                    span { class: "muted", "Changes apply immediately" }
                    button { class: "btn btn-primary", onclick: move |_| props.on_close.call(()), "Done" }
                }
            }
        }
    }
}

/// A row's preview strip: the preset's background, raised surface, text and accent, in the
/// order the eye reads them, so a theme is recognisable before its name is.
fn palette(theme: &Theme) -> Vec<String> {
    let tint = |name: &str| {
        theme
            .tokens()
            .iter()
            .find(|(token, _)| token == name)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| "var(--surface)".to_string())
    };
    vec![
        tint("bg"),
        tint("surface-solid"),
        tint("fg"),
        tint("accent"),
    ]
}

fn icon(name: &str) -> String {
    format!(
        "<svg class=\"icon\" aria-hidden=\"true\"><use href=\"{}/assets/icons.svg#i-{name}\"></use></svg>",
        env!("CARGO_MANIFEST_DIR")
    )
}
