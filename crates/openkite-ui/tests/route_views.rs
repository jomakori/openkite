//! Headless mounts of the route chrome (OKT-155).
//!
//! The chrome is props in, markup out: these mount the crate's `RouteView` the
//! way each host mounts it — desktop profile (in-process gateway, terminal and
//! context list) against browser profile (server-side gateway, read-only) — and
//! assert the design's structure, the declared empty state, and that an
//! affordance a host cannot honour is declared rather than dropped.
//! tests/capability_gate.rs pins the predicates the chrome reads.

mod support;

use std::sync::Mutex;

use dioxus::prelude::*;
use openkite_api::capability::Capabilities;
use openkite_ui::components::route_views::{route_page, RouteCapability, RouteView};
use openkite_ui::plugin_api::RegistrationStore;
use openkite_ui::runtime::set_published_capabilities;
use openkite_ui::shell::{core_nav, sidebar_model, ShellSection};

// The host descriptor is process-global, exactly like tests/shell_chrome.rs:
// hold this guard in every test that publishes a profile so parallel test
// threads cannot gate on each other's state.
static HOST_PROFILE: Mutex<()> = Mutex::new(());

/// Publish the desktop profile and hold the lock for the mount.
fn desktop_host() -> std::sync::MutexGuard<'static, ()> {
    let guard = HOST_PROFILE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    set_published_capabilities(Some(Capabilities::in_process()));
    guard
}

/// Publish the browser profile and hold the lock.
fn browser_host() -> std::sync::MutexGuard<'static, ()> {
    let guard = HOST_PROFILE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    set_published_capabilities(Some(Capabilities::server_side()));
    guard
}

/// The desktop's sidebar model: the core navigation block (terminal included).
fn desktop_sections() -> Vec<ShellSection> {
    vec![ShellSection {
        label: "Overview".into(),
        accent: None,
        items: core_nav(true),
    }]
}

/// The browser's sidebar model: the core sections, terminal excluded.
fn browser_sections() -> Vec<ShellSection> {
    sidebar_model(&RegistrationStore::default())
}

fn with_action(route: &'static str, sections: Vec<ShellSection>, wired: bool) -> Element {
    let on_action = wired.then(|| EventHandler::new(|_: String| {}));
    rsx! {
        RouteView { route: route.to_string(), sections, on_action }
    }
}

#[test]
fn the_page_head_reads_the_navigation_words() {
    let sections = desktop_sections();
    let workloads = route_page("workloads", &sections);
    assert_eq!(workloads.route, "/workloads");
    assert_eq!(workloads.title, "Workloads");
    assert_eq!(workloads.eyebrow.as_deref(), Some("Overview"));

    // The home route is not a nav entry: the head borrows the section's word
    // and the route copy's title.
    let home = route_page("/", &sections);
    assert_eq!(home.title, "Cluster");
    assert_eq!(home.eyebrow.as_deref(), Some("Overview"));

    // Same page for every spelling of the same contract.
    for spelling in ["", "/"] {
        assert_eq!(route_page(spelling, &sections).route, "/");
    }
    assert_eq!(route_page("/cluster/", &sections).route, "/cluster");
    assert_eq!(route_page(" workloads ", &sections).route, "/workloads");
}

#[test]
fn every_route_declares_capabilities_and_offers_actions() {
    let sections = desktop_sections();
    let workloads = route_page("/workloads", &sections);
    assert_eq!(
        workloads.declared_capabilities(),
        vec![RouteCapability::Terminal, RouteCapability::Mutations]
    );
    assert_eq!(
        workloads
            .actions
            .iter()
            .map(|action| action.id)
            .collect::<Vec<_>>(),
        vec!["terminal", "new-pod"]
    );
    assert_eq!(
        route_page("/cluster", &sections).declared_capabilities(),
        vec![RouteCapability::ClusterSwitch]
    );
    // The landing route is the cluster's landing: same affordance.
    assert_eq!(
        route_page("/", &sections).declared_capabilities(),
        vec![RouteCapability::ClusterSwitch]
    );
    assert_eq!(
        route_page("/config", &sections).declared_capabilities(),
        vec![RouteCapability::Mutations]
    );
    // A route the chrome does not own still gets a head, declares the one
    // capability it needs (a host that serves plugin bundles — OKT-156) and
    // offers no dead controls.
    let unknown = route_page("/argocd/apps", &sections);
    assert!(unknown.actions.is_empty());
    assert_eq!(
        unknown.declared_capabilities(),
        vec![RouteCapability::PluginRoute]
    );
}

