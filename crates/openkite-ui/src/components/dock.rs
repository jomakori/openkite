//! The bottom dock: one or two panes of tabs, a toolbar, a dropdown tab
//! manager and the active terminal or log surface.
//!
//! One tab per pane is the reusable temporary one a pod selection owns; the
//! rest are permanent. The rules live in [`crate::dock`]; this module renders
//! them and forwards every mutation to [`crate::runtime`] so the state
//! outlives the component's own re-renders.
//!
//! Single-pane keeps the dock's original shape: one bar holding the strip, the
//! layout controls and the tab manager, over one body. Splitting moves each
//! pane's strip into that pane, so both panes have their own tabs and their own
//! active tab. Swapping reverses which pane is painted first — the panes are
//! keyed by their slot, so the reorder moves the mounted pane instead of
//! remounting it and a terminal keeps its session.

use dioxus::prelude::*;

use crate::components::logs::LogsView;
use crate::components::terminal::TerminalView;
use crate::dock::{DockState, DockTabKind, PaneSlot};
use crate::runtime::{
    activate_dock_tab, close_all_dock_tabs, close_dock_tab, close_other_dock_tabs, focus_dock_pane,
    promote_dock_tab, set_dock_height, split_dock, swap_dock, unsplit_dock, DOCK, DOCK_HEIGHT,
    SELECTED_POD,
};

/// A tab as a pane's strip and the manager render it.
struct TabRow {
    id: String,
    label: String,
    active: bool,
    temporary: bool,
}

/// One pane as the view paints it.
struct PaneRow {
    slot: PaneSlot,
    tabs: Vec<TabRow>,
    focus: bool,
    kind: Option<DockTabKind>,
    empty: bool,
}

fn pane_rows(state: &DockState) -> Vec<PaneRow> {
    state
        .panes()
        .into_iter()
        .map(|pane| {
            let tabs: Vec<TabRow> = pane
                .tabs
                .iter()
                .map(|tab| TabRow {
                    id: tab.id.clone(),
                    label: tab.label(),
                    active: pane.active == Some(tab.id.as_str()),
                    temporary: tab.temporary,
                })
                .collect();
            let kind = pane
                .active
                .and_then(|id| pane.tabs.iter().find(|tab| tab.id == id))
                .map(|tab| tab.kind);
            PaneRow {
                slot: pane.slot,
                empty: tabs.is_empty(),
                tabs,
                focus: pane.focused,
                kind,
            }
        })
        .collect()
}

