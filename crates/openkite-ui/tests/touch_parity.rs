//! Touch parity (OKT-173): the namespace bar and the dock.
//!
//! Pins the ticket's three acceptance lines. Every desktop affordance names a
//! touch path — the crate carries the dock's (the long-press that replaces its
//! double-click, the sheet its dropdown opens as), the design system documents
//! the rest, including the side pane. No affordance is hover-only. And a
//! long-press can never be confused with a scroll.

mod support;

use dioxus::prelude::*;
use openkite_ui::components::namespace_bar::NamespaceBar;
use openkite_ui::runtime::toggle_namespace;
use openkite_ui::touch::{
    keep_tab_toast, resolve_press, PressGesture, PressIntent, KEEP_TAB_LABEL, LONG_PRESS_MS,
    PRESS_MOVE_TOLERANCE_PX,
};

const STYLESHEET: &str = openkite_ui::MAIN_CSS;
const DESIGN_SYSTEM: &str = include_str!("../../../docs/design-system.md");

// --- Acceptance 3: a long-press can never be confused with a scroll ---------

#[test]
fn a_still_press_held_past_the_threshold_is_the_long_press() {
    assert_eq!(
        resolve_press(0.0, 0.0, LONG_PRESS_MS),
        PressIntent::LongPress
    );
    assert_eq!(resolve_press(0.0, 0.0, 4_000), PressIntent::LongPress);
    // Exactly at the tolerance is still a press; the floor is inclusive.
    assert_eq!(
        resolve_press(PRESS_MOVE_TOLERANCE_PX, 0.0, LONG_PRESS_MS),
        PressIntent::LongPress
    );
}

#[test]
fn a_short_still_press_is_a_tap() {
    assert_eq!(resolve_press(0.0, 0.0, 0), PressIntent::Tap);
    assert_eq!(resolve_press(0.0, 0.0, LONG_PRESS_MS - 1), PressIntent::Tap);
    assert_eq!(resolve_press(3.0, 4.0, LONG_PRESS_MS - 1), PressIntent::Tap);
}

#[test]
fn movement_past_the_tolerance_is_a_scroll_however_long_it_is_held() {
    // Horizontal: the chip strip and the dock tab strip both scroll this way.
    assert_eq!(
        resolve_press(PRESS_MOVE_TOLERANCE_PX + 0.5, 0.0, LONG_PRESS_MS),
        PressIntent::Scroll
    );
    // Vertical: the page scroll.
    assert_eq!(
        resolve_press(0.0, 120.0, LONG_PRESS_MS),
        PressIntent::Scroll
    );
    // Diagonal drift counts by its length, not by one axis alone.
    assert_eq!(resolve_press(8.0, 8.0, LONG_PRESS_MS), PressIntent::Scroll);
    // Exhaustive: no travel past the tolerance ever resolves to a long-press,
    // at any hold duration.
    for ms in [0, LONG_PRESS_MS - 1, LONG_PRESS_MS, LONG_PRESS_MS * 4] {
        for (dx, dy) in [
            (60.0, 0.0),
            (-60.0, 2.0),
            (0.0, 60.0),
            (40.0, -40.0),
            (11.0, 0.0),
        ] {
            assert_ne!(
                resolve_press(dx, dy, ms),
                PressIntent::LongPress,
                "dx={dx} dy={dy} ms={ms}"
            );
        }
    }
}

#[test]
fn a_finger_that_drifts_and_settles_back_can_never_fire_the_long_press() {
    let mut gesture = PressGesture::default();
    gesture.down(0.0, 0.0);
    assert!(gesture.armed(), "a fresh press is a long-press candidate");
    // Past the tolerance: latched as a scroll.
    assert!(!gesture.moved(30.0, 0.0), "the gesture latched a scroll");
    // Back at the origin and held far past the threshold: still a scroll.
    assert!(!gesture.moved(0.0, 0.0));
    assert_eq!(gesture.resolve(LONG_PRESS_MS * 3), PressIntent::Scroll);
    assert_eq!(gesture.resolve(0), PressIntent::Scroll);
}

