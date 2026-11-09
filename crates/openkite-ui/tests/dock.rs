//! The bottom dock's tab rules, pinned without a cluster or a browser.
//!
//! The tab arithmetic is exercised on [`DockState`] directly; the rendered
//! strip and dropdown are exercised through the headless SSR mount.

mod support;

use std::sync::Mutex;

use dioxus::prelude::*;
use openkite_api::pod::{ContainerInfo, PodObject, PodSummary};
use openkite_ui::components::dock::DockView;
use openkite_ui::dock::{
    activate, clamp_height, close, close_all, close_others, open_pod, open_terminal, promote,
    resize_by, split, swap, unsplit, DockState, DockTabKind, PaneSlot, DEFAULT_HEIGHT, MAX_HEIGHT,
    MIN_HEIGHT, SPLIT_MIN_WIDTH,
};
use openkite_ui::runtime::{self, DOCK};

// DOCK and SELECTED_POD are process globals; hold this guard in every test that
// mounts the dock so parallel threads cannot observe each other's tabs.
static RUNTIME_SLOTS: Mutex<()> = Mutex::new(());

fn gate_lock() -> std::sync::MutexGuard<'static, ()> {
    RUNTIME_SLOTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn pod(name: &str) -> PodObject {
    PodObject {
        name: name.into(),
        namespace: Some("default".into()),
        summary: PodSummary::default(),
        containers: vec![ContainerInfo {
            name: format!("{name}-c"),
            image: "nginx".into(),
            ready: true,
            restarts: 0,
            state: "Running".into(),
        }],
        labels: Default::default(),
        annotations: Default::default(),
        yaml: String::new(),
    }
}

fn dock() -> Element {
    rsx! { DockView {} }
}

fn seed(open: impl FnOnce(&mut DockState)) {
    reset();
    DOCK.with_mut(open);
}

fn reset() {
    *DOCK.write() = DockState::default();
    runtime::clear_selected_pod();
    runtime::set_dock_height(DEFAULT_HEIGHT);
}

fn mount(open: impl FnOnce(&mut DockState)) -> String {
    let _gate = gate_lock();
    support::mount_html(dock, move || {
        runtime::set_published_capabilities(None);
        seed(open);
    })
}

// --- acceptance: selecting five pods leaves one temp tab, not five ---------

#[test]
fn five_pod_selections_leave_one_temporary_tab() {
    let mut state = DockState::default();
    for name in ["web-1", "api-2", "worker-3", "db-4", "cache-5"] {
        open_pod(&mut state, pod(name));
    }
    assert_eq!(state.tabs.len(), 1, "the temporary tab is reused");
    assert!(state.tabs[0].temporary);
    assert_eq!(
        state.tabs[0].label(),
        "cache-5",
        "the tab follows the last pick"
    );
    assert_eq!(state.tabs[0].kind, DockTabKind::Logs);
    assert_eq!(state.active.as_deref(), Some(state.tabs[0].id.as_str()));
}

#[test]
fn the_reused_tab_keeps_its_identity() {
    let mut state = DockState::default();
    open_pod(&mut state, pod("web-1"));
    let id = state.tabs[0].id.clone();
    open_pod(&mut state, pod("api-2"));
    assert_eq!(state.tabs.len(), 1);
    assert_eq!(
        state.tabs[0].id, id,
        "the same tab is retargeted, not replaced"
    );
}

// --- acceptance: promote survives further selections -----------------------

#[test]
fn a_promoted_tab_survives_further_selections() {
    let mut state = DockState::default();
    open_pod(&mut state, pod("web-1"));
    let promoted = state.tabs[0].id.clone();

    promote(&mut state, &promoted);
    assert!(
        !state.tabs[0].temporary,
        "double-click makes the tab permanent"
    );

    open_pod(&mut state, pod("api-2"));
    open_pod(&mut state, pod("worker-3"));

    assert_eq!(
        state.tabs.len(),
        2,
        "the promoted tab plus one reused temp tab"
    );
    let kept = state
        .tab(&promoted)
        .expect("the promoted tab is still open");
    assert_eq!(kept.label(), "web-1");
    assert!(!kept.temporary);

    let temp = state
        .tabs
        .iter()
        .find(|tab| tab.temporary)
        .expect("a fresh temporary tab follows the new selection");
    assert_eq!(temp.label(), "worker-3");
}

