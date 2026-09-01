//! Router + navigation shell: core routes plus a root
//! wildcard that dispatches unknown paths through the plugin route table;
//! the app shell chrome (sidebar + status footer); the `/openkite` bridge
//! asset handler; and one-time JS plugin bundle evaluation.

#![allow(non_snake_case)]

use crate::bridge::Bridge;
use crate::plugin_api::ApiResponse;
use crate::runtime::{bridge as shared_bridge, js_plugins, REGISTRATIONS};
use dioxus::desktop::wry;
use dioxus::desktop::wry::http::Response as AssetHttpResponse;
use dioxus::desktop::{use_asset_handler, AssetRequest, RequestAsyncResponder};
use dioxus::prelude::*;
use openkite_plugin_sdk::{SidebarEntry, SidebarSection};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::switcher::{ClusterSwitcher, SwitcherKeybind};
use crate::views::pod_detail::PodDetail;

/// Plugin sidebar sections (static Rust SDK plugins), populated at startup.
static PLUGIN_SECTIONS: GlobalSignal<Vec<SidebarSection>> = Signal::global(Vec::new);

/// Plugin route table keyed by full path, populated at startup.
static ROUTE_TABLE: GlobalSignal<HashMap<String, fn() -> Element>> = Signal::global(HashMap::new);

/// Evaluated-plugin guard: JS bundles eval exactly once per process, on the
/// first AppShell mount (after the asset handler is registered).
static EVALUATED_JS_PLUGINS: OnceLock<()> = OnceLock::new();

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
/// sidebar + status footer wrap core routes and the plugin catch-all alike.
#[derive(Routable, Clone, Debug, PartialEq)]
enum Route {
    #[layout(AppShell)]
    #[route("/")]
    Home {},
    #[route("/cluster")]
    Cluster {},
    #[route("/workloads")]
    Workloads {},
    #[route("/logs")]
    Logs {},
    #[route("/config")]
    Config {},
    #[route("/:..path")]
    Plugin { path: Vec<String> },
}

#[component]
fn AppShell() -> Element {
    // Mount the `/openkite` bridge endpoint. The webview's fetch
    // POSTs (plugin `register` calls + `openkite.api.*` requests) dispatch on
    // the first URL path segment, so the handler name `openkite` is the route.
    // The shared bridge lives in a process-wide `OnceLock` (set in `run`
    // before launch), so re-renders never re-mount or race it.
    use_asset_handler(
        "openkite",
        |req: AssetRequest, responder: RequestAsyncResponder| {
            dispatch_bridge_post(req, responder);
        },
    );

    // Evaluate every discovered JS plugin bundle exactly once per process:
    // the first AppShell mount, after the asset handler is registered and
    // the bootstrap script (`window.openkite`, injected in the page head by
    // `run`) is live. Register POSTs from the bundles then flow through the
    // asset handler, which refreshes the `REGISTRATIONS` mirror — the
    // sidebar and status footer re-render.
    use_effect(move || {
        if EVALUATED_JS_PLUGINS.set(()).is_ok() {
            for bundle in js_plugins() {
                match crate::plugin_js::load_source(&bundle) {
                    Ok(source) => {
                        tracing::info!(plugin = %bundle.name, "evaluating js plugin bundle");
                        // Stamp the plugin identity for the envelope `plugin`
                        // field; clear it so stray calls can't masquerade.
                        let wrapped = format!(
                            "window.__openkite_plugin = {:?};\n{}\nwindow.__openkite_plugin = null;",
                            bundle.name, source,
                        );
                        document::eval(&wrapped);
                    }
                    Err(error) => {
                        tracing::warn!(plugin = %bundle.name, %error, "js plugin bundle load failed");
                    }
                }
            }
        }
    });

    rsx! {
        div { class: "app-shell",
            SwitcherKeybind {}
            ClusterSwitcher {}
            PodDetail {}
            Sidebar {}
            div { class: "main-col",
                TopBar {}
                main { class: "content",
                    Outlet::<Route> {}
                }
                StatusFooter {}
            }
        }
    }
}

/// Top bar: namespace multi-select chips.
///
/// The cluster switcher lives in the ctrl-tab overlay (OKT-51), so the top
/// bar only shows namespace chips for scoping resource queries.
#[component]
fn TopBar() -> Element {
    let namespaces = crate::runtime::NAMESPACES.read();
    let selected = crate::runtime::SELECTED_NAMESPACES.read();
    let ns_list: Vec<String> = namespaces.clone();
    let chips: Vec<(String, bool)> = ns_list
        .iter()
        .map(|ns| (ns.clone(), selected.iter().any(|s| s == ns)))
        .collect();

    rsx! {
        header { class: "topbar",
            div { class: "ns-chips",
                for (ns, is_active) in chips {
                    button {
                        class: if is_active { "ns-chip active" } else { "ns-chip" },
                        onclick: move |_| crate::runtime::toggle_namespace(ns.clone()),
                        "{ns}"
                    }
                }
            }
        }
    }
}

