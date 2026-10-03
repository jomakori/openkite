//! The bottom dock: a tab strip, a dropdown tab manager and the active
//! terminal or log surface.
//!
//! One tab is the reusable temporary one a pod selection owns; the rest are
//! permanent. The rules live in [`crate::dock`]; this module renders them and
//! forwards every mutation to [`crate::runtime`] so the state outlives the
//! component's own re-renders.

use dioxus::prelude::*;

use crate::components::logs::LogsView;
use crate::components::terminal::TerminalView;
use crate::dock::{DockState, DockTabKind};
use crate::runtime::{
    activate_dock_tab, close_all_dock_tabs, close_dock_tab, close_other_dock_tabs,
    promote_dock_tab, set_dock_height, DOCK, DOCK_HEIGHT, SELECTED_POD,
};

/// A tab as the strip and the manager render it.
struct TabRow {
    id: String,
    label: String,
    active: bool,
    temporary: bool,
}

fn tab_rows(state: &DockState) -> Vec<TabRow> {
    state
        .tabs
        .iter()
        .map(|tab| TabRow {
            id: tab.id.clone(),
            label: tab.label(),
            active: state.active.as_deref() == Some(tab.id.as_str()),
            temporary: tab.temporary,
        })
        .collect()
}

/// The bottom dock. Renders nothing at all while it has no tab.
#[component]
pub fn DockView() -> Element {
    // A pod selection opens — or replaces — the dock's one temporary tab.
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

    if state.tabs.is_empty() {
        return rsx! {};
    }

    let rows = tab_rows(&state);
    let tab_count = rows.len();
    let active = state.active.clone();
    let active_for_others = active.clone();
    let active_kind = state
        .active
        .as_deref()
        .and_then(|id| state.tab(id))
        .map(|tab| tab.kind);

    rsx! {
        section {
            class: "dock",
            "data-dock": "open",
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
                div { class: "dock-tabs", role: "tablist", aria_label: "Dock tabs",
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
                        for row in rows.iter() {
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

            div { class: "dock-body",
                if active_kind == Some(DockTabKind::Terminal) {
                    TerminalView {}
                } else if active_kind == Some(DockTabKind::Logs) {
                    LogsView {}
                }
            }
        }
    }
}