#[test]
fn reselecting_an_open_pod_focuses_its_tab_instead_of_duplicating() {
    let mut state = DockState::default();
    open_pod(&mut state, pod("web-1"));
    let promoted = state.tabs[0].id.clone();
    promote(&mut state, &promoted);
    open_pod(&mut state, pod("api-2"));

    open_pod(&mut state, pod("web-1"));

    assert_eq!(state.tabs.len(), 2, "no duplicate for an already-open pod");
    assert_eq!(state.active.as_deref(), Some(promoted.as_str()));
}

#[test]
fn a_terminal_tab_is_never_the_temporary_one() {
    let mut state = DockState::default();
    open_terminal(&mut state, pod("web-1"));
    assert_eq!(state.tabs[0].kind, DockTabKind::Terminal);
    assert!(!state.tabs[0].temporary, "terminal tabs are permanent");

    open_pod(&mut state, pod("api-2"));
    assert_eq!(state.tabs.len(), 2, "the selection opens its own temp tab");
    let terminal = state
        .tabs
        .iter()
        .find(|tab| tab.kind == DockTabKind::Terminal)
        .expect("the terminal tab stands");
    assert!(!terminal.temporary);
    assert!(state.tabs.iter().any(|tab| tab.temporary));
}

// --- acceptance: the dropdown reflects live tabs ---------------------------

#[test]
fn an_empty_dock_renders_nothing() {
    let html = mount(|_| {});
    assert!(
        !html.contains("data-dock"),
        "collapsed dock must not paint: {html}"
    );
}

#[test]
fn the_dropdown_lists_every_live_tab() {
    let html = mount(|state| {
        open_terminal(state, pod("web-1"));
        open_pod(state, pod("api-2"));
    });
    assert!(html.contains("data-dock=\"open\""), "got: {html}");
    assert!(html.contains("data-dock-tab-count=\"2\""), "got: {html}");
    assert!(
        html.contains("dock-menu-switch"),
        "the dropdown lists rows: {html}"
    );
    assert!(html.contains(">web-1<"), "got: {html}");
    assert!(html.contains(">api-2<"), "got: {html}");
    assert!(html.contains(">Close others<"), "got: {html}");
    assert!(html.contains(">Close all<"), "got: {html}");
}

#[test]
fn the_dropdown_drops_a_tab_once_it_closes() {
    let html = mount(|state| {
        open_terminal(state, pod("web-1"));
        open_pod(state, pod("api-2"));
        let id = state.tabs[1].id.clone();
        close(state, &id);
    });
    assert!(html.contains("data-dock-tab-count=\"1\""), "got: {html}");
    assert!(html.contains(">web-1<"), "got: {html}");
    assert!(
        !html.contains(">api-2<"),
        "a closed tab leaves the dropdown: {html}"
    );
}

#[test]
fn the_temporary_tab_is_muted_and_dot_marked() {
    let html = mount(|state| {
        open_terminal(state, pod("web-1"));
        open_pod(state, pod("api-2"));
    });
    assert!(html.contains("data-temporary=\"1\""), "got: {html}");
    assert!(
        html.contains("class=\"dot\""),
        "the temporary tab carries the dot: {html}"
    );
    assert_eq!(
        html.matches("data-temporary=\"1\"").count(),
        1,
        "exactly one string of tabs is temporary: {html}"
    );
}

// --- acceptance: closing the last tab collapses the dock cleanly -----------

#[test]
fn closing_the_last_tab_collapses_the_dock() {
    let mut state = DockState::default();
    open_pod(&mut state, pod("web-1"));
    let id = state.tabs[0].id.clone();
    close(&mut state, &id);
    assert!(state.tabs.is_empty());
    assert!(state.active.is_none());
    assert!(!state.is_open());
}

#[test]
fn closing_the_last_tab_renders_no_dock() {
    let html = mount(|state| {
        open_pod(state, pod("web-1"));
        close_all(state);
    });
    assert!(
        !html.contains("data-dock"),
        "the dock collapses cleanly: {html}"
    );
}

#[test]
fn close_others_keeps_only_the_named_tab() {
    let mut state = DockState::default();
    open_pod(&mut state, pod("web-1"));
    open_terminal(&mut state, pod("api-2"));
    open_terminal(&mut state, pod("worker-3"));
    let keep = state.tabs[1].id.clone();

    close_others(&mut state, &keep);

    assert_eq!(state.tabs.len(), 1);
    assert_eq!(state.tabs[0].label(), "api-2");
    assert_eq!(state.active.as_deref(), Some(keep.as_str()));
}