#[test]
fn a_cancelled_or_absent_press_never_fires() {
    let mut gesture = PressGesture::default();
    gesture.down(4.0, 4.0);
    gesture.cancel();
    assert!(!gesture.armed());
    assert_eq!(gesture.resolve(LONG_PRESS_MS), PressIntent::Scroll);
    // A gesture that never went down has nothing to resolve either: a
    // `pointercancel` (the browser taking the pan for a scroll) is inert, never
    // a long-press.
    let idle = PressGesture::default();
    assert!(!idle.armed());
    assert_eq!(idle.resolve(LONG_PRESS_MS), PressIntent::Scroll);
}

#[test]
fn an_untouched_press_resolves_by_its_hold_alone() {
    let mut gesture = PressGesture::default();
    gesture.down(10.0, 10.0);
    assert!(gesture.moved(12.0, 10.0), "within tolerance stays armed");
    assert_eq!(gesture.resolve(LONG_PRESS_MS - 1), PressIntent::Tap);
    assert_eq!(gesture.resolve(LONG_PRESS_MS), PressIntent::LongPress);
}

// --- The dock's touch contract (acceptance 1, the dock half) ----------------

#[test]
fn the_keep_tab_action_and_its_confirming_toast_are_named() {
    assert_eq!(KEEP_TAB_LABEL, "Keep tab");
    let toast = keep_tab_toast("grafana-0");
    assert!(
        toast.contains("grafana-0"),
        "the toast names the tab: {toast}"
    );
    assert!(
        toast.to_lowercase().contains("keep"),
        "the toast confirms the keep: {toast}"
    );
}

#[test]
fn the_dock_manager_becomes_a_bottom_sheet_on_a_coarse_pointer() {
    let rule = rule_body_in_media(STYLESHEET, ".dock-menu.open", "(pointer: coarse)")
        .expect("the dock manager's coarse-pointer rule");
    for decl in [
        "position: fixed",
        "left: 0",
        "right: 0",
        "bottom: 0",
        "border-radius:",
        "max-height:",
    ] {
        assert!(
            rule.contains(decl),
            "the bottom sheet needs `{decl}`: {rule}"
        );
    }
    assert!(
        !rule.contains("bottom: calc(100%"),
        "the dropdown anchor is replaced, not kept: {rule}"
    );
}

#[test]
fn every_dock_sheet_control_meets_the_44px_floor() {
    for selector in [
        ".dock-tab",
        ".dock-menu-row",
        ".dock-menu-switch",
        ".dock-menu-close",
        ".dock-menu-action",
    ] {
        let rule = rule_body_in_media(STYLESHEET, selector, "(pointer: coarse)")
            .unwrap_or_else(|| panic!("`{selector}` has no coarse-pointer rule"));
        assert!(
            clears_the_44px_floor(rule),
            "`{selector}` is under the touch floor: {rule}"
        );
    }
}

#[test]
fn a_long_press_on_a_tab_never_starts_a_text_selection() {
    let rule = rule_body(STYLESHEET, ".dock-tab").expect("`.dock-tab` rule");
    assert!(
        rule.contains("user-select: none"),
        "a held tab must not select its label: {rule}"
    );
    assert!(
        !rule.contains("touch-action: pan-y"),
        "the tab strip must keep its own horizontal pan: {rule}"
    );
}

#[test]
fn the_dock_grab_strip_grows_for_a_finger() {
    let rule = rule_body_in_media(STYLESHEET, ".dock-resize", "(pointer: coarse)")
        .expect("the coarse-pointer grab strip");
    assert!(
        rule.contains("height: 22px"),
        "the 6px grip grows to a finger-sized strip: {rule}"
    );
}

// --- Acceptance 2: nothing is hover-only -----------------------------------

#[test]
fn the_stylesheet_has_hover_rules_for_this_test_to_be_meaningful() {
    let count = hover_declaration_bodies(STYLESHEET).len();
    assert!(
        count >= 8,
        "expected the console's hover rules to be present, found {count}"
    );
}

#[test]
fn no_hover_rule_escapes_the_precise_pointer_gate() {
    let ungated = ungated_hovers(STYLESHEET);
    assert!(
        ungated.is_empty(),
        "hover rules outside a `(hover: hover)` gate: {ungated:?}"
    );
}

#[test]
fn hover_only_ever_decorates_it_never_hides_an_affordance() {
    for body in hover_declaration_bodies(STYLESHEET) {
        assert!(
            !body.contains("display: none"),
            "hover hides content a touch screen cannot reveal: {body}"
        );
        assert!(
            !body.contains("visibility: hidden"),
            "hover hides content a touch screen cannot reveal: {body}"
        );
    }
}