#[test]
fn the_desktop_profile_renders_the_designs_route_chrome() {
    let _host = desktop_host();
    let html = support::mount_html(
        || with_action("/workloads", desktop_sections(), true),
        || {},
    );

    assert!(html.contains("data-route=\"/workloads\""), "route: {html}");
    assert!(html.contains("class=\"page-head\""), "page head: {html}");
    assert!(html.contains("class=\"eyebrow\""), "eyebrow: {html}");
    assert!(html.contains("<h1>Workloads</h1>"), "title: {html}");
    assert!(html.contains("class=\"page-sub\""), "sub line: {html}");
    assert!(
        html.contains("class=\"page-actions\""),
        "action row: {html}"
    );
    assert!(html.contains("class=\"toolbar\""), "toolbar: {html}");
    assert!(html.contains("class=\"chip-row\""), "filter row: {html}");
    assert!(
        html.contains("class=\"tag-row\""),
        "declaration row: {html}"
    );
    // The desktop honours both of the route's actions, so neither is declared
    // unsupported.
    assert!(
        html.contains("data-action=\"terminal\""),
        "terminal: {html}"
    );
    assert!(html.contains("data-action=\"new-pod\""), "create: {html}");
    assert!(
        !html.contains("data-unsupported=\"terminal\"")
            && !html.contains("data-unsupported=\"cluster-mutation\""),
        "an in-process host advertises terminal and mutations: {html}"
    );
    assert!(
        !html.contains("not-found") && !html.contains("browser preview image"),
        "the old placeholder vocabulary is gone: {html}"
    );
}

#[test]
fn a_route_with_no_surface_renders_its_declared_empty_state() {
    let _host = desktop_host();
    let html = support::mount_html(|| with_action("/cluster", desktop_sections(), true), || {});

    assert!(
        html.contains("data-state=\"empty\""),
        "the body declares itself empty: {html}"
    );
    assert!(
        html.contains("This host has no node inventory on this route."),
        "the empty copy names the route: {html}"
    );
    assert!(
        html.contains("class=\"panel-footer\""),
        "the panel keeps its footer: {html}"
    );
    assert!(html.contains("Showing 0 of 0"), "row count: {html}");
    assert!(html.contains("class=\"pager\""), "pager: {html}");
    // No namespaces to filter by is declared, not silently absent.
    assert!(
        html.contains("data-empty=\"namespaces\""),
        "empty filter row: {html}"
    );
    assert!(
        html.contains("data-unsupported=\"namespace-filter\""),
        "the filter declares why it is inert: {html}"
    );
    assert!(
        html.contains("class=\"tag\"") && html.contains("context switching: on"),
        "the declaration row states the host's facts: {html}"
    );
}

#[test]
fn the_browser_profile_declares_what_it_cannot_do() {
    let _host = browser_host();
    let html = support::mount_html(
        || with_action("/workloads", browser_sections(), true),
        || {},
    );

    // The route's own chrome renders on the browser profile too.
    assert!(html.contains("class=\"page-head\""), "page head: {html}");
    assert!(html.contains("<h1>Workloads</h1>"), "title: {html}");
    assert!(
        html.contains("class=\"tag-row\""),
        "declaration row: {html}"
    );
    // A server-side gateway has no terminal and no writes: both are declared.
    assert!(
        html.contains("data-unsupported=\"terminal\""),
        "terminal is declared unsupported: {html}"
    );
    assert!(
        html.contains("data-unsupported=\"cluster-mutation\""),
        "writes are declared unsupported: {html}"
    );
    assert!(
        html.contains("terminal: off") && html.contains("cluster changes: off"),
        "the chips state the same facts: {html}"
    );
    assert!(
        html.contains("disabled"),
        "a declared control is not clickable: {html}"
    );
}

#[test]
fn a_capability_the_host_advertises_but_did_not_wire_is_declared_unwired() {
    let _host = desktop_host();
    let html = support::mount_html(
        || with_action("/workloads", desktop_sections(), false),
        || {},
    );

    assert!(
        html.contains("data-unsupported=\"unwired\""),
        "an unwired action is declared, never a dead button: {html}"
    );
    assert!(html.contains("disabled"), "and not clickable: {html}");
}

#[test]
fn the_hosts_own_surface_replaces_the_empty_state() {
    let _host = browser_host();
    let html = support::mount_html(
        || {
            let sections = browser_sections();
            rsx! {
                RouteView {
                    route: "/".to_string(),
                    sections,
                    busy: true,
                    content: rsx! { section { class: "panel", "data-surface": "overview", "host surface" } },
                }
            }
        },
        || {},
    );

    assert!(
        html.contains("data-surface=\"overview\"") && html.contains("host surface"),
        "the host's body renders: {html}"
    );
    assert!(
        !html.contains("data-state=\"empty\""),
        "a filled route has no empty state: {html}"
    );
    assert!(
        html.contains("class=\"spinner\""),
        "an in-flight route shows the design's spinner: {html}"
    );
}

#[test]
fn every_declared_capability_has_its_own_descriptor_name() {
    let _host = desktop_host();
    let ids: Vec<&str> = RouteCapability::ALL
        .iter()
        .map(|capability| capability.id())
        .collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        RouteCapability::ALL.len(),
        "every capability is named by the descriptor's own vocabulary: {ids:?}"
    );
    // The chip text is the machine fact, the way the design's tags read.
    assert_eq!(
        openkite_ui::components::route_views::capability_tag(RouteCapability::Terminal),
        "terminal: on"
    );
}
