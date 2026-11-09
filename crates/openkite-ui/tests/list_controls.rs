//! OKT-204: the list's live controls, mounted for real.
//!
//! The facet logic and the picker's prefs are unit-tested next to their code;
//! these mounts cover the wiring the user actually touches — the toolbar shows
//! the three facets and clear-all, narrowing the list narrows the rendered
//! rows, and a column choice made in one mount is honoured by the next one
//! (a route round-trip remounts the table, so "next mount" is "next visit").

mod support;

use dioxus::prelude::*;
use openkite_ui::components::column_picker::{
    column_prefs, picker_action, restore_focus_script, show_all_columns, toggle_column,
    ColumnPicker, PickerAction,
};
use openkite_ui::components::resource_table::{Cell, ColumnDef, ResourceRow, ResourceTable};
use openkite_ui::components::status_badge::StatusKind;
use openkite_ui::runtime;

fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            key: "name".into(),
            label: "Name".into(),
            width: Some(180),
            sortable: true,
        },
        ColumnDef {
            key: "namespace".into(),
            label: "Namespace".into(),
            width: None,
            sortable: true,
        },
        ColumnDef {
            key: "node".into(),
            label: "Node".into(),
            width: None,
            sortable: false,
        },
    ]
}

fn row(name: &str, namespace: &str, kind: &str, status: StatusKind, node: &str) -> ResourceRow {
    ResourceRow {
        id: format!("{namespace}/{name}"),
        namespace: Some(namespace.to_string()),
        kind: Some(kind.to_string()),
        cells: vec![
            Cell::text(name),
            Cell::status("Running", status),
            Cell::text(node),
        ],
    }
}

fn rows() -> Vec<ResourceRow> {
    vec![
        row("web-1", "default", "Pod", StatusKind::Running, "node-a"),
        row("web-2", "default", "Pod", StatusKind::CrashLoop, "node-a"),
        row("db-0", "data", "StatefulSet", StatusKind::Pending, "node-b"),
    ]
}

fn table() -> Element {
    rsx! {
        ResourceTable { columns: columns(), rows: rows() }
    }
}

/// The picker owns no state of its own beyond "open": the table hands it the
/// visibility signal, so the mount has to look like a table, not a bare picker.
#[component]
fn PickerHost() -> Element {
    let prefs = use_signal(column_prefs);
    rsx! {
        ColumnPicker { columns: columns(), prefs, initial_open: true }
    }
}

fn picker_open() -> Element {
    rsx! {
        PickerHost {}
    }
}

/// The session store and the namespace selection are process-wide, so the
/// tests in this binary take turns: one test's "hide a column" must not land in
/// the middle of another's render.
fn session_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Every test starts from "all columns, all namespaces": the globals persist
/// across mounts inside one test binary.
fn reset() {
    show_all_columns();
    runtime::clear_namespace_selection();
}

#[test]
fn toolbar_offers_namespace_health_and_kind_facets_plus_clear_all() {
    let _turn = session_lock();
    let html = support::mount_html(table, reset);
    assert!(html.contains("list-filters"), "got: {html}");
    assert!(html.contains("data-facet=\"namespace\""), "got: {html}");
    assert!(html.contains("data-facet=\"health\""), "got: {html}");
    assert!(html.contains("data-facet=\"kind\""), "got: {html}");
    assert!(html.contains("data-value=\"all\""), "got: {html}");
    assert!(html.contains("Clear all"), "got: {html}");
    // Chips carry how many rows sit behind them.
    assert!(html.contains("facet-count"), "got: {html}");
    assert!(
        html.contains("StatefulSet"),
        "kinds are data-derived: {html}"
    );
}

#[test]
fn namespace_facet_narrows_rows_and_clear_all_restores_them() {
    let _turn = session_lock();
    let all = support::mount_html(table, reset);
    assert_eq!(all.matches("class=\"table-row").count(), 3);

    let scoped = support::mount_html(table, || {
        reset();
        runtime::set_namespace_selection(vec!["data".to_string()]);
    });
    assert!(scoped.contains("db-0"), "got: {scoped}");
    assert!(!scoped.contains("web-1"), "narrowed: {scoped}");
    assert_eq!(scoped.matches("class=\"table-row").count(), 1);
    // The namespace chip that filters is marked pressed.
    assert!(
        scoped.contains("data-value=\"data\""),
        "the active facet is visible: {scoped}"
    );

    let restored = support::mount_html(table, reset);
    assert!(restored.contains("web-1"), "clear-all restores: {restored}");
    assert_eq!(restored.matches("class=\"table-row").count(), 3);
}

