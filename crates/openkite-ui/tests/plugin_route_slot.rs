//! The plugin route slot is a declared capability (OKT-156).
//!
//! The wildcard route is owned by a plugin, and only one half of serving it is
//! genuine host machinery: the `document::eval` that mounts a JS bundle into the
//! webview. The crate owns the rest — the route chrome, the mount node the
//! evaluator fills, and the declaration of the capability the route needs — and
//! the host's evaluator arrives as `PluginRouteView`'s `evaluator` slot, which
//! renders only where `openkite_ui::runtime::plugin_route_can_render` says the
//! host serves plugin bundles.
//!
//! These mounts pin that split on both host profiles: the desktop's slot renders
//! with the evaluator inside it, the browser's route declares the surface it
//! does not have and the evaluator never mounts there. The eval's own behaviour
//! stays covered by the desktop E2E flow (`e2e/bridge/guard.sh`); the predicate
//! itself is pinned by tests/capability_gate.rs.

mod support;

use std::sync::Mutex;

use dioxus::prelude::*;
use openkite_api::capability::Capabilities;
use openkite_ui::components::route_views::{
    capability_tag, route_page, PluginRouteView, RouteCapability,
};
use openkite_ui::plugin_api::RegistrationStore;
use openkite_ui::runtime::set_published_capabilities;
use openkite_ui::shell::{core_nav, sidebar_model, ShellSection};

// The host descriptor is process-global, exactly like tests/route_views.rs:
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

/// Stands in for the desktop's `JsRouteEvaluator`: the host-only component that
/// evals the bundle into the mount node. It paints a marker, so the markup shows
/// whether the host's machinery rendered at all.
#[component]
fn HostEvaluator() -> Element {
    rsx! { span { "data-host-evaluator": "mounted" } }
}

#[test]
fn the_wildcard_route_declares_the_plugin_surface_it_needs() {
    let _host = desktop_host();
    let page = route_page("/argocd/apps", &desktop_sections());
    assert_eq!(page.route, "/argocd/apps");
    assert_eq!(
        page.declared_capabilities(),
        vec![RouteCapability::PluginRoute]
    );
    assert_eq!(
        capability_tag(RouteCapability::PluginRoute),
        "plugin routes: on"
    );
    assert_eq!(RouteCapability::PluginRoute.id(), "plugin-route");
}

#[test]
fn a_browser_host_declares_the_plugin_capability_off() {
    let _host = browser_host();
    let page = route_page("/argocd/apps", &desktop_sections());
    assert_eq!(
        page.declared_capabilities(),
        vec![RouteCapability::PluginRoute]
    );
    assert_eq!(
        capability_tag(RouteCapability::PluginRoute),
        "plugin routes: off"
    );
    // The declaration follows the host, not the route: it is the same list on
    // both profiles, read through the same predicate.
    assert_eq!(
        RouteCapability::PluginRoute.missing(),
        "This host serves no plugin bundles: a plugin route has nothing to render here."
    );
}

#[test]
fn the_desktop_profile_renders_the_mount_and_the_host_evaluator() {
    let _host = desktop_host();
    let html = support::mount_html(
        || {
            let sections = desktop_sections();
            rsx! {
                PluginRouteView {
                    route: "/argocd/apps".to_string(),
                    sections,
                    js_route: Some("/argocd/apps".to_string()),
                    evaluator: rsx! { HostEvaluator {} },
                }
            }
        },
        || {},
    );

    assert!(
        html.contains("data-route=\"/argocd/apps\""),
        "the crate renders the chrome for the plugin's path: {html}"
    );
    assert!(
        html.contains("class=\"js-route-slot\"")
            && html.contains("data-js-route-mount=\"/argocd/apps\""),
        "the crate renders the mount node the evaluator looks up: {html}"
    );
    assert!(
        html.contains("data-host-evaluator=\"mounted\""),
        "the host's evaluator renders inside the gated chrome: {html}"
    );
    assert!(
        html.contains("data-capability=\"plugin-route\"")
            && html.contains("plugin routes: on")
            && !html.contains("data-unsupported=\"plugin-route\""),
        "the declaration states the host's fact, with no unsupported marker: {html}"
    );
}

#[test]
fn the_browser_profile_declares_the_missing_plugin_surface() {
    let _host = browser_host();
    let html = support::mount_html(
        || {
            let sections = sidebar_model(&RegistrationStore::default());
            rsx! {
                PluginRouteView {
                    route: "/argocd/apps".to_string(),
                    sections,
                    js_route: Some("/argocd/apps".to_string()),
                    evaluator: rsx! { HostEvaluator {} },
                }
            }
        },
        || {},
    );

    assert!(
        html.contains("data-route=\"/argocd/apps\""),
        "the chrome is still the crate's: {html}"
    );
    assert!(
        html.contains("data-unsupported=\"plugin-route\"")
            && html.contains("data-state=\"unsupported\""),
        "the body declares what is missing instead of mounting a slot: {html}"
    );
    assert!(
        html.contains("This host serves no plugin bundles"),
        "the body carries the descriptor's sentence: {html}"
    );
    assert!(
        html.contains("plugin routes: off"),
        "and the tag row states the capability off: {html}"
    );
    assert!(
        !html.contains("data-js-route-mount"),
        "no mount node on a host that cannot fill it: {html}"
    );
    assert!(
        !html.contains("data-host-evaluator=\"mounted\""),
        "the host-only evaluator is gated out entirely: {html}"
    );
}

#[test]
fn a_host_with_the_surface_but_no_renderer_keeps_the_declared_empty_body() {
    let _host = desktop_host();
    let html = support::mount_html(
        || {
            let sections = desktop_sections();
            rsx! {
                PluginRouteView { route: "/argocd/apps".to_string(), sections }
            }
        },
        || {},
    );

    assert!(
        html.contains("data-state=\"empty\"")
            && html.contains("This host has no surface for this route."),
        "a path no plugin renders keeps the chrome's empty body: {html}"
    );
    assert!(
        !html.contains("data-js-route-mount"),
        "and mounts no node for it: {html}"
    );
    assert!(
        !html.contains("data-unsupported=\"plugin-route\"") && html.contains("plugin routes: on"),
        "the capability is advertised; only the route's renderer is missing: {html}"
    );
}