// --- The namespace bar's touch path (the ticket's bullets) ------------------

fn ns_bar_options() -> Vec<String> {
    [
        "default",
        "kube-system",
        "argo",
        "monitoring",
        "cert-manager",
        "istio-system",
        "kyverno",
        "platform",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn ns_bar() -> Element {
    rsx! {
        NamespaceBar { options: ns_bar_options() }
    }
}

#[test]
fn the_closed_bar_keeps_the_field_in_the_row_for_the_circle_to_show() {
    let html = support::mount_html(ns_bar, || {});
    assert!(html.contains("data-filtering=\"false\""), "got: {html}");
    assert!(
        html.contains("class=\"search-field ns-filter\""),
        "the field stays in the row: {html}"
    );
    assert!(
        html.contains("aria-label=\"Filter namespaces\""),
        "got: {html}"
    );
}

#[test]
fn the_strip_is_pinned_to_its_row_so_it_never_pushes_the_page_wide() {
    let bar = rule_body(STYLESHEET, ".ns-bar").expect("`.ns-bar` rule");
    assert!(
        bar.contains("max-width: 100%"),
        "a wrapping toolbar will not stretch the bar, so the strip must be capped \
         to scroll instead of overflowing: {bar}"
    );
}

#[test]
fn the_closed_style_hides_the_field_behind_the_hook_the_circle_sets() {
    let rule = rule_body(STYLESHEET, ".ns-bar[data-filtering=\"false\"] .ns-filter")
        .expect("the closed-field rule");
    assert!(rule.contains("display: none"), "got: {rule}");
}

#[test]
fn the_namespace_search_spans_the_width_above_the_strip_on_a_coarse_pointer() {
    let field = rule_body_in_media(
        STYLESHEET,
        ".ns-bar[data-filtering=\"true\"] .ns-filter",
        "(pointer: coarse)",
    )
    .expect("the coarse-pointer field rule");
    assert!(
        field.contains("order: -1"),
        "the field moves above the strip: {field}"
    );
    assert!(
        field.contains("flex: 1 0 100%"),
        "and takes its own line: {field}"
    );
    assert!(field.contains("width: 100%"), "and the full width: {field}");

    let bar = rule_body_in_media(
        STYLESHEET,
        ".ns-bar[data-filtering=\"true\"]",
        "(pointer: coarse)",
    )
    .expect("the coarse-pointer bar rule");
    assert!(
        bar.contains("flex-wrap: wrap"),
        "the row wraps for it: {bar}"
    );
    assert!(
        bar.contains("width: 100%"),
        "the wrapped row is pinned to its container so it cannot outgrow it: {bar}"
    );
}

#[test]
fn the_chip_strip_snaps_and_stops_so_no_chip_rests_half_visible() {
    let row = rule_body(STYLESHEET, ".chip-row").expect("`.chip-row` rule");
    assert!(
        row.contains("scroll-snap-type: x mandatory"),
        "proximity lets a chip rest half-visible; mandatory does not: {row}"
    );
    let chip = rule_body(STYLESHEET, ".chip-row .chip").expect("`.chip-row .chip` rule");
    assert!(chip.contains("scroll-snap-align: start"), "got: {chip}");
    assert!(
        chip.contains("scroll-snap-stop: always"),
        "a flick must not carry past a chip: {chip}"
    );
}

// --- Acceptance 1: the touch paths are documented --------------------------

#[test]
fn the_design_system_documents_a_touch_path_for_the_dock_and_the_side_pane() {
    assert!(
        DESIGN_SYSTEM.contains("## Touch paths"),
        "no touch-path section"
    );
    for anchor in [
        "`.dock-tab",
        "`.dock-menu`",
        "`.sidebar",
        "`.inspector`",
        "`.chip-row`",
        "`.ns-search`",
    ] {
        assert!(
            DESIGN_SYSTEM.contains(anchor),
            "the touch paths must cover `{anchor}`"
        );
    }
    assert!(
        DESIGN_SYSTEM.contains("pointer: coarse"),
        "the section must name the gate it describes"
    );
}

// --- PR visual: a real render of the shipped stylesheet + this rsx ----------

/// Writes the fixture behind the PR's captures: this crate's own namespace bar,
/// rendered headlessly with the shipped `main.css` inlined, to
/// `OPENKITE_TOUCH_FIXTURE_OUT`. Ignored by default — it exists only to
/// regenerate the committed `docs/media/okt173-*.png` captures.
#[test]
#[ignore = "writes the PR visual fixture; set OPENKITE_TOUCH_FIXTURE_OUT"]
fn dump_touch_fixture() {
    let Ok(out) = std::env::var("OPENKITE_TOUCH_FIXTURE_OUT") else {
        return;
    };
    let body = support::mount_html(ns_bar, || {
        toggle_namespace("default".to_string());
        toggle_namespace("argo".to_string());
    });
    let page = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>OpenKite namespace bar — OKT-173</title><style>{STYLESHEET}</style></head>\
<body style=\"margin:0;padding:12px;background:#0b1020\">\
<div class=\"toolbar\">{body}</div></body></html>"
    );
    std::fs::write(out, page).expect("write touch fixture");
}

// --- small parsers ---------------------------------------------------------

/// The declaration block of the first rule whose selector head is exactly
/// `selector`, searching from `from`.
fn rule_from<'a>(css: &'a str, selector: &str, from: usize) -> Option<&'a str> {
    let mut search_from = from;
    while let Some(offset) = css[search_from..].find(selector) {
        let start = search_from + offset;
        let after = start + selector.len();
        let boundary = css[after..].chars().next();
        let is_head = match boundary {
            Some('{') | Some(',') | None => true,
            Some(c) => c.is_whitespace(),
        };
        if !is_head {
            search_from = after;
            continue;
        }
        let open = css[start..].find('{').map(|i| start + i)?;
        let close = css[open..].find('}').map(|i| open + i)?;
        return Some(&css[open + 1..close]);
    }
    None
}

