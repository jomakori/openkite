//! Bulk selection bar.
//!
//! The table owns a single [`SelectionModel`]; rows and the bar read from it
//! and nothing else, so "how many are checked" has one answer. These tests pin
//! the model's view-scoping (select-all/none only ever touches the current
//! filter result), the keyboard intent mapping, and what the mounted table
//! actually prints.
//!
//! Event dispatch is not available in the SSR harness, so keyboard behaviour is
//! proved two ways: the pure `selection_key` mapping (which key means what) and
//! a source assertion that the row and panel really are wired to it. Anything
//! that needs a live DOM is called out where it happens instead of faked.

mod support;

use dioxus::prelude::*;
use openkite_ui::components::resource_table::{
    selection_key, BulkAction, BulkSelectionBar, Cell, ColumnDef, ResourceRow, ResourceTable,
    SelectionKey, SelectionModel, BULK_EMPTY_REASON,
};
use openkite_ui::components::status_badge::StatusKind;

/// The component source, so the keyboard wiring can be asserted without a DOM.
const RESOURCE_TABLE_SRC: &str = include_str!("../src/components/resource_table.rs");

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
    ]
}

fn row(id: &str) -> ResourceRow {
    ResourceRow {
        id: id.into(),
        namespace: Some("default".into()),
        kind: None,
        cells: vec![Cell::text(id), Cell::status("Running", StatusKind::Running)],
    }
}

fn three_rows() -> Vec<ResourceRow> {
    vec![row("a"), row("b"), row("c")]
}

fn ids(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| id.to_string()).collect()
}

fn table_unseeded() -> Element {
    rsx! {
        ResourceTable { columns: columns(), rows: three_rows() }
    }
}

fn table_seeded() -> Element {
    rsx! {
        ResourceTable {
            columns: columns(),
            rows: three_rows(),
            selected_ids: vec!["a".to_string(), "b".to_string()],
        }
    }
}

fn bar_idle() -> Element {
    rsx! { BulkSelectionBar { count: 0usize } }
}

fn bar_with_action() -> Element {
    rsx! {
        BulkSelectionBar {
            count: 2usize,
            selected: vec!["a".to_string(), "b".to_string()],
            actions: vec![BulkAction {
                id: "delete",
                label: "Delete selected",
                danger: true,
                on_run: None,
            }],
        }
    }
}

fn bar_idle_with_action() -> Element {
    rsx! {
        BulkSelectionBar {
            count: 0usize,
            actions: vec![BulkAction {
                id: "delete",
                label: "Delete selected",
                danger: true,
                on_run: None,
            }],
        }
    }
}

/// Checking rows shows their count.
#[test]
fn checking_rows_shows_the_count() {
    let seeded = support::mount_html(table_seeded, || {});
    assert!(seeded.contains("2 selected"), "got: {seeded}");
    assert!(!seeded.contains("0 selected"), "got: {seeded}");

    let unseeded = support::mount_html(table_unseeded, || {});
    assert!(unseeded.contains("0 selected"), "got: {unseeded}");
}

/// Clearing the selection empties the count — the count is model-driven, not
/// row-driven.
#[test]
fn clearing_the_selection_empties_the_count() {
    let mut model = SelectionModel::from_ids(ids(&["a", "b"]));
    assert_eq!(model.count(), 2);
    model.clear();
    assert_eq!(model.count(), 0);
    assert!(model.is_empty());

    // The same two states through the component: only the seed differs.
    let seeded = support::mount_html(table_seeded, || {});
    let unseeded = support::mount_html(table_unseeded, || {});
    assert!(seeded.contains("2 selected"), "got: {seeded}");
    assert!(unseeded.contains("0 selected"), "got: {unseeded}");
}

/// Escape clears the selection.
///
/// Escape is claimed by `selection_key`, and both handlers that receive it
/// (the row, and the panel that catches it bubbling) are asserted present in
/// the source — the SSR harness cannot dispatch a keydown.
#[test]
fn escape_clears_the_selection() {
    assert_eq!(selection_key("Escape", false), SelectionKey::Clear);
    assert_eq!(selection_key("escape", true), SelectionKey::Clear);

    let mut model = SelectionModel::from_ids(ids(&["a", "b"]));
    if selection_key("Escape", false) == SelectionKey::Clear {
        model.clear();
    }
    assert_eq!(model.count(), 0);
    assert!(model.is_empty());

    assert!(
        RESOURCE_TABLE_SRC.contains("SelectionKey::Clear"),
        "the table must act on the Escape intent"
    );
    let clear_handlers = RESOURCE_TABLE_SRC.matches("onkeydown").count();
    assert!(
        clear_handlers >= 2,
        "Escape must be caught on the row and on the panel, found {clear_handlers} handler(s)"
    );
}

/// With nothing selected the actions are disabled and the bar says why.
#[test]
fn actions_are_disabled_with_a_stated_reason_when_nothing_is_selected() {
    let html = support::mount_html(bar_idle, || {});
    assert!(html.contains(BULK_EMPTY_REASON), "got: {html}");
    assert!(html.contains("disabled"), "got: {html}");
    assert!(
        html.contains("aria-label=\"Bulk selection\""),
        "got: {html}"
    );

    // The table's own bar starts idle, so the reason is on screen before any
    // row is ticked rather than only after a failed click.
    let table = support::mount_html(table_unseeded, || {});
    assert!(table.contains(BULK_EMPTY_REASON), "got: {table}");
    assert!(table.contains("disabled"), "got: {table}");
}

