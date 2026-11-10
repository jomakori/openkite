//! Cross-host parity snapshot: the shared half of gate G3.
//!
//! The gate grades *computed* evidence — the class chain of the declared
//! contract nodes and the resolved token values — for the four core routes on
//! both hosts. It never compares pixels: those belong to the per-host baselines
//! (G2), whose screenshots legitimately differ in data, not in design.
//!
//! This module owns the parts both the desktop hook and the browser extractor
//! have to agree on: the extractor script itself ([`SNAPSHOT_JS`], embedded
//! from `assets/parity-snapshot.js` so a single file can never drift between
//! the two sides), the graded route table, and the snapshot file naming. It is
//! pure and host-agnostic; the desktop glue lives in `openkite-desktop`'s
//! `parity` module and the browser side in `e2e/visual/browser/capture.mjs`.
//!
//! See `e2e/parity/README.md` for the gate, its allowlist and its boundaries.

use std::path::{Path, PathBuf};

/// The extractor evaluated on both hosts. Its last expression is a JSON
/// string, which is what [`document::eval`] hands back on the desktop and what
/// `Runtime.evaluate` returns in the browser.
pub const SNAPSHOT_JS: &str = include_str!("../assets/parity-snapshot.js");

/// The graded frame, in CSS pixels: the desktop window's default inner size.
/// The gate does not require equal inner sizes (a window manager may take a
/// title bar) but it does require both hosts to sit in the same breakpoint
/// band, so the same media-query branch paints on each.
pub const GRADED_VIEWPORT: (u32, u32) = (800, 600);

/// Core routes, with the file slug the two sides share. Overlays and
/// desktop-only routes stay out of scope, exactly as in the visual baselines.
pub const ROUTES: &[(&str, &str)] = &[
    ("/", "home"),
    ("/cluster", "cluster"),
    ("/workloads", "workloads"),
    ("/config", "config"),
];

/// The route as the snapshot records it (`location.pathname`): a leading
/// slash, no trailing slash, `/` for the root.
pub fn normalise_route(path: &str) -> &str {
    let trimmed = path.trim();
    let trimmed = trimmed
        .strip_suffix('/')
        .filter(|p| !p.is_empty())
        .unwrap_or(trimmed);
    if trimmed.is_empty() {
        "/"
    } else {
        trimmed
    }
}

/// The shared file slug for a route, or `None` when the route is not graded.
pub fn slug_for(route: &str) -> Option<&'static str> {
    let route = normalise_route(route);
    ROUTES
        .iter()
        .find(|(path, _)| *path == route)
        .map(|(_, slug)| *slug)
}

/// `<dir>/<slug>.<host>.json` — the desktop and browser sides write into
/// separate directories, so the host tag keeps the artifact self-describing.
pub fn snapshot_path(dir: &Path, route: &str, host: &str) -> Option<PathBuf> {
    slug_for(route).map(|slug| dir.join(format!("{slug}.{host}.json")))
}

/// Settle time after mount before the snapshot is taken. Layout has to have
/// run at least once for computed values to be final; `OPENKITE_PARITY_SETTLE_MS`
/// shortens it for local runs and the clamp keeps a typo from stalling CI.
pub fn settle_ms(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .map(|ms| ms.clamp(50, 10_000))
        .unwrap_or(1200)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_normalise_to_the_pathname_the_snapshot_records() {
        assert_eq!(normalise_route("/"), "/");
        assert_eq!(normalise_route(""), "/");
        assert_eq!(normalise_route("  /workloads/  "), "/workloads");
        assert_eq!(normalise_route("workloads"), "workloads");
    }

    #[test]
    fn every_graded_route_has_a_unique_slug() {
        let mut slugs: Vec<&str> = ROUTES.iter().map(|(_, slug)| *slug).collect();
        let count = slugs.len();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), count, "duplicate slug in ROUTES");
        assert_eq!(count, 4, "the gate grades the four core routes");
    }

    #[test]
    fn slugs_cover_exactly_the_core_routes() {
        assert_eq!(slug_for("/"), Some("home"));
        assert_eq!(slug_for("/cluster/"), Some("cluster"));
        assert_eq!(slug_for("/workloads"), Some("workloads"));
        assert_eq!(slug_for("/config"), Some("config"));
        // Overlays and desktop-only routes are out of scope.
        assert_eq!(slug_for("/logs"), None);
        assert_eq!(slug_for("/terminal"), None);
        assert_eq!(slug_for("/plugins/some-plugin"), None);
    }

    #[test]
    fn snapshot_paths_are_host_tagged_and_route_slugged() {
        let dir = Path::new("/tmp/parity");
        assert_eq!(
            snapshot_path(dir, "/workloads", "desktop"),
            Some(dir.join("workloads.desktop.json"))
        );
        assert_eq!(
            snapshot_path(dir, "/", "web"),
            Some(dir.join("home.web.json"))
        );
        assert_eq!(snapshot_path(dir, "/logs", "web"), None);
    }

    #[test]
    fn settle_time_defaults_and_clamps() {
        assert_eq!(settle_ms(None), 1200);
        assert_eq!(settle_ms(Some("250")), 250);
        assert_eq!(settle_ms(Some("0")), 50);
        assert_eq!(settle_ms(Some("999999")), 10_000);
        assert_eq!(settle_ms(Some("nonsense")), 1200);
    }

    #[test]
    fn the_embedded_extractor_is_the_shared_asset() {
        // One JSON, one code path, two transports: the desktop host receives
        // `dioxus.send`, the browser host receives the evaluated value.
        assert!(SNAPSHOT_JS.contains("var json = JSON.stringify(snapshot);"));
        assert!(SNAPSHOT_JS.contains("dioxus.send(json)"));
        assert!(SNAPSHOT_JS.contains("return json;"));
        assert!(SNAPSHOT_JS.contains("var SCHEMA = 1;"));
        // The graded frame must be the band the extractor reports as `medium`.
        assert!(GRADED_VIEWPORT.0 > 767 && GRADED_VIEWPORT.0 <= 1024);
    }
}
