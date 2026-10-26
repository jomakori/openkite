//! The overview dashboard's four states (OKT-203).
//!
//! Props in, markup out: each state must say a different thing, and a summary
//! with no tiles must never paint a grid of zeroes. `tests/design.rs` covers
//! the classes; `tests/overview_state.rs`-style host wiring is covered by the
//! browser gate's `?fixture=` states.

mod support;

use dioxus::prelude::*;
use openkite_ui::components::overview::{
    OverviewDashboard, OverviewSummary, OverviewTile, TileTone,
};

fn summary() -> OverviewSummary {
    OverviewSummary {
        context: "kind-openkite".to_string(),
        gateway: "server-side".to_string(),
        connection: "Connected".to_string(),
        tiles: vec![
            OverviewTile::new(
                "pods",
                "Pods",
                "40/42",
                "running of all pods",
                TileTone::Warn,
            ),
            OverviewTile::new("nodes", "Nodes", "3/3", "ready", TileTone::Ok),
        ],
    }
}

fn ready() -> Element {
    rsx! { OverviewDashboard { summary: summary(), state: "ready" } }
}

fn loading() -> Element {
    rsx! { OverviewDashboard { state: "loading" } }
}

fn empty() -> Element {
    rsx! { OverviewDashboard { summary: OverviewSummary::default(), state: "empty" } }
}

fn refused() -> Element {
    rsx! {
        OverviewDashboard {
            state: "error",
            error: "forbidden: the read token expired",
        }
    }
}

#[test]
fn the_ready_state_paints_one_tile_per_summary_tile() {
    let html = support::mount_html(ready, || {});
    assert!(html.contains("data-surface=\"overview\""), "{html}");
    assert!(html.contains("data-state=\"ready\""), "{html}");
    assert_eq!(html.matches("class=\"stat-tile\"").count(), 2, "{html}");
    assert!(html.contains("data-tile=\"pods\""), "{html}");
    assert!(html.contains("40/42"), "{html}");
    assert!(
        html.contains("kind-openkite · Connected · gateway server-side"),
        "{html}"
    );
    assert!(!html.contains("table-state"), "{html}");
}

#[test]
fn the_loading_state_does_not_paint_a_tile() {
    let html = support::mount_html(loading, || {});
    assert!(html.contains("data-state=\"loading\""), "{html}");
    assert!(html.contains("table-loading"), "{html}");
    assert!(html.contains("Reading the cluster"), "{html}");
    assert!(!html.contains("stat-tile"), "{html}");
}

#[test]
fn the_empty_state_says_the_read_returned_nothing() {
    let html = support::mount_html(empty, || {});
    assert!(html.contains("data-state=\"empty\""), "{html}");
    assert!(
        html.contains("returned no pods, nodes, deployments or services"),
        "{html}"
    );
    assert!(!html.contains("stat-tile"), "{html}");
}

#[test]
fn the_error_state_carries_the_refusal() {
    let html = support::mount_html(refused, || {});
    assert!(html.contains("data-state=\"error\""), "{html}");
    assert!(html.contains("table-error"), "{html}");
    assert!(html.contains("forbidden: the read token expired"), "{html}");
    assert!(!html.contains("stat-tile"), "{html}");
}

#[test]
fn state_precedence_is_pending_then_refusal_then_absence() {
    let full = summary();
    let none = OverviewSummary::default();
    assert_eq!(full.state(true, None), "loading");
    assert_eq!(full.state(false, Some("boom")), "error");
    assert_eq!(none.state(false, None), "empty");
    assert_eq!(full.state(false, None), "ready");
    // A pending read is not an error yet, even if the last one failed.
    assert_eq!(none.state(true, Some("boom")), "loading");
}

#[test]
fn every_tone_maps_to_a_dot_the_stylesheet_defines() {
    let css = include_str!("../assets/main.css");
    for tone in [
        TileTone::Neutral,
        TileTone::Ok,
        TileTone::Warn,
        TileTone::Bad,
    ] {
        for class in tone.dot_class().split(' ') {
            let selector = format!(".{class}");
            assert!(css.contains(&selector), "main.css is missing {selector}");
        }
        // `bad` reads as the shell's error dot, so the two vocabularies agree.
        assert!(["neutral", "ok", "warn", "bad"].contains(&tone.as_str()));
    }
}
