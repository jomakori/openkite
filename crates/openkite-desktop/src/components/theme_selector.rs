//! Theme selector — omarchy-style picker.
//!
//! Live swatch preview per theme (bg/fg/accent chips), curated defaults
//! section first, then the full 39-theme store. Click applies instantly —
//! the theme engine makes switching a CSS variable swap, so the picker row's
//! preview is exactly what the app becomes.

// No live consumer yet; the settings view is the intended entry point.
#![allow(dead_code)]
#![allow(non_snake_case)]

use dioxus::prelude::*;

use crate::theme_catalog::{self, ThemeEntry};

/// One row: swatch + name + variant, click to apply.
#[component]
fn ThemeRow(entry: ThemeEntry, current: String, on_select: EventHandler<String>) -> Element {
    let (bg, fg, accent) = theme_catalog::swatches(&entry.id);
    let active = theme_catalog::matches_current(&entry.id, &current);
    let id = entry.id.clone();
    let class = if active {
        "theme-row theme-row-active"
    } else {
        "theme-row"
    };
    rsx! {
        button {
            class: "{class}",
            onclick: move |_| on_select.call(id.clone()),
            span { class: "theme-swatch",
                span { class: "swatch-chip", background_color: "{bg}", title: "{bg}" }
                span { class: "swatch-chip", background_color: "{fg}", title: "{fg}" }
                span { class: "swatch-chip", background_color: "{accent}", title: "{accent}" }
            }
            span { class: "theme-name", "{entry.display_name}" }
            span { class: "theme-variant", "{entry.variant}" }
        }
    }
}

/// The full picker: defaults group, then the store group.
#[component]
pub fn ThemeSelector(current: String, on_select: EventHandler<String>) -> Element {
    let all = theme_catalog::catalog();
    let defaults: Vec<ThemeEntry> = all.iter().filter(|e| e.is_default).cloned().collect();
    let store: Vec<ThemeEntry> = all.iter().filter(|e| !e.is_default).cloned().collect();
    rsx! {
        div { class: "theme-selector",
            div { class: "theme-group", "Defaults" }
            for entry in &defaults {
                ThemeRow { entry: entry.clone(), current: current.clone(), on_select }
            }
            div { class: "theme-group", "Store ({store.len()})" }
            for entry in &store {
                ThemeRow { entry: entry.clone(), current: current.clone(), on_select }
            }
        }
    }
}