/// One pane's tab strip.
fn strip_markup(pane: &PaneRow) -> Element {
    let label = format!("Dock pane {} tabs", pane.slot.key().to_uppercase());
    let rows = &pane.tabs;
    rsx! {
        div { class: "dock-tabs", role: "tablist", aria_label: "{label}",
            for row in rows.iter() {
                {
                    let select = row.id.clone();
                    let promote = row.id.clone();
                    let close_id = row.id.clone();
                    rsx! {
                        div {
                            key: "{row.id}",
                            class: if row.active { "dock-tab active" } else { "dock-tab" },
                            role: "tab",
                            "data-dock-tab": "{row.id}",
                            "data-temporary": if row.temporary { Some("1") } else { None },
                            "aria-selected": if row.active { "true" } else { "false" },
                            tabindex: "0",
                            title: if row.temporary { "Temporary tab — double-click to keep it" } else { "Permanent tab" },
                            onclick: move |_| activate_dock_tab(&select),
                            ondoubleclick: move |_| promote_dock_tab(&promote),
                            if row.temporary {
                                span { class: "dot" }
                            }
                            span { class: "dock-tab-label", "{row.label}" }
                            button {
                                class: "dock-tab-close",
                                r#type: "button",
                                aria_label: "Close {row.label}",
                                onclick: move |event| {
                                    event.stop_propagation();
                                    close_dock_tab(&close_id);
                                },
                                "×"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One pane: its strip (while split) and the surface its active tab names.
///
/// Keyed by slot, not by position, so [`swap`](crate::dock::swap) reorders the
/// mounted pane instead of tearing it down.
fn pane_markup(pane: &PaneRow, split: bool) -> Element {
    let slot = pane.slot;
    let key = slot.key();
    let label = format!("Dock pane {}", slot.key().to_uppercase());
    let empty = pane.empty;
    let focused = if pane.focus { "true" } else { "false" };
    // The focus stamp is a split affordance: with one pane there is no target
    // to pick, so the single dock carries no focus ring and no extra stop.
    let focused_attr = split.then_some(focused);
    let kind = pane.kind;
    let strip = strip_markup(pane);

    rsx! {
        div {
            key: "{key}",
            class: "dock-pane",
            "data-pane": "{key}",
            "data-focused": focused_attr,
            role: "group",
            aria_label: "{label}",
            // Only a split has a target to pick: the single pane is the only
            // pane, so it is already focused and needs no tab stop of its own.
            tabindex: split.then_some("0"),
            onfocus: move |_| {
                if split {
                    focus_dock_pane(slot);
                }
            },
            onpointerdown: move |_| {
                if split {
                    focus_dock_pane(slot);
                }
            },

            if split {
                div { class: "dock-pane-bar", {strip} }
            }

            div { class: "dock-body",
                if empty && split {
                    p { class: "dock-pane-empty",
                        "Empty pane — pick a pod or open a terminal to fill it."
                    }
                } else if kind == Some(DockTabKind::Terminal) {
                    TerminalView {}
                } else if kind == Some(DockTabKind::Logs) {
                    LogsView {}
                }
            }
        }
    }
}

/// The bottom dock. Renders nothing at all while it has no tab.
#[component]
pub fn DockView() -> Element {
    // A pod selection opens — or replaces — the focused pane's temporary tab.
    use_effect(move || {
        let selected = SELECTED_POD.read().clone();
        if let Some(pod) = selected {
            DOCK.with_mut(|state| crate::dock::open_pod(state, pod));
        }
    });

    let state = DOCK.read().clone();
    let height = *DOCK_HEIGHT.read();
    let mut manager_open = use_signal(|| false);
    let mut drag_origin = use_signal(|| None::<(f64, u32)>);

    if !state.is_open() {
        return rsx! {};
    }

    let split = state.is_split();
    let panes = pane_rows(&state);
    let tab_count: usize = panes.iter().map(|pane| pane.tabs.len()).sum();
    // The tab the manager's "Close others" keeps: the focused pane's, since
    // pane A's `active` is only the dock's tab while the focus is there.
    let active = state.pane_active(state.focused()).map(str::to_string);
    let active_for_others = active.clone();
    let layout = if split { "split" } else { "single" };
    let focus_key = state.focused().key();
    let toggle_label = if split { "Unsplit" } else { "Split" };
    let toggle_name = if split { "Unsplit dock" } else { "Split dock" };
    let toggle_action = if split { "unsplit" } else { "split" };
    let single_strip = panes.first().map(strip_markup);

    rsx! {
        section {
            class: "dock",
            "data-dock": "open",
            "data-dock-layout": "{layout}",
            "data-dock-focus": "{focus_key}",
            "data-dock-height": "{height}",
            style: "--dock-height: {height}px;",

            div {
                class: "dock-resize",
                "data-dock-resize": "1",
                role: "separator",
                aria_label: "Resize dock",
                onpointerdown: move |event| {
                    let y = event.client_coordinates().y;
                    let start = *DOCK_HEIGHT.read();
                    drag_origin.set(Some((y, start)));
                    event.prevent_default();
                },
                onpointermove: move |event| {
                    let Some((start_y, start_height)) = drag_origin() else {
                        return;
                    };
                    let y = event.client_coordinates().y;
                    let next = crate::dock::resize_by(start_height, start_y - y);
                    if next != *DOCK_HEIGHT.read() {
                        set_dock_height(next);
                    }
                },
                onpointerup: move |_| drag_origin.set(None),
                onpointerleave: move |_| drag_origin.set(None),
            }

            div { class: "dock-bar",
                div { class: "dock-layout",
                    button {
                        class: "dock-layout-btn",
                        r#type: "button",
                        "data-dock-action": "{toggle_action}",
                        aria_label: "{toggle_name}",
                        onclick: move |_| {
                            if split {
                                unsplit_dock();
                            } else {
                                split_dock();
                            }
                        },
                        "{toggle_label}"
                    }
                    button {
                        class: "dock-layout-btn",
                        r#type: "button",
                        "data-dock-action": "swap",
                        aria_label: "Swap panes",
                        // Focusable and announced while single-pane (nothing to
                        // swap yet), so the control never disappears from the
                        // keyboard path; the model's `swap` is a no-op there.
                        "aria-disabled": if split { "false" } else { "true" },
                        title: if split { "Swap the panes" } else { "Split the dock first" },
                        onclick: move |_| swap_dock(),
                        "Swap"
                    }
                }
                if let Some(strip) = single_strip {
                    {strip}
                }
                div { class: "dock-manager",
                    button {
                        class: "dock-manager-btn",
                        r#type: "button",
                        aria_label: "Tab manager",
                        "aria-expanded": if manager_open() { "true" } else { "false" },
                        "data-dock-tab-count": "{tab_count}",
                        onclick: move |_| {
                            let next = !manager_open();
                            manager_open.set(next);
                        },
                        "Tabs"
                    }
                    div {
                        class: if manager_open() { "dock-menu open" } else { "dock-menu" },
                        role: "menu",
                        aria_label: "Open tabs",
                        for pane in panes.iter() {
                            for row in pane.tabs.iter() {
                                {
                                    let switch = row.id.clone();
                                    let close_id = row.id.clone();
                                    rsx! {
                                        div { key: "{row.id}", class: "dock-menu-row",
                                            button {
                                                class: "dock-menu-switch",
                                                r#type: "button",
                                                onclick: move |_| activate_dock_tab(&switch),
                                                "{row.label}"
                                            }
                                            button {
                                                class: "dock-menu-close",
                                                r#type: "button",
                                                aria_label: "Close {row.label}",
                                                onclick: move |_| close_dock_tab(&close_id),
                                                "×"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "dock-menu-actions",
                            button {
                                class: "dock-menu-action",
                                r#type: "button",
                                disabled: active.is_none(),
                                onclick: move |_| {
                                    if let Some(id) = active_for_others.clone() {
                                        close_other_dock_tabs(&id);
                                    }
                                },
                                "Close others"
                            }
                            button {
                                class: "dock-menu-action",
                                r#type: "button",
                                onclick: move |_| close_all_dock_tabs(),
                                "Close all"
                            }
                        }
                    }
                }
            }

            div { class: "dock-panes",
                for pane in panes.iter() {
                    { pane_markup(pane, split) }
                }
            }
        }
    }
}
