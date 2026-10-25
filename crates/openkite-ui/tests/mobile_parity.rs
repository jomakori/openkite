//! The ≤767px chrome close-out (OKT-197, spec T9 + T11).
//!
//! T9 finished the bottom bar into the mockup's shape: one cell per
//! destination, each with the design's glyph, plus the Logs toggle and the
//! drawer's Menu tab on a single non-wrapping row. T11 gave the log surface a
//! bottom sheet — the mockup's `.log-panel` parked below the bar, opened from
//! the bar's Logs tab or its own handle, dragged down to dismiss.
//!
//! These mount the crate's components headless and read the stylesheet and the
//! two component sources, the same way `design.rs` and `shell_chrome.rs` do:
//! the parity claim is a DOM + stylesheet claim, not a screenshot claim (the
//! captures are `e2e/visual`, refreshed separately).

mod support;

use dioxus::prelude::*;
use openkite_ui::components::logs::LogSheet;
use openkite_ui::components::shell::{AppShell, ClusterInfo};
use openkite_ui::shell::{core_sections_with_counts, NavCounts, StatusBarEntry};
use openkite_ui::touch::{
    resolve_sheet_drag, SheetDrag, SHEET_DISMISS_MAX_PX, SHEET_DISMISS_MIN_PX,
};

const STYLESHEET: &str = openkite_ui::MAIN_CSS;
/// The shell that renders the sheet's siblings, read as text: the keybind and
/// focus contracts are scripts, and a broken selector there is silent.
const SHELL_SOURCE: &str = include_str!("../src/components/shell.rs");
/// The sheet's own source, for its drag contract.
const LOGS_SOURCE: &str = include_str!("../src/components/logs.rs");

/// A connected shell on the workloads route: three destinations, so the bar's
/// Logs toggle and Menu tab are the fourth and fifth cells.
fn shell_root() -> Element {
    rsx! {
        AppShell {
            sections: core_sections_with_counts(&NavCounts {
                nodes: Some(8),
                pods: Some(124),
                deployments: Some(37),
                services: Some(29),
                config_maps: Some(46),
            }),
            current_route: "/workloads",
            cluster: Some(ClusterInfo {
                label: "prod-us-east-1".into(),
                detail: None,
                connected: true,
            }),
            status: Vec::<StatusBarEntry>::new(),
            div { "route outlet" }
        }
    }
}

/// The sheet on its own, in one state.
fn sheet_root(open: bool) -> Element {
    rsx! {
        LogSheet {
            open,
            on_toggle: EventHandler::new(|_: ()| {}),
            on_close: EventHandler::new(|_: ()| {}),
        }
    }
}

fn sheet_closed() -> Element {
    sheet_root(false)
}

fn sheet_open() -> Element {
    sheet_root(true)
}

/// The one bar the shell renders.
fn bottom_bar(html: &str) -> &str {
    let start = html
        .find("class=\"bottom-nav\"")
        .expect("the bar is in the shell");
    let end = html[start..].find("</nav>").expect("the bar closes");
    &html[start..start + end]
}

/// The declarations of the first rule whose selector list carries `selector`.
fn rule<'a>(css: &'a str, selector: &str) -> &'a str {
    for chunk in css.split('}') {
        let Some((selectors, body)) = chunk.split_once('{') else {
            continue;
        };
        let selectors = selectors.rsplit(['\n', ';']).next().unwrap_or(selectors);
        if selectors
            .split(',')
            .any(|candidate| candidate.trim() == selector)
        {
            return body;
        }
    }
    panic!("no rule for `{selector}`");
}

#[test]
fn every_destination_and_both_toggles_get_one_bar_cell() {
    let html = support::mount_html(shell_root, || {});
    let bar = bottom_bar(&html);
    let cells = bar.matches("<a class=\"bottom-tab").count()
        + bar.matches("<button class=\"bottom-tab").count();
    assert_eq!(cells, 5, "three destinations + Logs + Menu: {bar}");
    // The bar's grid takes a cell per tab rather than the four it used to
    // hard-code, so a fifth tab cannot wrap or overflow the row.
    let grid = rule(STYLESHEET, ".bottom-tabs");
    assert!(
        !grid.contains("repeat(4"),
        "the bar must not hard-code four columns: {grid}"
    );
    assert!(
        grid.contains("grid-auto-flow: column") || grid.contains("grid-template-columns"),
        "the bar lays its cells out on one axis: {grid}"
    );
}

#[test]
fn every_bar_cell_carries_the_designs_glyph() {
    let html = support::mount_html(shell_root, || {});
    let bar = bottom_bar(&html);
    let cells = bar.matches("<a class=\"bottom-tab").count()
        + bar.matches("<button class=\"bottom-tab").count();
    let glyphs = bar.matches("<svg class=\"icon\"").count();
    assert_eq!(glyphs, cells, "each cell has a glyph: {bar}");
    // The icon chain is styled: 19px muted, accent when its tab is current,
    // the plugin's accent for an Argo CD tab.
    let icon = rule(STYLESHEET, ".bottom-tab .icon");
    assert!(
        icon.contains("19px"),
        "the mockup's tab glyph is 19px: {icon}"
    );
    assert!(icon.contains("var(--muted)"), "muted at rest: {icon}");
    assert!(rule(STYLESHEET, ".bottom-tab.active .icon").contains("var(--accent)"));
    assert!(rule(STYLESHEET, ".bottom-tab.argo.active .icon").contains("var(--argo)"));
}

