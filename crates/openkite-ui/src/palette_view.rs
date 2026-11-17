//! The palette overlay: a search field, grouped hits, and the keyboard hints underneath.
//!
//! All querying happens in [`crate::palette`]; this module renders whatever list it is handed and
//! reports the input, the highlighted row and the accept. It holds no index of its own.

use dioxus::prelude::*;

use crate::palette::Hit;

/// Props for [`PaletteOverlay`].
#[derive(Props, Clone, PartialEq)]
pub struct PaletteOverlayProps {
    pub open: bool,
    pub query: String,
    pub hits: Vec<Hit>,
    /// Row the keyboard would accept.
    pub active: usize,
    pub on_query: EventHandler<String>,
    pub on_active: EventHandler<usize>,
    pub on_accept: EventHandler<Hit>,
    pub on_close: EventHandler<()>,
}

/// The command palette.
#[component]
pub fn PaletteOverlay(props: PaletteOverlayProps) -> Element {
    if !props.open {
        return rsx! {};
    }
    let query = props.query.clone();
    let on_query = props.on_query;
    let on_accept = props.on_accept;
    let on_close = props.on_close;
    let active = props.active.min(props.hits.len().saturating_sub(1));
    let objects = props
        .hits
        .iter()
        .filter(|h| matches!(h, Hit::Object { .. }))
        .count();
    let commands = props.hits.len() - objects;

    rsx! {
        div { class: "dlg-scrim", "data-palette": "scrim", onclick: move |_| on_close.call(()) ,
        div {
            class: "dlg wide cm-panel",
            role: "dialog",
            "aria-modal": "true",
            "aria-label": "Search and commands",
            "data-palette": "panel",
            div { class: "cm-search",
                label { class: "search-field",
                    span { class: "ic", dangerous_inner_html: "{icon(\"search\")}" }
                    input {
                        r#type: "search",
                        class: "cm-input",
                        placeholder: "Search objects, or > for commands",
                        "aria-label": "Search objects or commands",
                        autocomplete: "off",
                        value: "{query}",
                        oninput: move |evt| on_query.call(evt.value()),
                        onkeydown: move |evt| {
                            if evt.key() == Key::Enter {
                                if let Some(hit) = props.hits.get(active) {
                                    on_accept.call(hit.clone());
                                }
                            }
                        },
                    }
                }
            }
            div { class: "cm-list",
                if props.hits.is_empty() {
                    div { class: "cm-empty muted", "Nothing matches that." }
                } else {
                    if objects > 0 {
                        div { class: "menu-group", "Objects" }
                        for (at, hit) in props.hits.iter().enumerate() {
                            if matches!(hit, Hit::Object { .. }) {
                                HitRow { hit: hit.clone(), at, active, on_active: props.on_active, on_accept }
                            }
                        }
                    }
                    if commands > 0 {
                        div { class: "menu-group", "Commands" }
                        for (at, hit) in props.hits.iter().enumerate() {
                            if matches!(hit, Hit::Command { .. }) {
                                HitRow { hit: hit.clone(), at, active, on_active: props.on_active, on_accept }
                            }
                        }
                    }
                }
            }
            div { class: "cm-foot",
                span { class: "cm-keys",
                    kbd { "↑" }
                    kbd { "↓" }
                    span { "move" }
                    kbd { "↵" }
                    span { "open" }
                    kbd { "esc" }
                    span { "close" }
                }
                span { class: "muted cm-count", "{objects} objects · {commands} commands" }
            }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct HitRowProps {
    hit: Hit,
    at: usize,
    active: usize,
    on_active: EventHandler<usize>,
    on_accept: EventHandler<Hit>,
}

#[allow(non_snake_case)]
fn HitRow(props: HitRowProps) -> Element {
    let on_active = props.on_active;
    let on_accept = props.on_accept;
    let hit = props.hit.clone();
    let at = props.at;
    let chosen = props.at == props.active;
    let is_command = matches!(hit, Hit::Command { .. });
    rsx! {
        button {
            class: "cm-item",
            r#type: "button",
            role: "option",
            "aria-selected": "{chosen}",
            "data-hit": "{hit.key()}",
            "data-command": "{is_command}",
            onmouseenter: move |_| on_active.call(at),
            onclick: {
                let hit = hit.clone();
                move |_| on_accept.call(hit.clone())
            },
            span { class: "ic", dangerous_inner_html: "{icon(icon_for(&hit))}" }
            span { class: "cm-text",
                b { "{hit.title()}" }
                small { "{hit.subtitle()}" }
            }
            if !keys_for(&hit).is_empty() {
                span { class: "cm-key", "{keys_for(&hit)}" }
            }
        }
    }
}

fn icon_for(hit: &Hit) -> &'static str {
    match hit {
        Hit::Object { kind, .. } => match kind.as_str() {
            "pod" => "pods",
            "deployment" => "deployments",
            "node" => "nodes",
            "event" => "events",
            "argoapp" => "argo",
            "namespace" => "namespaces",
            _ => "search",
        },
        Hit::Command { id, .. } => match *id {
            "overview" => "overview",
            "logs" => "logs",
            "customize" | "settings" => "settings",
            _ => "search",
        },
    }
}

fn keys_for(hit: &Hit) -> &str {
    match hit {
        Hit::Command { keys, .. } => keys,
        Hit::Object { .. } => "",
    }
}

fn icon(name: &str) -> String {
    format!(
        "<svg class=\"icon\" aria-hidden=\"true\"><use href=\"{}/assets/icons.svg#i-{name}\"></use></svg>",
        env!("CARGO_MANIFEST_DIR")
    )
}
