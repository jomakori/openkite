//! Router + navigation shell (OKT-7). Static core routes plus a root wildcard
//! that dispatches unknown paths through the plugin route table.

#![allow(non_snake_case)]

use dioxus::prelude::*;
use openkite_plugin_sdk::{SidebarEntry, SidebarSection};
use std::collections::HashMap;

/// Plugin sidebar sections, populated from the registry at startup.
static PLUGIN_SECTIONS: GlobalSignal<Vec<SidebarSection>> = Signal::global(Vec::new);

/// Plugin route table keyed by full path, populated at startup.
static ROUTE_TABLE: GlobalSignal<HashMap<String, fn() -> Element>> = Signal::global(HashMap::new);

/// Install plugin navigation + routes from the registry (once, in `main`).
pub fn install_plugins(sections: Vec<SidebarSection>, routes: HashMap<String, fn() -> Element>) {
    *PLUGIN_SECTIONS.write() = sections;
    *ROUTE_TABLE.write() = routes;
}

/// App entry: render the router.
pub fn app() -> Element {
    rsx! { Router::<Route> {} }
}

/// Convert a plugin route path string to the wildcard `Route` variant.
fn plugin_route(path: &str) -> Route {
    Route::Plugin {
        path: path
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
    }
}

/// Reconstruct a full path from wildcard segments.
fn full_path(path: &[String]) -> String {
    format!("/{}", path.join("/"))
}

/// The `#[layout(AppShell)]` stays open for every route that follows, so the
/// sidebar wraps core routes and the plugin catch-all alike.
#[derive(Routable, Clone, Debug, PartialEq)]
enum Route {
    #[layout(AppShell)]
    #[route("/")]
    Home {},
    #[route("/cluster")]
    Cluster {},
    #[route("/workloads")]
    Workloads {},
    #[route("/config")]
    Config {},
    #[route("/:..path")]
    Plugin { path: Vec<String> },
}

#[component]
fn AppShell() -> Element {
    rsx! {
        div { class: "app-shell",
            Sidebar {}
            main { class: "content",
                Outlet::<Route> {}
            }
        }
    }
}

#[component]
fn Sidebar() -> Element {
    let sections = PLUGIN_SECTIONS.read();
    rsx! {
        aside { class: "sidebar",
            h1 { class: "brand", "OpenKite" }
            span { class: "tagline", "Kubernetes from above." }
            nav { class: "nav",
                NavItem { label: "Cluster", to: Route::Cluster {} }
                NavItem { label: "Workloads", to: Route::Workloads {} }
                NavItem { label: "Config", to: Route::Config {} }
                if !sections.is_empty() {
                    div { class: "nav-divider" }
                    for section in sections.iter() {
                        SectionView { section: section.clone() }
                    }
                }
            }
        }
    }
}

#[component]
fn NavItem(label: String, to: Route) -> Element {
    let current = use_route::<Route>();
    let active = current == to;
    rsx! {
        Link {
            to: to,
            class: if active { "nav-item active" } else { "nav-item" },
            "{label}"
        }
    }
}

#[component]
fn SectionView(section: SidebarSection) -> Element {
    let accent = section
        .accent_color
        .clone()
        .unwrap_or_else(|| "var(--accent)".into());
    rsx! {
        div { class: "nav-section",
            div { class: "nav-section-label", style: "color: {accent}", "{section.label}" }
            for entry in section.entries.iter() {
                PluginNavItem { entry: entry.clone(), accent: accent.clone() }
            }
        }
    }
}

#[component]
fn PluginNavItem(entry: SidebarEntry, accent: String) -> Element {
    let current = use_route::<Route>();
    let target = plugin_route(&entry.route);
    let active = match &current {
        Route::Plugin { path } => full_path(path) == entry.route,
        _ => false,
    };
    rsx! {
        Link {
            to: target,
            class: if active { "nav-item plugin active" } else { "nav-item plugin" },
            style: "color: {accent}",
            "{entry.label}"
        }
    }
}

#[component]
fn Home() -> Element {
    rsx! {
        h2 { "OpenKite" }
        p { "Connect a cluster context to begin." }
    }
}

#[component]
fn Cluster() -> Element {
    rsx! { h2 { "Cluster" } p { "Cluster overview lands in a later ticket." } }
}

#[component]
fn Workloads() -> Element {
    rsx! { crate::workloads::WorkloadView {} }
}

#[component]
fn Config() -> Element {
    rsx! { h2 { "Config" } p { "Config views land in a later ticket." } }
}

/// Wildcard dispatcher: reconstruct the path, look it up in the plugin route
/// table, render the plugin component or the 404 fallback.
#[component]
fn Plugin(path: Vec<String>) -> Element {
    let full = full_path(&path);
    let table = ROUTE_TABLE.read();
    match table.get(&full) {
        Some(render) => (render)(),
        None => rsx! {
            div { class: "not-found",
                h2 { "404" }
                p { "No view for /{full}" }
                Link { to: Route::Home {}, "Back home" }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_route_splits_path_into_segments() {
        match plugin_route("/argocd/apps") {
            Route::Plugin { path } => assert_eq!(path, vec!["argocd", "apps"]),
            other => panic!("expected Plugin variant, got {other:?}"),
        }
    }

    #[test]
    fn plugin_route_strips_trailing_slash() {
        match plugin_route("/argocd/apps/") {
            Route::Plugin { path } => assert_eq!(path, vec!["argocd", "apps"]),
            other => panic!("expected Plugin variant, got {other:?}"),
        }
    }

    #[test]
    fn full_path_round_trips_through_plugin_route() {
        let Route::Plugin { path } = plugin_route("/argocd/apps") else {
            panic!("expected Plugin variant");
        };
        assert_eq!(full_path(&path), "/argocd/apps");
    }
}