#[test]
fn the_logs_cell_is_a_toggle_not_a_route() {
    let html = support::mount_html(shell_root, || {});
    let bar = bottom_bar(&html);
    assert!(
        bar.contains("aria-label=\"Logs\""),
        "the Logs cell is named: {bar}"
    );
    assert!(
        bar.contains("class=\"bottom-tab\"") && bar.contains("aria-expanded=\"false\""),
        "the Logs toggle reports its state: {bar}"
    );
    // It toggles the sheet; it must not navigate away from the current route.
    assert!(
        !bar.contains("href=\"/logs\""),
        "the Logs cell is not a destination in the bar: {bar}"
    );
}

#[test]
fn the_sheet_is_parked_until_it_is_opened() {
    let closed = support::mount_html(sheet_closed, || {});
    assert!(
        closed.contains("class=\"log-panel log-sheet\""),
        "the sheet is the mockup's log panel: {closed}"
    );
    assert!(closed.contains("class=\"log-handle\""));
    assert!(closed.contains("aria-expanded=\"false\""));

    let open = support::mount_html(sheet_open, || {});
    assert!(
        open.contains("class=\"log-panel log-sheet open\""),
        "opening sets the mockup's `.open`: {open}"
    );
    assert!(open.contains("aria-expanded=\"true\""));
    // The handle is a real control, not a decoration: 44px tall, named.
    let handle = rule(STYLESHEET, ".log-handle");
    assert!(
        handle.contains("min-height: 44px"),
        "the handle keeps the touch floor: {handle}"
    );
}

#[test]
fn the_sheet_stylesheet_keeps_the_mockup_geometry() {
    // Hidden everywhere, including the sheet's own `display` while parked.
    let base = rule(STYLESHEET, ".log-sheet");
    assert!(
        base.contains("display: none"),
        "the sheet is mobile-only chrome: {base}"
    );
    assert!(rule(STYLESHEET, ".log-scrim").contains("display: none"));
    // The sheet itself: fixed above the bar, translated off-screen until open.
    let mobile = STYLESHEET
        .rsplit("max-width: 767px")
        .next()
        .expect("the mobile block");
    for expected in [
        ".log-sheet {",
        ".log-sheet.open {",
        ".log-scrim {",
        ".log-scrim.show {",
        "position: fixed",
        "transform: translateY(",
    ] {
        assert!(
            mobile.contains(expected),
            "the ≤767px block is missing `{expected}`"
        );
    }
    // Open is the rest position; parked is the same distance plus the bar.
    let open = rule(STYLESHEET, ".log-sheet.open");
    assert!(open.contains("translateY(0)"), "open sits at rest: {open}");
    // Parked is hidden, not merely off-screen: a closed sheet must not take
    // focus or announce itself (verified in Chromium: 0 focusable children).
    assert!(
        mobile.contains("visibility: hidden"),
        "a parked sheet is out of the tab order"
    );
    assert!(
        open.contains("visibility: visible"),
        "opening publishes the sheet: {open}"
    );
}

#[test]
fn the_bar_keeps_the_touch_floor() {
    let tab = rule(STYLESHEET, ".bottom-tab");
    assert!(
        tab.contains("min-width: 44px"),
        "a fifth cell must stay tappable at 320px: {tab}"
    );
}

#[test]
fn a_drag_past_the_threshold_dismisses_and_a_short_one_snaps_back() {
    // A short sheet asks for the floor, not a quarter of nothing.
    assert_eq!(
        resolve_sheet_drag(SHEET_DISMISS_MIN_PX - 1.0, 120.0),
        SheetDrag::Snap
    );
    assert_eq!(
        resolve_sheet_drag(SHEET_DISMISS_MIN_PX, 120.0),
        SheetDrag::Dismiss
    );
    // A mid-height sheet asks for a quarter of its own height…
    assert_eq!(resolve_sheet_drag(69.0, 280.0), SheetDrag::Snap);
    assert_eq!(resolve_sheet_drag(70.0, 280.0), SheetDrag::Dismiss);
    // …but a tall one asks for no more than a comfortable flick.
    assert_eq!(resolve_sheet_drag(95.0, 900.0), SheetDrag::Snap);
    assert_eq!(
        resolve_sheet_drag(SHEET_DISMISS_MAX_PX, 900.0),
        SheetDrag::Dismiss
    );
    // Upward travel never dismisses.
    assert_eq!(resolve_sheet_drag(-200.0, 430.0), SheetDrag::Snap);
}

#[test]
fn the_handle_drag_and_the_keys_use_the_sheet_selectors() {
    for expected in [
        "'.log-sheet .log-handle'",
        "'pointerdown'",
        "'pointermove'",
        "'pointerup'",
        "dioxus.send('close')",
        "setPointerCapture",
    ] {
        assert!(
            LOGS_SOURCE.contains(expected),
            "the drag contract is missing `{expected}`"
        );
    }
    for expected in [
        "'.log-sheet.open'",
        "'Escape'",
        "'Tab'",
        "LOG_SHEET_RESTORE_JS",
    ] {
        assert!(
            SHELL_SOURCE.contains(expected),
            "the sheet's key contract is missing `{expected}`"
        );
    }
}
