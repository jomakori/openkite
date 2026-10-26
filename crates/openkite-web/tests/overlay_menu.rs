//! The overlay menu primitive and the account menu built on it: the placement
//! rules, the SSR markup both hosts paint, and the stylesheet's scoping.
//!
//! The browser-only half (measurement, Escape, focus return) is exercised in
//! `e2e/browser/account-menu.mjs`; what a document can prove without a DOM is
//! proved here.

use dioxus::prelude::*;
use openkite_web::account_menu::{AccountMenu, PANEL_ID, TRIGGER_SELECTOR};
use openkite_web::overlay_menu::{
    place, Align, Anchor, Panel, Placement, Side, Viewport, GAP, MARGIN,
};
use openkite_web::ssr::AccountIdentity;

const CSS: &str = include_str!("../assets/overlay-menu.css");

fn anchor(x: f64, y: f64, width: f64, height: f64) -> Anchor {
    Anchor {
        x,
        y,
        width,
        height,
    }
}

fn viewport(width: f64, height: f64) -> Viewport {
    Viewport { width, height }
}

fn panel(width: f64, height: f64) -> Panel {
    Panel { width, height }
}

#[test]
fn a_menu_with_room_below_opens_below() {
    let placed = place(
        anchor(1370.0, 8.0, 44.0, 44.0),
        panel(232.0, 180.0),
        viewport(1440.0, 900.0),
        Align::End,
    );
    assert_eq!(placed.side, Side::Below);
    assert_eq!(placed.top, 8.0 + 44.0 + GAP);
    // End-aligned: the panel's right edge meets the trigger's right edge.
    assert_eq!(placed.left, 1370.0 + 44.0 - 232.0);
}

#[test]
fn a_menu_with_no_room_below_flips_above() {
    let placed = place(
        anchor(1400.0, 860.0, 44.0, 30.0),
        panel(232.0, 200.0),
        viewport(1440.0, 900.0),
        Align::End,
    );
    assert_eq!(placed.side, Side::Above);
    assert_eq!(placed.top, 860.0 - GAP - 200.0);
}

#[test]
fn a_menu_that_fits_neither_way_stays_below() {
    let placed = place(
        anchor(100.0, 300.0, 40.0, 40.0),
        panel(232.0, 800.0),
        viewport(1440.0, 600.0),
        Align::End,
    );
    assert_eq!(placed.side, Side::Below);
    assert_eq!(placed.top, 300.0 + 40.0 + GAP);
}

#[test]
fn the_panel_never_leaves_the_viewport() {
    // A trigger against the right edge, a panel wider than the space left of it.
    let placed = place(
        anchor(1430.0, 8.0, 44.0, 44.0),
        panel(400.0, 200.0),
        viewport(1440.0, 900.0),
        Align::End,
    );
    assert!(placed.left >= MARGIN, "{placed:?}");
    assert!(placed.left + 400.0 <= 1440.0 - MARGIN, "{placed:?}");

    // Start alignment near the left edge clamps the other way.
    let placed = place(
        anchor(2.0, 8.0, 44.0, 44.0),
        panel(232.0, 200.0),
        viewport(1440.0, 900.0),
        Align::Start,
    );
    assert_eq!(placed.left, MARGIN);
}

#[test]
fn start_alignment_hangs_off_the_triggers_leading_edge() {
    let placed = place(
        anchor(300.0, 8.0, 44.0, 44.0),
        panel(232.0, 200.0),
        viewport(1440.0, 900.0),
        Align::Start,
    );
    assert_eq!(placed.left, 300.0);
}

#[test]
fn a_panel_wider_than_the_viewport_sits_at_the_margin() {
    let placed = place(
        anchor(700.0, 8.0, 44.0, 44.0),
        panel(2000.0, 200.0),
        viewport(1200.0, 800.0),
        Align::End,
    );
    assert_eq!(placed.left, MARGIN);
}

#[test]
fn the_placed_panel_is_positioned_by_style() {
    let placed = place(
        anchor(1400.0, 8.0, 44.0, 44.0),
        panel(232.0, 180.0),
        viewport(1440.0, 900.0),
        Align::End,
    );
    let style = placed.style();
    assert!(style.contains("position: fixed"), "{style}");
    assert!(
        style.contains(&format!("top: {:.1}px", placed.top)),
        "{style}"
    );
    assert!(
        style.contains(&format!("left: {:.1}px", placed.left)),
        "{style}"
    );
    assert_eq!(placed.side.data_value(), "below");

    let default: Placement = Default::default();
    assert_eq!(default.side, Side::Below);
}