/// Dispatch one bridge POST from the webview.
///
/// Non-POST requests and a missing bridge answer immediately with an error
/// envelope; real work runs on the ambient tokio runtime (the same runtime
/// dioxus uses for its own protocol handlers) and answers through the async
/// responder, keeping the UI thread free.
fn dispatch_bridge_post(req: AssetRequest, responder: RequestAsyncResponder) {
    if req.method() != wry::http::Method::POST {
        responder.respond(json_response(ApiResponse::Error {
            error: "method not allowed: bridge requests must be POST".into(),
        }));
        return;
    }
    let Some(bridge) = shared_bridge() else {
        responder.respond(json_response(ApiResponse::Error {
            error: "bridge not installed".into(),
        }));
        return;
    };
    let Ok(text) = std::str::from_utf8(req.body()) else {
        responder.respond(json_response(ApiResponse::Error {
            error: "request body is not utf-8".into(),
        }));
        return;
    };
    let text = text.to_string();
    tokio::spawn(async move {
        let resp = bridge.handle_post(&text).await;
        refresh_registrations(&bridge);
        responder.respond(json_response(resp));
    });
}

/// Mirror the bridge's registration store into the reactive `REGISTRATIONS`
/// signal; the sidebar + status footer render from the mirror.
///
/// A failed borrow just means a render holds the signal right now — safe to
/// skip, the next register POST re-mirrors.
fn refresh_registrations(bridge: &Arc<Bridge>) {
    match REGISTRATIONS.try_write_unchecked() {
        Ok(mut mirror) => *mirror = bridge.snapshot(),
        Err(_) => {
            tracing::warn!("registration mirror busy; sidebar refresh deferred to next register")
        }
    }
}

/// Serialize an [`ApiResponse`] into a JSON HTTP response for the webview.
fn json_response(resp: ApiResponse) -> AssetHttpResponse<Vec<u8>> {
    let body = serde_json::to_vec(&resp).unwrap_or_else(|err| {
        serde_json::to_vec(&ApiResponse::Error {
            error: format!("serialize response: {err}"),
        })
        .expect("serializing an error fallback cannot fail")
    });
    AssetHttpResponse::builder()
        .header("Content-Type", "application/json")
        .body(body)
        .expect("static response parts")
}

/// One status-bar slot: precomputed label + dot style (pure render data).
fn status_rows(entries: &[crate::shell::StatusBarEntry]) -> Vec<(String, String)> {
    entries
        .iter()
        .map(|entry| {
            let dot = match entry.color.as_deref() {
                Some(color) => format!("background: {}", crate::shell::status_dot_color(color)),
                None => "display: none".into(),
            };
            (entry.label.clone(), dot)
        })
        .collect()
}