/// The declaration block of the rule `selector`, anywhere in the stylesheet.
fn rule_body<'a>(css: &'a str, selector: &str) -> Option<&'a str> {
    rule_from(css, selector, 0)
}

/// The declaration block of the rule `selector` when it sits inside a
/// `@media …` block whose prelude contains `media`.
fn rule_body_in_media<'a>(css: &'a str, selector: &str, media: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(offset) = css[from..].find(selector) {
        let at = from + offset;
        if inside_media(css, at, media) {
            return rule_from(css, selector, offset + from);
        }
        from = at + selector.len();
    }
    None
}

/// Whether the byte at `at` sits inside a block whose prelude is an `@media`
/// naming `media`. Tracks brace depth, so the second and later rules of a
/// block are seen as well as the first.
fn inside_media(css: &str, at: usize, media: &str) -> bool {
    let mut stack: Vec<usize> = Vec::new();
    for (i, b) in css[..at].bytes().enumerate() {
        match b {
            b'{' => stack.push(i),
            b'}' => {
                stack.pop();
            }
            _ => {}
        }
    }
    let Some(&open) = stack.last() else {
        return false;
    };
    // A prelude runs from the previous brace to the block's own `{`; comments
    // between blocks are not part of it.
    let prelude_start = css[..open].rfind(['}', '{']).map_or(0, |i| i + 1);
    let prelude = strip_comments(&css[prelude_start..open]);
    prelude.contains("@media") && prelude.contains(media)
}

/// Drop `/* … */` spans from a selector prelude.
fn strip_comments(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start..].find("*/") {
            Some(end) => rest = &rest[start + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Every `:hover` in the stylesheet whose rule is not inside a
/// `(hover: hover)` gate, as `(selector-ish, offset)`.
fn ungated_hovers(css: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(offset) = css[from..].find(":hover") {
        let at = from + offset;
        if !inside_media(css, at, "(hover: hover)") {
            let line_start = css[..at].rfind('\n').map_or(0, |i| i + 1);
            out.push((css[line_start..at].trim().to_string(), at));
        }
        from = at + ":hover".len();
    }
    out
}

/// The declaration body of every rule that carries a `:hover` selector.
fn hover_declaration_bodies(css: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(offset) = css[from..].find(":hover") {
        let at = from + offset;
        let Some(open) = css[at..].find('{').map(|i| at + i) else {
            break;
        };
        let Some(close) = css[open..].find('}').map(|i| open + i) else {
            break;
        };
        out.push(&css[open + 1..close]);
        from = close;
    }
    out
}

/// Whether a rule's declarations place its control on the 44px touch floor.
fn clears_the_44px_floor(rule: &str) -> bool {
    rule.contains("min-height: 44px") || rule.contains("height: 44px")
}