#[test]
fn close_all_empties_the_dock() {
    let mut state = DockState::default();
    open_pod(&mut state, pod("web-1"));
    open_terminal(&mut state, pod("api-2"));
    close_all(&mut state);
    assert!(state.tabs.is_empty());
    assert!(state.active.is_none());
}

#[test]
fn closing_the_active_tab_focuses_a_neighbour() {
    let mut state = DockState::default();
    open_terminal(&mut state, pod("web-1"));
    open_terminal(&mut state, pod("api-2"));
    let first = state.tabs[0].id.clone();
    activate(&mut state, &first);
    close(&mut state, &first);
    assert_eq!(state.tabs.len(), 1);
    assert_eq!(state.active.as_deref(), Some(state.tabs[0].id.as_str()));
}

// --- acceptance: the height drag-resizes and persists ----------------------

#[test]
fn resize_clamps_and_follows_the_pointer() {
    assert_eq!(clamp_height(50.0), MIN_HEIGHT);
    assert_eq!(clamp_height(2000.0), MAX_HEIGHT);
    assert_eq!(clamp_height(f64::NAN), DEFAULT_HEIGHT);
    assert_eq!(resize_by(DEFAULT_HEIGHT, 60.0), DEFAULT_HEIGHT + 60);
    assert_eq!(resize_by(DEFAULT_HEIGHT, -1000.0), MIN_HEIGHT);
    assert_eq!(resize_by(DEFAULT_HEIGHT, 1000.0), MAX_HEIGHT);
}

#[test]
fn the_height_is_persisted_outside_the_runtime() {
    let _gate = gate_lock();
    let vdom = VirtualDom::new(|| rsx! {});
    vdom.in_runtime(|| {
        runtime::set_dock_height(412);
        assert_eq!(*openkite_ui::runtime::DOCK_HEIGHT.read(), 412);
        runtime::set_dock_height(99_999);
        assert_eq!(*openkite_ui::runtime::DOCK_HEIGHT.read(), MAX_HEIGHT);
    });
    assert_eq!(
        runtime::dock_height(),
        MAX_HEIGHT,
        "the runtime-free mirror holds it"
    );
}

#[test]
fn the_height_survives_tab_churn() {
    let _gate = gate_lock();
    let vdom = VirtualDom::new(|| rsx! {});
    vdom.in_runtime(|| {
        runtime::set_dock_height(388);
        runtime::open_pod_tab(pod("web-1"));
        runtime::open_terminal_tab(pod("api-2"));
        runtime::close_all_dock_tabs();
        assert_eq!(*openkite_ui::runtime::DOCK_HEIGHT.read(), 388);
    });
    assert_eq!(runtime::dock_height(), 388);
}

#[test]
fn the_dock_renders_the_persisted_height() {
    let _html_gate = gate_lock();
    let html = support::mount_html(dock, || {
        runtime::set_published_capabilities(None);
        runtime::set_dock_height(424);
        reset_tabs();
    });
    assert!(html.contains("--dock-height: 424px"), "got: {html}");
    assert!(html.contains("data-dock-height=\"424\""), "got: {html}");
}

fn reset_tabs() {
    runtime::open_pod_tab(pod("web-1"));
}

// --- OKT-207: split, swap, unsplit ----------------------------------------
//
// Swap is the interesting one: it exchanges the panes' positions and moves no
// tab, so a terminal tab stays attached to its own pane (and its own session)
// and a log tab keeps the follow position that belongs to it.

/// Two panes with different content: `web-1` terminal in A, `api-2` terminal
/// in B, and `B` focused — the state every swap test starts from.
fn split_with_two_terminals() -> DockState {
    let mut state = DockState::default();
    open_terminal(&mut state, pod("web-1"));
    split(&mut state);
    state.focus_pane(PaneSlot::B);
    open_terminal(&mut state, pod("api-2"));
    state
}

#[test]
fn a_default_dock_is_one_pane() {
    let state = DockState::default();
    assert!(!state.is_split());
    assert!(!state.is_open());
    assert_eq!(state.panes().len(), 1);
    assert_eq!(state.panes()[0].slot, PaneSlot::A);
    assert_eq!(state.visible_panes(375.0), vec![PaneSlot::A]);
}