fn render(seed: fn() -> Element) -> String {
    let mut dom = VirtualDom::new(seed);
    dom.rebuild(&mut dioxus::core::NoOpMutations);
    dioxus_ssr::pre_render(&dom)
}

#[test]
fn the_account_menu_renders_the_hosts_identity() {
    let html = render(|| {
        rsx! {
            AccountMenu {
                account: AccountIdentity::for_context("e2e"),
                on_refresh: EventHandler::default(),
                initially_open: true,
            }
        }
    });

    assert!(html.contains(r#"id="openkite-account-menu""#), "{html}");
    assert!(html.contains(r#"role="menu""#), "{html}");
    assert!(html.contains(r#"aria-label="Account""#), "{html}");
    assert!(html.contains(r#"data-overlay-menu"#), "{html}");
    assert!(html.contains("overlay-menu-header"), "{html}");
    assert!(html.contains(">e2e<"), "{html}");
    assert!(html.contains("kubeconfig context"), "{html}");
    assert!(html.contains(">Refresh<"), "{html}");
}

#[test]
fn sign_out_is_declared_and_disabled_with_its_reason() {
    let html = render(|| {
        rsx! {
            AccountMenu {
                account: AccountIdentity::for_service_account("openkite"),
                on_refresh: EventHandler::default(),
                initially_open: true,
            }
        }
    });

    let sign_out = html
        .split("Sign out")
        .next()
        .and_then(|head| head.rsplit("<button").next())
        .expect("the sign-out button");
    assert!(sign_out.contains("disabled"), "{sign_out}");
    assert!(sign_out.contains(r#"aria-disabled="true""#), "{sign_out}");
    assert!(sign_out.contains("no session to end"), "{sign_out}");
    assert!(html.contains("in-cluster ServiceAccount"), "{html}");
}

#[test]
fn the_trigger_is_the_shared_topbars_avatar() {
    assert_eq!(TRIGGER_SELECTOR, ".topbar .avatar");
    assert_eq!(PANEL_ID, "openkite-account-menu");
}

#[test]
fn the_panel_is_hidden_until_it_is_measured() {
    // A server render has no viewport to measure against: the panel ships
    // positioned=false so the stylesheet can keep it invisible for the frame
    // between hydration and the browser's measurement.
    let html = render(|| {
        rsx! {
            AccountMenu {
                account: AccountIdentity::for_context("e2e"),
                on_refresh: EventHandler::default(),
                initially_open: true,
            }
        }
    });
    assert!(html.contains(r#"data-positioned="false""#), "{html}");
    assert!(html.contains("overlay-menu-scrim"), "{html}");
}

#[test]
fn every_selector_in_the_sheet_is_scoped_to_the_primitive() {
    let mut selectors = 0;
    for block in CSS.split('{') {
        let selector = match block.rsplit('}').next() {
            Some(selector) => selector
                .lines()
                .filter(|line| {
                    !line.trim_start().starts_with("/*") && !line.trim_start().starts_with('*')
                })
                .collect::<Vec<_>>()
                .join(" "),
            None => continue,
        };
        let selector = selector.trim();
        if selector.is_empty() || selector.starts_with("/*") {
            continue;
        }
        for one in selector.split(',') {
            let one = one.trim();
            if one.is_empty() {
                continue;
            }
            selectors += 1;
            assert!(
                one.contains("[data-overlay-menu]"),
                "unscoped selector {one:?} in overlay-menu.css"
            );
        }
    }
    assert!(selectors > 8, "only {selectors} selectors found");
}

#[test]
fn every_class_the_panel_paints_is_declared_by_the_sheet() {
    let html = render(|| {
        rsx! {
            AccountMenu {
                account: AccountIdentity::for_context("e2e"),
                on_refresh: EventHandler::default(),
                initially_open: true,
            }
        }
    });

    let mut classes = Vec::new();
    for fragment in html.split("class=\"").skip(1) {
        let list = fragment.split('"').next().unwrap_or_default();
        classes.extend(list.split_whitespace().map(str::to_string));
    }
    assert!(!classes.is_empty(), "{html}");

    let missing: Vec<_> = classes
        .iter()
        .filter(|class| !CSS.contains(&format!(".{class}")))
        .collect();
    assert!(missing.is_empty(), "classes with no rule: {missing:?}");
}
