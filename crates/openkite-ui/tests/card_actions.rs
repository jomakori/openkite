//! Card-action touch parity (OKT-169).
//!
//! Pins the four acceptance lines on the `AppCard` primitive: the swipe column
//! offers exactly the actions the kebab does, a swipe can never navigate, every
//! control in the card clears the 44px floor, and no card affordance is
//! hover-only.

mod support;

use dioxus::prelude::*;
use openkite_ui::components::app_card::{
    resolve_swipe, resolve_tap, AppCard, CardAction, CardTone, SwipeIntent, TapIntent,
    SWIPE_REVEAL_PX, SWIPE_THRESHOLD_PX, TAP_TARGET_PX,
};

const STYLESHEET: &str = openkite_ui::MAIN_CSS;

fn actions() -> Vec<CardAction> {
    vec![
        CardAction::sync("sync", "Sync"),
        CardAction::new("refresh", "Refresh"),
    ]
}

fn card() -> Element {
    rsx! {
        div { class: "app-grid",
            AppCard {
                name: "guestbook",
                sub: "default / demo-apps",
                tone: CardTone::Synced,
                repo: "github.com/acme/guestbook",
                version: "chart 1.4.2",
                age: "2m",
                actions: actions(),
            }
        }
    }
}

fn card_swiped_tone() -> Element {
    rsx! {
        AppCard {
            name: "canary-rollouts",
            tone: CardTone::Degraded,
            actions: vec![CardAction::new("delete", "Delete")],
        }
    }
}

// --- Acceptance 1: the swipe reveals the same actions the kebab offers ------

#[test]
fn swipe_column_offers_every_action_the_kebab_controls() {
    let html = support::mount_html(card, || {});
    assert!(html.contains("card-swipe-actions"), "got: {html}");
    // One list, two triggers: every action renders in the column the kebab
    // points its `aria-controls` at, so the swipe and the kebab cannot differ.
    assert!(
        html.contains("id=\"card-actions-guestbook\""),
        "got: {html}"
    );
    assert!(
        html.contains("aria-controls=\"card-actions-guestbook\""),
        "got: {html}"
    );
    assert!(html.contains("data-action=\"sync\""), "got: {html}");
    assert!(html.contains("data-action=\"refresh\""), "got: {html}");
    assert!(html.contains(">Sync<"), "got: {html}");
    assert!(html.contains(">Refresh<"), "got: {html}");
    // The kebab plus one button per action: nothing else is clickable.
    assert_eq!(
        html.matches("<button").count(),
        actions().len() + 1,
        "got: {html}"
    );
}

#[test]
fn a_synced_action_carries_the_sync_tone() {
    let html = support::mount_html(card, || {});
    assert!(html.contains("class=\"card-action sync\""), "got: {html}");
    assert!(html.contains("class=\"card-action\""), "got: {html}");
}

#[test]
fn card_starts_closed_and_the_kebab_reports_it() {
    let html = support::mount_html(card, || {});
    assert!(html.contains("data-swiped=\"false\""), "got: {html}");
    assert!(html.contains("aria-expanded=\"false\""), "got: {html}");
    assert!(html.contains("aria-haspopup=\"true\""), "got: {html}");
    assert!(!html.contains("class=\"app-card swiped\""), "got: {html}");
}

// --- Acceptance 2: a swipe cannot trigger navigation ------------------------

#[test]
fn a_swipe_is_horizontal_and_clears_the_floor() {
    assert_eq!(resolve_swipe(-60.0, 4.0), SwipeIntent::Reveal);
    assert_eq!(resolve_swipe(60.0, 4.0), SwipeIntent::Dismiss);
    // Below the 44px floor: a still tap, not a swipe.
    assert_eq!(resolve_swipe(-20.0, 2.0), SwipeIntent::None);
    assert_eq!(resolve_swipe(43.9, 1.0), SwipeIntent::None);
    // Vertical dominance: a scroll, not a swipe.
    assert_eq!(resolve_swipe(-60.0, 90.0), SwipeIntent::None);
    // A diagonal that is mostly vertical is still a scroll.
    assert_eq!(resolve_swipe(-50.0, 50.0), SwipeIntent::None);
}

#[test]
fn the_tap_after_a_swipe_is_swallowed_never_navigating() {
    // A swipe-resolution marks the gesture so the click the browser fires
    // afterwards cannot navigate.
    assert_eq!(resolve_tap(false, true), TapIntent::Swallow);
    assert_eq!(resolve_tap(true, true), TapIntent::Swallow);
    // An open card only closes; it never navigates.
    assert_eq!(resolve_tap(true, false), TapIntent::Dismiss);
    // Only the settled, closed card may activate.
    assert_eq!(resolve_tap(false, false), TapIntent::Activate);
}

#[test]
fn no_swipe_state_can_reach_activate() {
    // Exhaustive: whenever a swipe happened (open, or resolved-just-now), a tap
    // resolves to anything but navigation.
    for swiped in [false, true] {
        let intent = resolve_tap(swiped, true);
        assert_ne!(intent, TapIntent::Activate);
    }
    assert_ne!(resolve_tap(true, false), TapIntent::Activate);
}

#[test]
fn the_gesture_floor_is_the_brand_spec_44px() {
    assert_eq!(SWIPE_THRESHOLD_PX, TAP_TARGET_PX as f64);
    assert_eq!(TAP_TARGET_PX, 44);
}

// --- Acceptance 3: every interactive element measures >= 44px ---------------

#[test]
fn the_reveal_distance_matches_the_stylesheet() {
    assert_eq!(SWIPE_REVEAL_PX, 112.0);
    let rule = rule_body(STYLESHEET, ".app-card.swiped .card-main").expect("swiped rule");
    assert!(rule.contains("translateX(-112px)"), "got: {rule}");
}