#[test]
fn split_gives_two_panes_each_with_its_own_tabs_and_focus() {
    let state = split_with_two_terminals();
    assert!(state.is_split());
    assert_eq!(state.panes().len(), 2, "split paints two lanes");
    assert_eq!(state.pane_tabs(PaneSlot::A).len(), 1);
    assert_eq!(state.pane_tabs(PaneSlot::B).len(), 1);
    assert_eq!(state.pane_tabs(PaneSlot::A)[0].label(), "web-1");
    assert_eq!(state.pane_tabs(PaneSlot::B)[0].label(), "api-2");
    assert_ne!(
        state.pane_active(PaneSlot::A),
        state.pane_active(PaneSlot::B),
        "each pane keeps its own active tab"
    );
    assert_eq!(state.focused(), PaneSlot::B, "the last selection won focus");
    assert_eq!(
        state.active.as_deref(),
        state.pane_active(PaneSlot::A),
        "pane A's focus is the dock's historical `active` field"
    );
}

#[test]
fn splitting_twice_is_idempotent_and_keeps_pane_a() {
    let mut state = split_with_two_terminals();
    let before = state.clone();
    split(&mut state);
    assert_eq!(state, before, "a second split changes nothing");
}

#[test]
fn swap_moves_the_panes_without_moving_their_state() {
    let mut state = split_with_two_terminals();
    let a_tab = state.pane_tabs(PaneSlot::A)[0].id.clone();
    let b_tab = state.pane_tabs(PaneSlot::B)[0].id.clone();
    let a_active = state.pane_active(PaneSlot::A).map(str::to_string);
    let b_active = state.pane_active(PaneSlot::B).map(str::to_string);

    swap(&mut state);

    let order: Vec<PaneSlot> = state.panes().iter().map(|pane| pane.slot).collect();
    assert_eq!(order, vec![PaneSlot::B, PaneSlot::A], "positions exchange");
    assert_eq!(
        state.pane_tabs(PaneSlot::A)[0].id,
        a_tab,
        "pane A keeps its own tab"
    );
    assert_eq!(state.pane_tabs(PaneSlot::B)[0].id, b_tab);
    assert_eq!(state.pane_active(PaneSlot::A).map(str::to_string), a_active);
    assert_eq!(state.pane_active(PaneSlot::B).map(str::to_string), b_active);
    assert_eq!(
        state.focused(),
        PaneSlot::B,
        "focus follows the pane, not the position"
    );
}

#[test]
fn a_swapped_terminal_stays_attached_to_its_own_tab() {
    let mut state = split_with_two_terminals();
    let a_id = state
        .pane_active(PaneSlot::A)
        .expect("A has a tab")
        .to_string();
    let b_id = state
        .pane_active(PaneSlot::B)
        .expect("B has a tab")
        .to_string();

    swap(&mut state);

    let (slot, _) = state.tab_slot(&a_id).expect("A's tab is still open");
    assert_eq!(slot, PaneSlot::A, "the tab did not change pane");
    assert_eq!(state.pane_active(PaneSlot::A), Some(a_id.as_str()));
    assert_eq!(
        state.tab(&a_id).map(|tab| tab.label()),
        Some("web-1".into())
    );
    assert_eq!(
        state
            .tab(&b_id)
            .and_then(|tab| tab.pod.as_ref())
            .map(|pod| pod.name.as_str()),
        Some("api-2"),
        "and B's terminal still points at its own pod"
    );
}

#[test]
fn swap_without_a_split_does_nothing() {
    let mut state = DockState::default();
    open_terminal(&mut state, pod("web-1"));
    let before = state.clone();
    swap(&mut state);
    assert_eq!(state, before);
    assert_eq!(state.panes().len(), 1);
}

#[test]
fn unsplit_returns_to_one_pane_keeping_the_focused_panes_tabs() {
    let mut state = split_with_two_terminals();
    let b_tab = state.pane_tabs(PaneSlot::B)[0].id.clone();
    assert_eq!(state.focused(), PaneSlot::B);

    unsplit(&mut state);

    assert!(!state.is_split());
    assert_eq!(state.panes().len(), 1);
    assert_eq!(
        state.active.as_deref(),
        Some(b_tab.as_str()),
        "the focused pane's active tab is the survivor"
    );
    assert_eq!(state.tabs[0].id, b_tab, "and it comes first");
    assert!(
        state.tabs.iter().any(|tab| tab.label() == "web-1"),
        "the other pane's tab is adopted, not dropped: {:?}",
        state.tabs.iter().map(|t| t.label()).collect::<Vec<_>>()
    );
}

