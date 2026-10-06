//! The console namespace bar: one narrow strip of chips, not a section.

use dioxus::prelude::*;

use crate::runtime::{clear_namespace_selection, toggle_namespace, NAMESPACE_SELECTION};

/// The chip that restores the empty selection — "all namespaces".
pub const ALL_LABEL: &str = "All namespaces";

/// The namespaces the strip shows for `query`.
///
/// Takes the options, never the selection, so a filter cannot change what is
/// selected.
pub fn visible_namespaces(options: &[String], query: &str) -> Vec<String> {
    let needle = query.trim().to_lowercase();
    options
        .iter()
        .filter(|namespace| needle.is_empty() || namespace.to_lowercase().contains(&needle))
        .cloned()
        .collect()
}

/// Toggle `namespace` in `selection`, preserving insertion order.
pub fn toggle_selection(selection: &[String], namespace: &str) -> Vec<String> {
    let mut next = selection.to_vec();
    if let Some(position) = next.iter().position(|candidate| candidate == namespace) {
        next.remove(position);
    } else {
        next.push(namespace.to_string());
    }
    next
}

/// The namespace chips the strip paints for `query`: `(label, active)` pairs.
///
/// `active` comes from `selection`, the visible list from `options` narrowed by
/// `query`; the two inputs stay separate so a filter cannot change the
/// selection.
pub fn chip_rows(options: &[String], query: &str, selection: &[String]) -> Vec<(String, bool)> {
    visible_namespaces(options, query)
        .into_iter()
        .map(|namespace| {
            let active = selection.iter().any(|selected| selected == &namespace);
            (namespace, active)
        })
        .collect()
}

/// Whether `namespace` is in scope for `selection`.
///
/// Empty selection matches everything; a cluster-scoped object (`None`) is
/// never excluded by a namespace selection.
pub fn selection_matches(selection: &[String], namespace: Option<&str>) -> bool {
    match namespace {
        None => true,
        Some(namespace) => selection.is_empty() || selection.iter().any(|ns| ns == namespace),
    }
}

/// The namespace strip; `options` are the namespace names the host published.
#[component]
pub fn NamespaceBar(options: Vec<String>) -> Element {
    let mut filter_open = use_signal(|| false);
    let mut query = use_signal(String::new);
    let has_selection = !NAMESPACE_SELECTION.read().is_empty();
    let selected: Vec<String> = NAMESPACE_SELECTION.read().clone();
    let empty = options.is_empty();

    rsx! {
        div {
            class: "ns-bar",
            "data-empty": if empty { Some("namespaces") } else { None },
            "data-filtering": if filter_open() { "true" } else { "false" },
            if empty {
                div { class: "chip-row", role: "group", aria_label: "Namespace filter",
                    button {
                        class: "chip",
                        r#type: "button",
                        disabled: true,
                        "data-unsupported": "namespace-filter",
                        title: "This host has no namespace inventory to filter by.",
                        "No namespaces"
                    }
                }
            } else {
                button {
                    class: "ns-search",
                    r#type: "button",
                    aria_label: "Filter namespaces",
                    "aria-expanded": if filter_open() { "true" } else { "false" },
                    onclick: move |_| {
                        let next = !filter_open();
                        filter_open.set(next);
                    },
                    svg { class: "icon", "viewBox": "0 0 24 24",
                        circle { cx: "11", cy: "11", r: "6.5" }
                        path { d: "m16 16 4 4" }
                    }
                }
                // The field is always in the row; the search circle shows it and
                // the coarse-pointer stylesheet lifts it above the strip.
                label { class: "search-field ns-filter",
                    input {
                        r#type: "search",
                        placeholder: "Filter namespaces…",
                        aria_label: "Filter namespaces",
                        value: "{query}",
                        oninput: move |event| query.set(event.value()),
                    }
                }
                div { class: "chip-row", role: "group", aria_label: "Namespace filter",
                    button {
                        class: if selected.is_empty() { "chip active" } else { "chip" },
                        r#type: "button",
                        "data-ns": "all",
                        "aria-pressed": if selected.is_empty() { "true" } else { "false" },
                        onclick: move |_| clear_namespace_selection(),
                        "{ALL_LABEL}"
                    }
                    for (namespace, active) in chip_rows(&options, &query(), &selected) {
                        {
                            let for_click = namespace.clone();
                            rsx! {
                                button {
                                    key: "{namespace}",
                                    class: if active { "chip active" } else { "chip" },
                                    r#type: "button",
                                    "data-ns": "{namespace}",
                                    "aria-pressed": if active { "true" } else { "false" },
                                    onclick: move |_| toggle_namespace(for_click.clone()),
                                    "{namespace}"
                                    if active {
                                        svg { class: "chip-mark", "viewBox": "0 0 24 24",
                                            path { d: "m5 12 5 5 9-11" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if has_selection {
                    button {
                        class: "ns-reset",
                        r#type: "button",
                        aria_label: "Clear namespace filter",
                        title: "Clear namespace filter",
                        onclick: move |_| clear_namespace_selection(),
                        svg { class: "icon", "viewBox": "0 0 24 24",
                            path { d: "M6 6l12 12M18 6 6 18" }
                        }
                    }
                }
            }
        }
    }
}
