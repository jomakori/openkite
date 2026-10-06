//! Integration tests for the router's path → Route resolution.
//!
//! `route_from_path` is the pure seam behind the `OPENKITE_ROUTE` test
//! hook (boot the app directly onto a route) and the render-target
//! resolver. Core routes map 1:1 from their `#[route]` paths; anything
//! else falls through to the plugin wildcard.
//!
//! What the *console view* shows is a separate question, and one contract
//! answers it on both hosts: [`console_route`] resolves the address against
//! the sidebar model this host renders, the same call the browser host makes
//! for a request path (OKT-125).

use openkite::plugin_api::{PluginRegistration, RegistrationStore, SidebarItem};
use openkite::router::{console_route, route_from_path, Route};
use openkite::shell::{core_nav, sidebar_model, ShellSection};

#[test]
fn core_routes_map_from_paths() {
    assert_eq!(route_from_path("/"), Route::Home {});
    assert_eq!(route_from_path("/home"), Route::Home {});
    assert_eq!(route_from_path("/cluster"), Route::Cluster {});
    assert_eq!(route_from_path("/workloads"), Route::Workloads {});
    assert_eq!(route_from_path("/logs"), Route::Logs {});
    assert_eq!(route_from_path("/terminal"), Route::Terminal {});
    assert_eq!(route_from_path("/config"), Route::Config {});
}

#[test]
fn empty_and_whitespace_resolve_to_home() {
    assert_eq!(route_from_path(""), Route::Home {});
    assert_eq!(route_from_path("  "), Route::Home {});
    assert_eq!(route_from_path("///"), Route::Home {});
}

#[test]
fn leading_trailing_slashes_are_tolerated() {
    assert_eq!(route_from_path("/cluster/"), Route::Cluster {});
}

#[test]
fn unknown_paths_fall_through_to_plugin_wildcard() {
    assert_eq!(
        route_from_path("/argocd"),
        Route::Plugin {
            path: vec!["argocd".into()],
        }
    );
}

#[test]
fn plugin_wildcard_preserves_multi_segment_path() {
    assert_eq!(
        route_from_path("/plugins/argocd/apps"),
        Route::Plugin {
            path: vec!["plugins".into(), "argocd".into(), "apps".into()],
        }
    );
    assert_eq!(
        route_from_path("//a///b//"),
        Route::Plugin {
            path: vec!["a".into(), "b".into()],
        }
    );
}

#[test]
fn routes_display_as_the_paths_the_sidebar_links_to() {
    // The shell marks a sidebar entry active by comparing the entry's route
    // with the current route's `Display`, so the two vocabularies have to
    // agree — and `Display` has to round-trip back through `route_from_path`.
    for (route, path) in [
        (Route::Home {}, "/"),
        (Route::Cluster {}, "/cluster"),
        (Route::Workloads {}, "/workloads"),
        (Route::Logs {}, "/logs"),
        (Route::Terminal {}, "/terminal"),
        (Route::Config {}, "/config"),
    ] {
        let displayed = route.to_string();
        assert_eq!(displayed, path);
        assert_eq!(route_from_path(&displayed), route);
    }

    let plugin = Route::Plugin {
        path: vec!["argocd".into(), "apps".into()],
    };
    assert_eq!(plugin.to_string(), "/argocd/apps");
    assert_eq!(route_from_path(&plugin.to_string()), plugin);
}

// ---------------------------------------------------------------------------
// The console view's route is the address (OKT-125)
// ---------------------------------------------------------------------------

/// The desktop's sidebar model: the core navigation block, terminal included
/// when the host can render one.
fn desktop_sections(terminal: bool) -> Vec<ShellSection> {
    vec![ShellSection {
        label: "Overview".into(),
        accent: None,
        items: core_nav(terminal),
    }]
}

/// The browser's sidebar model: the core sections, terminal excluded.
fn browser_sections() -> Vec<ShellSection> {
    sidebar_model(&RegistrationStore::default())
}

#[test]
fn the_console_view_route_is_the_address_resolved_against_the_hosts_model() {
    let sections = desktop_sections(true);
    // Every route this host's navigation carries resolves to itself, so the
    // chrome, the sidebar's active entry and the breadcrumbs all read the path
    // the address names.
    for route in [
        "/",
        "/cluster",
        "/workloads",
        "/logs",
        "/terminal",
        "/config",
    ] {
        assert_eq!(console_route(route, &sections), route, "{route}");
    }
    // The same console view for every spelling of the same address.
    assert_eq!(console_route("/workloads/", &sections), "/workloads");
    assert_eq!(console_route(" workloads ", &sections), "/workloads");
    assert_eq!(console_route("", &sections), "/");
    // A path the model does not carry is not a console view of its own: it
    // falls back to the landing route, the same fallback the browser host
    // applies to the same address.
    for unknown in ["/nonsense", "/workloads/pods/probe-pod", "/argocd/apps"] {
        assert_eq!(console_route(unknown, &sections), "/", "{unknown}");
    }
    // The terminal entry is host-gated: a host that cannot render one does not
    // carry it, so the address stops resolving to it.
    assert_eq!(console_route("/terminal", &desktop_sections(false)), "/");
}

#[test]
fn the_router_wildcard_is_not_a_second_console_view() {
    // This host's own router still hands an unknown path to the plugin
    // wildcard, which is what routes into the JS slot...
    assert_eq!(
        route_from_path("/argocd/apps"),
        Route::Plugin {
            path: vec!["argocd".into(), "apps".into()],
        }
    );
    // ...but the console view is the one contract's: an address the sidebar
    // model does not carry lands on the landing route here too.
    assert_eq!(console_route("/argocd/apps", &desktop_sections(true)), "/");
    // Once a plugin registers the route in the model, the same address is a
    // console view of its own — and it is the model that says so, not a table
    // this host writes down beside the crate's.
    let mut store = RegistrationStore::default();
    store.upsert(
        "argocd",
        PluginRegistration {
            sidebar: vec![SidebarItem {
                label: "Applications".into(),
                icon: String::new(),
                route: "/argocd/apps".into(),
            }],
            ..PluginRegistration::default()
        },
    );
    let sections = sidebar_model(&store);
    assert_eq!(console_route("/argocd/apps", &sections), "/argocd/apps");
}

#[test]
fn both_hosts_derive_the_shared_console_view_from_the_address() {
    let desktop = desktop_sections(true);
    let browser = browser_sections();
    // The four primary routes are the same console view on both hosts from the
    // same address.
    for path in ["/", "/cluster", "/workloads", "/config"] {
        assert_eq!(
            console_route(path, &desktop),
            console_route(path, &browser),
            "{path}"
        );
    }
    // /logs and /terminal are the desktop's own surfaces: the browser's model
    // does not carry them, so there the same address is the landing route.
    for host_only in ["/logs", "/terminal"] {
        assert_eq!(console_route(host_only, &desktop), host_only);
        assert_eq!(console_route(host_only, &browser), "/");
    }
}