#[test]
fn unsplit_from_pane_a_keeps_a_focused_and_adopts_b() {
    let mut state = split_with_two_terminals();
    state.focus_pane(PaneSlot::A);
    let a_tab = state
        .pane_active(PaneSlot::A)
        .expect("A is focused")
        .to_string();

    unsplit(&mut state);

    assert_eq!(state.active.as_deref(), Some(a_tab.as_str()));
    assert_eq!(state.tabs.len(), 2);
    assert_eq!(state.tabs[0].id, a_tab);
}

#[test]
fn unsplit_keeps_at_most_one_temporary_tab() {
    let mut state = DockState::default();
    open_pod(&mut state, pod("api-2")); // temporary, pane A
    split(&mut state);
    state.focus_pane(PaneSlot::B);
    open_pod(&mut state, pod("db-4")); // temporary, pane B

    unsplit(&mut state);

    assert!(!state.is_split());
    assert_eq!(
        state.tabs.iter().filter(|tab| tab.temporary).count(),
        1,
        "two temporary tabs would both fight the next selection"
    );
    assert_eq!(
        state.tabs[0].label(),
        "db-4",
        "the focused pane's temporary tab wins"
    );
}

#[test]
fn a_selection_targets_the_focused_pane() {
    let mut state = split_with_two_terminals();
    let a_before = state.pane_tabs(PaneSlot::A).len();

    open_pod(&mut state, pod("cache-5"));

    assert_eq!(
        state.pane_tabs(PaneSlot::A).len(),
        a_before,
        "pane A is untouched"
    );
    assert_eq!(
        state.pane_tabs(PaneSlot::B).last().expect("B grew").label(),
        "cache-5"
    );
    assert_eq!(state.focused(), PaneSlot::B);
}

#[test]
fn each_pane_reuses_its_own_temporary_tab() {
    let mut state = split_with_two_terminals();
    open_pod(&mut state, pod("cache-5"));
    open_pod(&mut state, pod("db-4"));
    assert_eq!(
        state
            .pane_tabs(PaneSlot::B)
            .iter()
            .filter(|tab| tab.temporary)
            .count(),
        1,
        "pane B has one temporary tab, retargeted"
    );
    state.focus_pane(PaneSlot::A);
    open_pod(&mut state, pod("worker-3"));
    assert_eq!(
        state
            .pane_tabs(PaneSlot::A)
            .iter()
            .filter(|tab| tab.temporary)
            .count(),
        1,
        "pane A has its own temporary tab, it never had one before"
    );
    assert_eq!(state.pane_tabs(PaneSlot::A).len(), 2);
}

#[test]
fn a_selection_focuses_a_tab_that_is_already_open_in_the_other_pane() {
    let mut state = split_with_two_terminals();
    state.focus_pane(PaneSlot::B);
    open_pod(&mut state, pod("worker-3"));
    state.focus_pane(PaneSlot::A);

    open_pod(&mut state, pod("worker-3"));

    assert_eq!(
        state.focused(),
        PaneSlot::B,
        "the existing tab's pane takes focus instead of a duplicate"
    );
    assert_eq!(state.tabs.len(), 1, "pane A never opened a second tab");
}

#[test]
fn closing_the_last_tab_of_a_pane_keeps_the_split() {
    let mut state = split_with_two_terminals();
    let b_tab = state.pane_tabs(PaneSlot::B)[0].id.clone();
    close(&mut state, &b_tab);
    assert!(state.pane_tabs(PaneSlot::B).is_empty());
    assert_eq!(state.pane_active(PaneSlot::B), None);
    assert!(state.is_split(), "an empty pane is still a pane");
    assert_eq!(state.panes().len(), 2);
}

