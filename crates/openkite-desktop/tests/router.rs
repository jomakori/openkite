//! Integration tests for the router's path → Route resolution.
//!
//! `route_from_path` is the pure seam behind the `OPENKITE_ROUTE` test
//! hook (boot the app directly onto a route) and the render-target
//! resolver. Core routes map 1:1 from their `#[route]` paths; anything
//! else falls through to the plugin wildcard.

use openkite::router::{route_from_path, Route};

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