/// Select-all covers the current filter result and nothing beyond it.
#[test]
fn select_all_covers_only_the_current_filter_result() {
    let order = ids(&["a", "b"]);
    let everything = ids(&["a", "b", "c"]);
    let mut model = SelectionModel::default();

    model.select_all(&order);
    assert_eq!(model.count(), 2);
    assert_eq!(model.count_in(&order), 2);
    assert!(model.all_in(&order));
    // "c" was filtered out: selecting the visible rows must not select it.
    assert!(!model.contains("c"));
    assert_eq!(model.count_in(&everything), 2);
    assert_eq!(model.selected_in(&everything), order);
}

/// Select-none clears the current filter result and nothing beyond it.
#[test]
fn select_none_clears_only_the_current_filter_result() {
    let order = ids(&["a", "b"]);
    let mut model = SelectionModel::from_ids(ids(&["a", "b", "c"]));

    model.clear_all(&order);
    assert_eq!(model.count_in(&order), 0);
    assert!(model.selected_in(&order).is_empty());
    // Hiding rows must not become a way to lose a pick they still hold.
    assert!(model.contains("c"));
    assert_eq!(model.count(), 1);
}

/// Shift-range selects every row between the anchor and the target.
#[test]
fn shift_range_selects_every_row_between_anchor_and_target() {
    let order = ids(&["a", "b", "c"]);
    let mut model = SelectionModel::default();

    model.toggle("a");
    model.extend_to("c", &order);
    assert_eq!(model.selected_in(&order), order);

    // Reverse range, same rows.
    let mut back = SelectionModel::default();
    back.toggle("c");
    back.extend_to("a", &order);
    assert_eq!(back.selected_in(&order), order);

    // No anchor yet: a shift-click is just a click.
    let mut fresh = SelectionModel::default();
    fresh.extend_to("b", &order);
    assert_eq!(fresh.selected_in(&order), ids(&["b"]));
}

/// Keyboard: space and x toggle, shift+either extends, Escape clears, the rest
/// is ignored.
#[test]
fn keyboard_space_toggles_and_shift_x_extends() {
    assert_eq!(selection_key(" ", false), SelectionKey::Toggle);
    assert_eq!(selection_key("spacebar", false), SelectionKey::Toggle);
    assert_eq!(selection_key("x", false), SelectionKey::Toggle);
    assert_eq!(selection_key("X", false), SelectionKey::Toggle);
    assert_eq!(selection_key("X", true), SelectionKey::Extend);
    assert_eq!(selection_key(" ", true), SelectionKey::Extend);
    assert_eq!(selection_key("Enter", false), SelectionKey::Ignore);

    let order = ids(&["a", "b", "c"]);
    let mut model = SelectionModel::default();
    for (id, shift) in [("a", false), ("b", false), ("c", true)] {
        match selection_key("x", shift) {
            SelectionKey::Toggle => model.toggle(id),
            SelectionKey::Extend => model.extend_to(id, &order),
            SelectionKey::Clear => model.clear(),
            SelectionKey::Ignore => {}
        }
    }
    assert_eq!(model.ids(), order);
    assert!(model.all_in(&order));

    // Space must not scroll the table: the row handler prevents the default.
    assert!(
        RESOURCE_TABLE_SRC.contains("prevent_default"),
        "space selection must stop the page from scrolling"
    );
}

/// The bar is mounted even at zero, so checking a row never reflows the table.
#[test]
fn bar_is_always_rendered_so_rows_never_reflow() {
    let html = support::mount_html(bar_idle, || {});
    assert!(html.contains("bulk-bar"), "got: {html}");
    assert!(html.contains("0 selected"), "got: {html}");
    assert!(html.contains("data-active=\"false\""), "got: {html}");

    let table = support::mount_html(table_unseeded, || {});
    assert!(
        table.contains("bulk-bar"),
        "the table always reserves the bar: {table}"
    );
    assert!(table.contains("0 selected"), "got: {table}");

    let seeded = support::mount_html(table_seeded, || {});
    assert!(seeded.contains("data-active=\"true\""), "got: {seeded}");
}

/// The bar's buttons are tab-reachable (no tabindex="-1" trap).
#[test]
fn bar_buttons_are_tab_reachable() {
    let html = support::mount_html(bar_with_action, || {});
    assert!(html.contains("<button"), "got: {html}");
    assert!(!html.contains("tabindex=\"-1\""), "got: {html}");
    assert!(html.contains("Clear"), "got: {html}");
}

/// Rows and their checkboxes carry the labels a screen reader needs.
#[test]
fn row_checkboxes_and_rows_carry_accessible_labels() {
    let html = support::mount_html(table_seeded, || {});
    assert!(html.contains("aria-label=\"Select a\""), "got: {html}");
    assert!(
        html.contains("aria-label=\"Select all rows in this view\""),
        "got: {html}"
    );
    assert!(html.contains("role=\"row\""), "got: {html}");
    assert!(html.contains("data-row-id=\"a\""), "got: {html}");
    assert!(html.contains("tabindex=\"0\""), "got: {html}");
}

/// Host-wired actions render with the count, and are disabled when idle.
#[test]
fn bulk_bar_actions_render_with_the_selected_count_when_the_host_wires_them() {
    let html = support::mount_html(bar_with_action, || {});
    assert!(html.contains("2 selected"), "got: {html}");
    assert!(html.contains("Delete selected"), "got: {html}");
    assert!(html.contains("btn btn-danger"), "got: {html}");

    let idle = support::mount_html(bar_idle_with_action, || {});
    assert!(idle.contains("Delete selected"), "got: {idle}");
    assert!(idle.contains("disabled"), "got: {idle}");
}