#[test]
fn narrow_viewports_show_the_focused_pane_and_restore_the_split() {
    let mut state = split_with_two_terminals();
    assert_eq!(
        state.visible_panes(SPLIT_MIN_WIDTH - 1.0),
        vec![PaneSlot::B],
        "<=767px shows the focused pane alone"
    );
    assert_eq!(
        state.visible_panes(SPLIT_MIN_WIDTH),
        vec![PaneSlot::A, PaneSlot::B],
        ">=768px restores both"
    );

    swap(&mut state);
    assert_eq!(
        state.visible_panes(SPLIT_MIN_WIDTH - 1.0),
        vec![PaneSlot::B],
        "collapse keeps the focused pane, wherever it is painted"
    );
    assert_eq!(
        state.visible_panes(SPLIT_MIN_WIDTH),
        vec![PaneSlot::B, PaneSlot::A],
        "widening restores the swapped order"
    );

    state.focus_pane(PaneSlot::A);
    assert_eq!(state.visible_panes(375.0), vec![PaneSlot::A]);
}

#[test]
fn the_stylesheet_collapses_at_the_boundary_the_model_pins() {
    let css = include_str!("../assets/main.css");
    assert!(
        css.contains("@media (max-width: 767px)"),
        "the stylesheet carries the narrow rule"
    );
    assert!(
        css.contains(".dock[data-dock-layout=\"split\"] .dock-pane:not([data-focused=\"true\"])"),
        "the rule hides the pane the toolbar is not on"
    );
    assert_eq!(SPLIT_MIN_WIDTH, 768.0, "767 is the last collapsed width");
}

#[test]
fn close_others_keeps_the_focused_panes_tab() {
    // Pane A is empty, so its `active` is `None`; the manager must still offer
    // "Close others", because the focused pane has a tab to keep.
    let html = mount(|state| {
        split(state);
        state.focus_pane(PaneSlot::B);
        open_terminal(state, pod("web-1"));
    });
    assert!(
        !html.contains(" disabled"),
        "close others is live when the focused pane has a tab: {html}"
    );
}

// --- OKT-207: rendered panes ----------------------------------------------

/// The markup of one pane: from its `data-pane` marker to the next one.
fn pane_slice<'a>(html: &'a str, key: &str) -> &'a str {
    let marker = format!("data-pane=\"{key}\"");
    let start = html
        .find(&marker)
        .unwrap_or_else(|| panic!("no pane {key}: {html}"));
    let rest = &html[start..];
    match rest[marker.len()..].find("data-pane=\"") {
        Some(next) => &rest[..marker.len() + next],
        None => rest,
    }
}

#[test]
fn split_renders_two_panes_with_their_own_strips() {
    let html = mount(|state| {
        open_terminal(state, pod("web-1"));
        split(state);
        state.focus_pane(PaneSlot::B);
        open_terminal(state, pod("api-2"));
    });
    assert!(html.contains("data-dock-layout=\"split\""), "got: {html}");
    assert_eq!(html.matches("class=\"dock-pane\"").count(), 2, "two lanes");
    assert!(
        html.contains("class=\"dock-pane-bar\""),
        "each pane has a strip"
    );
    let a = pane_slice(&html, "a");
    let b = pane_slice(&html, "b");
    assert!(a.contains(">web-1<"), "pane A paints its own tab: {a}");
    assert!(!a.contains(">api-2<"), "and not pane B's: {a}");
    assert!(b.contains(">api-2<"), "pane B paints its own tab: {b}");
    assert!(
        b.contains("aria-selected=\"true\""),
        "pane B has a focus: {b}"
    );
    assert!(html.contains("data-dock-focus=\"b\""), "got: {html}");
    assert!(html.contains("data-dock-tab-count=\"2\""), "got: {html}");
}

#[test]
fn the_toolbar_offers_split_swap_and_then_unsplit() {
    let single = mount(|state| {
        open_terminal(state, pod("web-1"));
    });
    assert!(
        single.contains("data-dock-action=\"split\""),
        "got: {single}"
    );
    assert!(
        single.contains("aria-label=\"Split dock\""),
        "got: {single}"
    );
    assert!(
        single.contains("data-dock-action=\"swap\""),
        "got: {single}"
    );
    assert!(
        single.contains("aria-label=\"Swap panes\""),
        "got: {single}"
    );
    assert!(
        !single.contains("data-dock-action=\"unsplit\""),
        "nothing to unsplit yet: {single}"
    );
    assert!(
        single.contains("aria-disabled=\"true\"") && single.contains("Split the dock first"),
        "swap stays focusable and explained before a split: {single}"
    );

    let split_html = mount(|state| {
        open_terminal(state, pod("web-1"));
        split(state);
    });
    assert!(
        split_html.contains("data-dock-action=\"unsplit\""),
        "got: {split_html}"
    );
    assert!(
        split_html.contains("aria-label=\"Unsplit dock\""),
        "got: {split_html}"
    );
    assert!(
        !split_html.contains("data-dock-action=\"split\""),
        "one toggle, not two buttons: {split_html}"
    );
    assert!(
        split_html.contains(
            "data-dock-action=\"swap\" aria-label=\"Swap panes\" aria-disabled=\"false\""
        ),
        "swap is live once split: {split_html}"
    );
}