#[test]
fn column_picker_toggles_a_column_off_and_back_on() {
    let _turn = session_lock();
    let before = support::mount_html(table, reset);
    assert!(before.contains("data-label=\"Node\""), "got: {before}");

    toggle_column("node", &columns());
    let hidden = support::mount_html(table, || {});
    assert!(!hidden.contains("Node"), "header gone: {hidden}");
    assert!(
        !hidden.contains("data-label=\"Node\""),
        "cells gone: {hidden}"
    );
    assert!(
        hidden.contains("data-label=\"Name\""),
        "others stay: {hidden}"
    );

    toggle_column("node", &columns());
    let shown = support::mount_html(table, || {});
    assert!(shown.contains("Node"), "header back: {shown}");
    assert!(shown.contains("data-label=\"Node\""), "cells back: {shown}");
}

#[test]
fn column_choice_survives_a_remount() {
    let _turn = session_lock();
    toggle_column("node", &columns());
    assert!(!column_prefs().is_visible("node"));
    // Two mounts in a row: this is the route round-trip — the table is built
    // fresh, the session preference is not.
    for _ in 0..2 {
        let html = support::mount_html(table, reset_columns_only);
        assert!(!html.contains("data-label=\"Node\""), "got: {html}");
    }
    show_all_columns();
}

fn reset_columns_only() {
    runtime::clear_namespace_selection();
}

#[test]
fn expanded_picker_lists_every_column_with_its_checked_state() {
    let _turn = session_lock();
    show_all_columns();
    let html = support::mount_html(picker_open, || {});
    assert!(html.contains("column-picker-panel"), "got: {html}");
    assert!(html.contains("aria-expanded=\"true\""), "got: {html}");
    assert_eq!(html.matches("column-picker-option").count(), 3);

    toggle_column("node", &columns());
    let html = support::mount_html(picker_open, || {});
    assert_eq!(
        html.matches("type=\"checkbox\"").count(),
        3,
        "every column keeps a checkbox even when unchecked: {html}"
    );
    assert!(html.contains("2/3"), "visible/total: {html}");
    show_all_columns();
}

#[test]
fn escape_closes_the_picker_and_returns_focus_to_its_trigger() {
    let _turn = session_lock();
    // The keyboard contract the component's onkeydown applies.
    assert_eq!(picker_action(&Key::Escape), PickerAction::CloseAndRefocus);
    for key in [Key::Tab, Key::Enter, Key::Character(" ".into())] {
        assert_eq!(
            picker_action(&key),
            PickerAction::Ignore,
            "only Escape is the picker's own key"
        );
    }
    let script = restore_focus_script("column-picker-3");
    assert!(script.contains("column-picker-3"), "{script}");
    assert!(script.contains(".column-picker-trigger"), "{script}");

    // The trigger is the Tab stop and the panel is its popup.
    let html = support::mount_html(picker_open, || {});
    assert!(html.contains("aria-haspopup=\"true\""), "got: {html}");
    assert!(
        html.contains("class=\"btn btn-secondary column-picker-trigger\""),
        "the trigger is a plain button, so Enter/Space activate it: {html}"
    );
}

/// The picker's DOM id feeds the refocus script and must survive a second
/// render unchanged, or server and client disagree and the empty result can
/// hide behind a "ready" panel.
#[test]
fn the_picker_id_is_derived_from_the_columns_not_a_render_counter() {
    let _turn = session_lock();
    let first = support::mount_html(picker_open, || {});
    let second = support::mount_html(picker_open, || {});
    let id = |html: &str| {
        html.split("id=\"column-picker-")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .map(str::to_string)
    };
    assert_eq!(
        id(&first),
        id(&second),
        "two renders of the same columns must mint the same id"
    );
}

/// An empty result is an empty surface: the panel carries that state so the
/// state gates read "empty" rather than the "ready" its toolbar would imply.
#[test]
fn an_empty_result_paints_the_empty_state_on_the_panel() {
    let _turn = session_lock();
    fn empty_table() -> Element {
        rsx! {
            ResourceTable {
                columns: columns(),
                rows: Vec::<ResourceRow>::new(),
                empty_message: Some("No pods in the gateway's scope".to_string()),
            }
        }
    }
    let html = support::mount_html(empty_table, reset);
    assert!(html.contains("data-state=\"empty\""), "got: {html}");
    assert!(
        !html.contains("data-state=\"ready\""),
        "an empty table must not paint ready: {html}"
    );
    assert!(
        html.contains("No pods"),
        "the empty message survives the state change: {html}"
    );
}
