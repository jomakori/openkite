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
    resize_by, DockState, DockTabKind, DEFAULT_HEIGHT, MAX_HEIGHT, MIN_HEIGHT,
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