#[test]
fn swap_repaints_the_panes_in_the_opposite_order() {
    let html = mount(|state| {
        open_terminal(state, pod("web-1"));
        split(state);
        state.focus_pane(PaneSlot::B);
        open_terminal(state, pod("api-2"));
        swap(state);
    });
    let a_at = html.find("data-pane=\"a\"").expect("pane A is painted");
    let b_at = html.find("data-pane=\"b\"").expect("pane B is painted");
    assert!(b_at < a_at, "pane B paints first after the swap: {html}");
    let b = pane_slice(&html, "b");
    assert!(b.contains(">api-2<"), "and still carries its own tab: {b}");
    assert!(!b.contains(">web-1<"), "not A's: {b}");
    let a = pane_slice(&html, "a");
    assert!(a.contains(">web-1<"), "pane A keeps its tab: {a}");
}

#[test]
fn unsplit_repaints_the_single_pane_bar() {
    let html = mount(|state| {
        open_terminal(state, pod("web-1"));
        split(state);
        state.focus_pane(PaneSlot::B);
        open_terminal(state, pod("api-2"));
        unsplit(state);
    });
    assert!(html.contains("data-dock-layout=\"single\""), "got: {html}");
    assert_eq!(html.matches("class=\"dock-pane\"").count(), 1);
    assert!(
        !html.contains("class=\"dock-pane-bar\""),
        "the single pane's strip is the dock bar again: {html}"
    );
    assert!(html.contains(">api-2<"), "the focused pane's tab survives");
    assert!(html.contains(">web-1<"), "and the other was adopted");
}

#[test]
fn the_single_pane_dock_keeps_its_original_bar() {
    let html = mount(|state| {
        open_terminal(state, pod("web-1"));
        open_pod(state, pod("api-2"));
    });
    assert!(html.contains("data-dock-layout=\"single\""), "got: {html}");
    assert_eq!(html.matches("class=\"dock-pane\"").count(), 1);
    assert!(
        !html.contains("class=\"dock-pane-bar\""),
        "no pane strip in single mode — the bar holds the tabs: {html}"
    );
    let bar = html.find("class=\"dock-bar\"").expect("the bar is painted");
    let panes = html.find("class=\"dock-panes\"").expect("the lane wrapper");
    assert!(bar < panes, "the strip stays above the body: {html}");
    assert!(
        html[bar..panes].contains("data-dock-tab"),
        "the single strip lives in the dock bar: {html}"
    );
}

#[test]
fn the_single_pane_is_not_an_extra_tab_stop() {
    let single = mount(|state| {
        open_terminal(state, pod("web-1"));
    });
    assert_eq!(
        single.matches("class=\"dock-pane\"").count(),
        1,
        "got: {single}"
    );
    // A pane you can focus is a split pane's affordance: the single pane is
    // already the only pane, so it adds no stop to the page's tab order.
    let at = single.find("class=\"dock-pane\"").expect("pane is painted");
    let tag = single[at..].split('>').next().unwrap_or_default();
    assert!(
        !tag.contains("tabindex"),
        "single pane is not focusable: {tag}"
    );
    assert!(
        !tag.contains("data-focused"),
        "and carries no focus ring: {tag}"
    );

    let split_html = mount(|state| {
        open_terminal(state, pod("web-1"));
        split(state);
    });
    assert!(
        split_html.contains("data-pane=\"a\" data-focused=\"true\""),
        "a split pane is focusable and stamped with its focus: {split_html}"
    );
    assert!(
        split_html.contains("role=\"group\" aria-label=\"Dock pane A\" tabindex=\"0\""),
        "and is a labelled, focusable group: {split_html}"
    );
}