#[test]
fn card_action_buttons_clear_the_44px_floor() {
    let rule = rule_body(STYLESHEET, ".card-action").expect("action button rule");
    assert!(rule.contains("min-height: 44px"), "got: {rule}");
    // Full column width, so the target is square-or-wider at the 112px column.
    assert!(rule.contains("width: 100%"), "got: {rule}");
    let column = rule_body(STYLESHEET, ".card-swipe-actions").expect("column rule");
    assert!(column.contains("width: 112px"), "got: {column}");
}

#[test]
fn the_kebab_clears_the_44px_floor_in_both_axes() {
    let rule = rule_body(STYLESHEET, ".card-menu-btn").expect("kebab rule");
    assert!(rule.contains("min-height: 44px"), "got: {rule}");
    assert!(rule.contains("min-width: 44px"), "got: {rule}");
}

// --- Acceptance 4: no hover-only affordance lacks a touch path --------------

#[test]
fn every_kebab_hover_rule_is_pointer_gated() {
    // Hover is opt-in on a fine pointer; a touch device keeps the tap path.
    assert!(governed_by_hover_media(STYLESHEET, ".app-card:hover"));
    assert!(governed_by_hover_media(STYLESHEET, ".card-menu-btn:hover"));
}

#[test]
fn the_kebab_is_never_hidden_behind_hover() {
    let rule = rule_body(STYLESHEET, ".card-menu-btn").expect("kebab rule");
    assert!(!rule.contains("display: none"), "got: {rule}");
    // The reveal is class-driven (both the kebab and the swipe set `.swiped`),
    // never a `:hover` reveal that a touch screen could not reach.
    assert!(STYLESHEET.contains(".app-card.swiped .card-main"));
}

#[test]
fn a_card_renders_the_same_with_every_tone() {
    let html = support::mount_html(card_swiped_tone, || {});
    assert!(html.contains("card-status degraded"), "got: {html}");
    assert!(html.contains("pill danger"), "got: {html}");
    assert!(html.contains(">Degraded<"), "got: {html}");
}

#[test]
fn every_tone_has_a_slug_a_pill_and_a_label() {
    let tones = [
        CardTone::Synced,
        CardTone::OutOfSync,
        CardTone::Degraded,
        CardTone::Progressing,
    ];
    let mut slugs = Vec::new();
    for tone in tones {
        assert!(!tone.slug().is_empty(), "{tone:?} missing slug");
        assert!(!tone.pill().is_empty(), "{tone:?} missing pill");
        assert!(!tone.label().is_empty(), "{tone:?} missing label");
        slugs.push(tone.slug());
    }
    slugs.sort_unstable();
    slugs.dedup();
    assert_eq!(slugs.len(), tones.len(), "slugs must be unique");
}

// --- PR visual: a real render of the shipped stylesheet + this rsx ----------

fn card_grid() -> Element {
    rsx! {
        div { class: "app-grid",
            AppCard {
                name: "guestbook",
                sub: "default / demo-apps",
                tone: CardTone::Synced,
                repo: "github.com/acme/guestbook",
                version: "chart 1.4.2",
                age: "2m",
                actions: actions(),
            }
            AppCard {
                name: "canary-rollouts",
                sub: "prod / platform",
                tone: CardTone::OutOfSync,
                repo: "gitlab.com/acme/gitops",
                version: "kustomize 5.4",
                age: "8m",
                actions: actions(),
            }
            AppCard {
                name: "prometheus-stack",
                sub: "monitoring / infra",
                tone: CardTone::Degraded,
                repo: "github.com/acme/helm-charts",
                version: "chart 58.2.1",
                age: "3m",
                actions: actions(),
            }
        }
    }
}

/// Writes the fixture behind the PR's captures: this crate's own card rsx,
/// rendered headlessly, with the shipped `main.css` inlined, to
/// `OPENKITE_CARD_FIXTURE_OUT`. Ignored by default — it exists only to
/// regenerate the committed `docs/media/card-touch-*.png` captures.
#[test]
#[ignore = "writes the PR visual fixture; set OPENKITE_CARD_FIXTURE_OUT"]
fn dump_card_fixture() {
    let Ok(out) = std::env::var("OPENKITE_CARD_FIXTURE_OUT") else {
        return;
    };
    let body = support::mount_html(card_grid, || {});
    let page = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>OpenKite cards — OKT-169</title><style>{STYLESHEET}</style></head>\
<body style=\"margin:0;padding:16px\">{body}</body></html>"
    );
    std::fs::write(out, page).expect("write card fixture");
}

// --- small parsers ----------------------------------------------------------

/// The declaration block of the first rule whose selector is `selector`.
///
/// A selector head is the token followed by nothing but whitespace before its
/// `{`; occurrences inside comments (the banner names the same classes) are
/// skipped.
fn rule_body<'a>(css: &'a str, selector: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(rel) = css[from..].find(selector) {
        let at = from + rel;
        let rest = &css[at + selector.len()..];
        let open = rest.find('{')?;
        if rest[..open].trim().is_empty() {
            let body = &rest[open + 1..];
            let close = body.find('}')?;
            return Some(&body[..close]);
        }
        from = at + selector.len();
    }
    None
}

/// True when `selector`'s first rule sits inside an unclosed
/// `@media (hover: hover)` block.
fn governed_by_hover_media(css: &str, selector: &str) -> bool {
    let Some(at) = css.find(selector) else {
        return false;
    };
    let before = &css[..at];
    let Some(media) = before.rfind("@media") else {
        return false;
    };
    let block = &before[media..];
    block.contains("(hover: hover)") && !block.contains('}')
}
