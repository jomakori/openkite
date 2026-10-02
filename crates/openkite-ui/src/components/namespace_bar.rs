//! The namespace bar (OKT-171, spec §5.1): one narrow strip, not a section.
//!
//! The console's namespace selector is a single horizontally scrollable line of
//! chips that never grows taller and never wraps:
//!
//! - **Search circle on the left** — a 44px circular button opens an inline
//!   field that narrows the *chip list*. It filters options only; the
//!   selection is untouched ([`visible_namespaces`] takes `options`, never the
//!   selection).
//! - **Multi-select chips** — `.chip.active` marks a selected namespace; the
//!   first chip restores the empty selection ("all namespaces"), which is the
//!   only state that is *not* combinable with others.
//! - **× circle reset** — rendered only while a selection exists, pinned after
//!   the scroll container so it is reachable without scrolling.
//!
//! Selection lives in [`crate::runtime::NAMESPACE_SELECTION`], one selection
//! for every data surface ([`selection_matches`] is the predicate they all
//! call). The bar owns the state; the surfaces read it.

use dioxus::prelude::*;

use crate::runtime::{clear_namespace_selection, toggle_namespace, NAMESPACE_SELECTION};

/// The chip that restores the empty selection — "all namespaces".
pub const ALL_LABEL: &str = "All namespaces";

/// The namespaces the strip shows for `query`.
///
/// The search field narrows this list only: the caller passes the *options*,
/// so a filter can never change which namespaces are selected.
pub fn visible_namespaces(options: &[String], query: &str) -> Vec<String> {
    let needle = query.trim().to_lowercase();
    options
        .iter()
        .filter(|namespace| needle.is_empty() || namespace.to_lowercase().contains(&needle))
        .cloned()
        .collect()
}

/// Toggle `namespace` in `selection`, preserving insertion order.
///
/// Pure so the chip handler and the tests share one definition of
/// "multi-select".
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
/// The visible list comes from `options` narrowed by `query` (the search
/// circle's job); `active` comes from `selection`. Keeping the two inputs
/// separate in one function is what makes "filtering never changes the
/// selection" testable: a selected namespace filtered out of the list is still
/// selected, and reappears active the moment the query clears.
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
/// The one predicate every data surface uses: an empty selection matches
/// everything ("all namespaces"), a cluster-scoped object (`None`, e.g. a
/// node) is never excluded by a namespace selection, and a namespaced object
/// matches when its namespace is selected.
pub fn selection_matches(selection: &[String], namespace: Option<&str>) -> bool {
    match namespace {
        None => true,
        Some(namespace) => selection.is_empty() || selection.iter().any(|ns| ns == namespace),
    }
}

/// The namespace strip.
///
/// `options` are the namespace names the host published. The host advertises
/// none (a server-side console with no namespace inventory): the strip renders
/// the design's disabled chip marked `data-unsupported="namespace-filter"`
/// instead of a control that cannot be honoured.
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
            if empty {
                // No namespaces to filter by is declared, not silently absent.
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
                // The search circle sits *outside* the scroll strip: it is the
                // fixed left edge of the bar, so it never scrolls away.
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
                if filter_open() {
                    label { class: "search-field ns-filter",
                        input {
                            r#type: "search",
                            placeholder: "Filter namespaces…",
                            aria_label: "Filter namespaces",
                            value: "{query}",
                            oninput: move |event| query.set(event.value()),
                        }
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
                // The × reset trails the strip (a fixed trailing edge), so it
                // is reachable without scrolling the chips.
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
