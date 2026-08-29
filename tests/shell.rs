//! Shell model integration: the pure-logic sidebar/status model
//! the app shell renders from, exercised through the public crate API.

use openkite::plugin_api::{
    PluginRegistration, RegistrationStore, RouteSpec, SidebarItem, StatusItem,
};
use openkite::shell::{
    nav_item_from_plugin, plugin_sections, sidebar_model, status_bar_model, status_dot_color,
    status_items_of, ShellState,
};

fn argocd_registration() -> PluginRegistration {
    PluginRegistration {
        sidebar: vec![SidebarItem {
            label: "Applications".into(),
            icon: "grid".into(),
            route: "/argocd/apps".into(),
        }],
        routes: vec![RouteSpec {
            path: "/argocd/apps".into(),
            title: "ArgoCD Applications".into(),
        }],
        status: vec![StatusItem {
            label: "ArgoCD: Synced".into(),
            color: "green".into(),
        }],
    }
}

fn istio_registration() -> PluginRegistration {
    PluginRegistration {
        sidebar: vec![SidebarItem {
            label: "Mesh".into(),
            icon: "mesh".into(),
            route: "/istio/mesh".into(),
        }],
        routes: vec![RouteSpec {
            path: "/istio/mesh".into(),
            title: "Istio Mesh".into(),
        }],
        status: vec![StatusItem {
            label: "Istio: Degraded".into(),
            color: "red".into(),
        }],
    }
}

#[test]
fn sidebar_orders_core_then_registered_plugins() {
    let mut store = RegistrationStore::new();
    store.upsert("istio", istio_registration());
    store.upsert("argocd", argocd_registration());

    let model = sidebar_model(&store);
    assert_eq!(model[0].label, "Overview");
    assert_eq!(model[1].label, "argocd");
    assert_eq!(model[2].label, "istio");

    // `plugin_sections` is exactly sidebar_model minus the core section —
    // what the AppShell appends after the static nav.
    let plugins = plugin_sections(&store);
    assert_eq!(plugins.len(), 2);
    assert_eq!(plugins[0].label, "argocd");
    assert_eq!(plugins[0].items[0].label, "Applications");
    assert_eq!(plugins[0].items[0].route, "/argocd/apps");
    assert_eq!(plugins[0].items[0].plugin.as_deref(), Some("argocd"));
}

#[test]
fn status_bar_lists_connection_version_then_plugins() {
    let mut store = RegistrationStore::new();
    store.upsert("argocd", argocd_registration());
    let state = ShellState {
        cluster: Some("gke_prod".into()),
        namespace: "default".into(),
        connected: true,
    };

    let bar = status_bar_model(&state, &store, "1.2.3");
    assert_eq!(bar[0].label, "gke_prod · Connected");
    assert_eq!(bar[0].color.as_deref(), Some("green"));
    assert_eq!(bar[1].label, "v1.2.3");
    assert_eq!(bar[2].label, "ArgoCD: Synced");

    // Disconnected: muted red dot, fallback cluster label.
    let offline = status_bar_model(&ShellState::default(), &store, "1.2.3");
    assert_eq!(offline[0].label, "no cluster · Disconnected");
    assert_eq!(offline[0].color.as_deref(), Some("red"));
}

#[test]
fn dot_colors_are_safe_css_values() {
    assert_eq!(status_dot_color("green"), "var(--green)");
    assert_eq!(status_dot_color("red"), "var(--red)");
    assert_eq!(status_dot_color("#0d9488"), "#0d9488");
    assert_eq!(status_dot_color("url(https://evil.test)"), "var(--fg-2)");
    assert_eq!(status_dot_color("red;} body{display:none"), "var(--fg-2)");
}

#[test]
fn helpers_wrap_plugin_contributions() {
    let mut store = RegistrationStore::new();
    store.upsert("argocd", argocd_registration());

    let item = store.get("argocd").unwrap().sidebar[0].clone();
    let nav = nav_item_from_plugin("argocd", &item);
    assert_eq!(nav.label, "Applications");
    assert_eq!(nav.route, "/argocd/apps");
    assert_eq!(nav.plugin.as_deref(), Some("argocd"));

    let status = status_items_of(&store, "argocd");
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].label, "ArgoCD: Synced");
    assert!(status_items_of(&store, "ghost").is_empty());
}

#[test]
fn minimal_registration_contributes_no_extra_sections() {
    // Plugins without sidebar items still register status entries; the
    // sidebar skips them, the status bar does not.
    let mut store = RegistrationStore::new();
    store.upsert(
        "metrics",
        PluginRegistration {
            sidebar: Vec::new(),
            routes: Vec::new(),
            status: vec![StatusItem {
                label: "Metrics: Scraping".into(),
                color: "blue".into(),
            }],
        },
    );
    assert!(plugin_sections(&store).is_empty());
    let bar = status_bar_model(&ShellState::default(), &store, "1.2.3");
    assert_eq!(bar.len(), 3);
    assert_eq!(bar[2].label, "Metrics: Scraping");
}
