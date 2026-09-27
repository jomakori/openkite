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
    // Trailing slash on a core route path.
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
    // Leading slashes stripped; empty segments dropped.
    assert_eq!(
        route_from_path("//a///b//"),
        Route::Plugin {
            path: vec!["a".into(), "b".into()],
        }
    );
}