#[component]
fn Sidebar() -> Element {
    let sections = PLUGIN_SECTIONS.read();
    let registrations = REGISTRATIONS.read();
    let js_sections = crate::shell::plugin_sections(&registrations);
    rsx! {
        aside { class: "sidebar",
            h1 { class: "brand", "OpenKite" }
            span { class: "tagline", "Kubernetes from above." }
            nav { class: "nav",
                NavItem { label: "Cluster", to: Route::Cluster {} }
                NavItem { label: "Workloads", to: Route::Workloads {} }
                NavItem { label: "Logs", to: Route::Logs {} }
                NavItem { label: "Config", to: Route::Config {} }
                if !sections.is_empty() {
                    div { class: "nav-divider" }
                    for section in sections.iter() {
                        SectionView { section: section.clone() }
                    }
                }
                if !js_sections.is_empty() {
                    div { class: "nav-divider" }
                    for section in js_sections.iter() {
                        ShellSectionView { section: section.clone() }
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

/// A sidebar section contributed by a JS plugin at runtime (rendered from the
/// `REGISTRATIONS` mirror, not the static SDK registry).
#[component]
fn ShellSectionView(section: crate::shell::ShellSection) -> Element {
    rsx! {
        div { class: "nav-section",
            div { class: "nav-section-label", "{section.label}" }
            for item in section.items.iter() {
                ShellNavItemView { item: item.clone() }
            }
        }
    }
}

#[component]
fn ShellNavItemView(item: crate::shell::ShellNavItem) -> Element {
    let current = use_route::<Route>();
    let target = plugin_route(&item.route);
    let active = match &current {
        Route::Plugin { path } => full_path(path) == item.route,
        _ => false,
    };
    rsx! {
        Link {
            to: target,
            class: if active { "nav-item plugin active" } else { "nav-item plugin" },
            "{item.label}"
        }
    }
}

/// Status footer: cluster · connection dot + app version + plugin
/// status items — the mockup's bottom bar. Renders from the same model the
/// pure shell module exposes.
#[component]
fn StatusFooter() -> Element {
    let context = crate::runtime::CONTEXT.read();
    let connected = crate::runtime::CLIENT.read().is_some();
    let registrations = REGISTRATIONS.read();
    let state = crate::shell::ShellState {
        cluster: context.clone(),
        namespace: "default".into(),
        connected,
        prometheus: crate::runtime::PROMETHEUS.read().clone(),
    };
    let entries = crate::shell::status_bar_model(&state, &registrations, env!("CARGO_PKG_VERSION"));
    let rows = status_rows(&entries);
    rsx! {
        footer { class: "status",
            for (label, dot) in rows {
                span { class: "status-entry",
                    span { class: "status-dot", style: "{dot}" }
                    "{label}"
                }
            }
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
    rsx! { crate::views::workloads::WorkloadView {} }
}

#[component]
fn Logs() -> Element {
    rsx! { crate::views::logs::LogsView {} }
}

#[component]
fn Config() -> Element {
    rsx! { h2 { "Config" } p { "Config views land in a later ticket." } }
}

/// Wildcard dispatcher: reconstruct the path, look it up in the static
/// Rust SDK route table, then fall back to the JS-registered renderer
/// paths. A JS match renders a `JsRouteSlot`; otherwise the 404 fallback.
#[component]
fn Plugin(path: Vec<String>) -> Element {
    let full = full_path(&path);

    // Publish the current path for JS-side consumers that need to know
    // the host's URL (e.g. plugins that want to read the current route
    // without waiting for the next `_renderRoute` dispatch).
    *crate::runtime::CURRENT_ROUTE.write() = full.clone();

    let table = ROUTE_TABLE.read();
    if let Some(render) = table.get(&full) {
        return (render)();
    }
    drop(table);

    // Static table missed — try the JS-registered renderer paths. A path
    // match here means a JS plugin declared "I render `<full>`" via
    // `openkite.registerRouteRenderer`; the host's `Route::Plugin`
    // wildcard has captured it, so render the slot.
    let registrations = REGISTRATIONS.read();
    let is_js_route = registrations
        .all_renderer_paths()
        .iter()
        .any(|(_, p)| *p == full);
    drop(registrations);
    if is_js_route {
        return rsx! { JsRouteSlot { path: full } };
    }

    rsx! {
        div { class: "not-found",
            h2 { "404" }
            p { "No view for /{full}" }
            Link { to: Route::Home {}, "Back home" }
        }
    }
}

/// Mount node for a JS-owned route. Renders a `<div
/// data-js-route-mount={path}>` inside the host main outlet (NOT a
/// `position: fixed` overlay), then dispatches
/// `window.openkite._renderRoute(path, container)` via `document::eval`
/// in a `use_effect` that re-runs on every path change. The plugin's
/// render fn is responsible for idempotency (call the previous unmount
/// before mounting new UI; storing the unmount on the container keeps
/// re-runs cheap).
#[component]
fn JsRouteSlot(path: String) -> Element {
    // Precompute the eval source outside `rsx!` and `use_effect` so the
    // path is an owned `String` (the `&'static` requirement of
    // `document::eval` is satisfied by the runtime-allocated literal
    // inside the format!).
    let source = format!(
        r#"(function() {{
          var el = document.querySelector('[data-js-route-mount="{}"]');
          if (!el) return;
          if (window.openkite && typeof window.openkite._renderRoute === 'function') {{
            window.openkite._renderRoute('{}', el);
          }}
        }})();"#,
        path.replace('\\', "\\\\").replace('\'', "\\'"),
        path.replace('\\', "\\\\").replace('\'', "\\'")
    );

    use_effect(move || {
        document::eval(&source);
    });

    rsx! {
        div { class: "js-route-slot", "data-js-route-mount": "{path}" }
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

    #[test]
    fn status_rows_maps_colors_and_hides_undotted_entries() {
        let entries = vec![
            crate::shell::StatusBarEntry {
                label: "prod · Connected".into(),
                color: Some("green".into()),
                plugin: None,
            },
            crate::shell::StatusBarEntry {
                label: "v0.0.0".into(),
                color: None,
                plugin: None,
            },
        ];
        let rows = status_rows(&entries);
        assert_eq!(rows[0].0, "prod · Connected");
        assert_eq!(rows[0].1, "background: var(--green)");
        assert_eq!(rows[1].0, "v0.0.0");
        assert_eq!(rows[1].1, "display: none");
    }

    #[test]
    fn registration_renderers_are_exposed_by_path() {
        use crate::plugin_api::{PluginRegistration, RegistrationStore, RendererSpec};
        let mut store = RegistrationStore::new();
        store.upsert(
            "argocd",
            PluginRegistration {
                renderers: vec![RendererSpec {
                    path: "/argocd/apps".into(),
                }],
                ..PluginRegistration::default()
            },
        );
        let renderers = store.all_renderer_paths();
        assert_eq!(renderers, vec![("argocd", "/argocd/apps")]);
    }
}
