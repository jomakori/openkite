//! Shell model integration: the pure-logic sidebar/status model
//! the app shell renders from, exercised through the public crate API.

use openkite_ui::plugin_api::{
    PluginRegistration, RegistrationStore, RouteSpec, SidebarItem, StatusItem,
};
use openkite_ui::shell::{
    bottom_tabs, core_nav, nav_item_from_plugin, plugin_sections, sidebar_model, status_bar_model,
    status_dot_color, status_items_of, ShellNavItem, ShellSection, ShellState, BOTTOM_TAB_SLOTS,
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
        renderers: Vec::new(),
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
        renderers: Vec::new(),
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
        prometheus: None,
    };

    let bar = status_bar_model(&state, &store, "1.2.3");
    assert_eq!(bar[0].label, "gke_prod · Connected");
    assert_eq!(bar[0].color.as_deref(), Some("green"));
    assert_eq!(bar[1].label, "v1.2.3");
    assert_eq!(bar[2].label, "ArgoCD: Synced");

    let offline = status_bar_model(&ShellState::default(), &store, "1.2.3");
    assert_eq!(offline[0].label, "no cluster · Disconnected");
    assert_eq!(offline[0].color.as_deref(), Some("red"));
}

#[test]
fn status_bar_omits_the_version_slot_when_the_build_carries_none() {
    let store = RegistrationStore::new();
    let bar = status_bar_model(&ShellState::default(), &store, "");
    assert_eq!(bar.len(), 1);
    assert_eq!(bar[0].label, "no cluster · Disconnected");
}

#[test]
fn dot_colors_are_safe_css_values() {
    assert_eq!(status_dot_color("green"), "var(--success)");
    assert_eq!(status_dot_color("red"), "var(--danger)");
    assert_eq!(status_dot_color("#0d9488"), "#0d9488");
    assert_eq!(status_dot_color("url(https://evil.test)"), "var(--subtle)");
    assert_eq!(status_dot_color("red;} body{display:none"), "var(--subtle)");
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
            renderers: Vec::new(),
        },
    );
    assert!(plugin_sections(&store).is_empty());
    let bar = status_bar_model(&ShellState::default(), &store, "1.2.3");
    assert_eq!(bar.len(), 3);
    assert_eq!(bar[2].label, "Metrics: Scraping");
}

#[test]
fn status_bar_includes_prometheus_entry_when_detected() {
    let store = RegistrationStore::new();
    let state = ShellState {
        cluster: Some("prod".into()),
        connected: true,
        prometheus: Some("kube-prometheus-stack-prometheus".into()),
        ..ShellState::default()
    };
    let bar = status_bar_model(&state, &store, "1.2.3");
    assert_eq!(bar.len(), 3);
    assert_eq!(
        bar[2].label,
        "Prometheus · kube-prometheus-stack-prometheus"
    );
    assert_eq!(bar[2].color.as_deref(), Some("green"));
}

#[test]
fn dot_color_rejects_non_function_shapes_and_keeps_hex_lowercase() {
    assert_eq!(status_dot_color("#ABCDEF"), "#abcdef");
    assert_eq!(status_dot_color("info"), "var(--accent)");
    assert_eq!(status_dot_color("red)"), "var(--subtle)");
    assert_eq!(status_dot_color("#0123456789"), "var(--subtle)");
    assert_eq!(status_dot_color("hsla(0,0%,0%)\",x:1)"), "var(--subtle)");
}

/// The bottom bar is a second view of this model, never a second route list.
#[test]
fn bottom_tabs_take_the_models_leading_routes_up_to_the_grid() {
    let sections = vec![ShellSection {
        label: "Overview".into(),
        accent: None,
        items: core_nav(true),
    }];

    let tabs = bottom_tabs(&sections);
    assert_eq!(BOTTOM_TAB_SLOTS, 3);
    assert_eq!(
        tabs.iter()
            .map(|tab| tab.route.as_str())
            .collect::<Vec<_>>(),
        vec!["/cluster", "/workloads", "/logs"]
    );
    // Each tab is the model's own entry — label, route and plugin intact.
    assert_eq!(tabs, core_nav(true)[..BOTTOM_TAB_SLOTS].to_vec());
}

#[test]
fn bottom_tabs_never_repeat_a_destination() {
    let mut store = RegistrationStore::new();
    store.upsert("argocd", argocd_registration());
    let mut model = sidebar_model(&store);
    // A plugin contributing a route the core navigation already carries.
    model.push(ShellSection {
        label: "dup".into(),
        accent: None,
        items: vec![ShellNavItem {
            label: "Pods Again".into(),
            route: "/workloads".into(),
            plugin: Some("dup".into()),
            badge: None,
        }],
    });

    let tabs = bottom_tabs(&model);
    let routes: Vec<&str> = tabs.iter().map(|tab| tab.route.as_str()).collect();
    let unique: std::collections::HashSet<&str> = routes.iter().copied().collect();
    assert_eq!(
        routes.len(),
        unique.len(),
        "a tab repeats a destination: {routes:?}"
    );
    assert!(
        tabs.iter().all(|tab| model
            .iter()
            .flat_map(|section| section.items.iter())
            .any(|item| item == tab)),
        "every tab is an entry of the model: {tabs:?}"
    );
}

#[test]
fn bottom_tabs_take_the_destinations_a_shorter_model_has() {
    let one = vec![ShellSection {
        label: "Overview".into(),
        accent: None,
        items: vec![ShellNavItem {
            label: "Cluster".into(),
            route: "/cluster".into(),
            plugin: None,
            badge: None,
        }],
    }];
    assert_eq!(bottom_tabs(&one).len(), 1);
    assert!(bottom_tabs(&[]).is_empty());
}
