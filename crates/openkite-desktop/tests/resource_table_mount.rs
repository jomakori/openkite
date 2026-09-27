//! Headless mounts of the ResourceTable + HealthDots render bodies
//! (coverage roadmap B5). The pure sort/filter/window helpers are already
//! pinned in tests/resource_table.rs; these run the `#[component]` code:
//! Loading / Error / empty-state / populated rows + row-action buttons.

mod support;

use dioxus::prelude::*;
use openkite::components::resource_table::{
    Cell, ColumnDef, HealthDot, HealthDots, ResourceRow, ResourceTable, RowActions, TableStatus,
};
use openkite::components::status_badge::StatusKind;

fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            key: "name",
            label: "Name",
            width: Some(180),
            sortable: true,
        },
        ColumnDef {
            key: "namespace",
            label: "Namespace",
            width: None,
            sortable: true,
        },
    ]
}

fn row(id: &str, namespace: Option<&str>, cells: Vec<Cell>) -> ResourceRow {
    ResourceRow {
        id: id.into(),
        namespace: namespace.map(str::to_string),
        cells,
    }
}

fn two_rows() -> Vec<ResourceRow> {
    vec![
        row(
            "nginx-1",
            Some("default"),
            vec![
                Cell::text("nginx"),
                Cell::status("Running", StatusKind::Running),
            ],
        ),
        row(
            "redis-1",
            Some("default"),
            vec![
                Cell::text("redis"),
                Cell::status("Pending", StatusKind::Pending),
            ],
        ),
    ]
}

fn actions() -> Option<RowActions> {
    Some(RowActions {
        on_delete: Some(EventHandler::new(|_| {})),
        on_edit: Some(EventHandler::new(|_| {})),
        on_scale: Some(EventHandler::new(|_| {})),
    })
}

fn table_loading() -> Element {
    rsx! {
        ResourceTable {
            columns: Vec::new(),
            rows: Vec::new(),
            status: TableStatus::Loading,
        }
    }
}

fn table_error() -> Element {
    rsx! {
        ResourceTable {
            columns: Vec::new(),
            rows: Vec::new(),
            status: TableStatus::Error("cluster unreachable".to_string()),
        }
    }
}

fn table_empty_default() -> Element {
    rsx! {
        ResourceTable {
            columns: Vec::new(),
            rows: Vec::new(),
        }
    }
}

fn table_empty_custom() -> Element {
    rsx! {
        ResourceTable {
            columns: Vec::new(),
            rows: Vec::new(),
            empty_message: Some("Nothing in this namespace".to_string()),
        }
    }
}

fn table_populated() -> Element {
    rsx! {
        ResourceTable {
            columns: columns(),
            rows: two_rows(),
            row_actions: actions(),
        }
    }
}

fn table_health_dot_rows() -> Element {
    rsx! {
        ResourceTable {
            columns: columns(),
            rows: vec![row(
                "pod-a",
                Some("default"),
                vec![
                    Cell::health_dots(vec![HealthDot::Ok, HealthDot::Ok, HealthDot::Err]),
                    Cell::number("250m", 0.25),
                ],
            )],
        }
    }
}

fn table_empty_dot_rows() -> Element {
    rsx! {
        ResourceTable {
            columns: columns(),
            rows: vec![row(
                "pod-b",
                None,
                vec![Cell::health_dots(Vec::new()), Cell::number("—", 0.0)],
            )],
        }
    }
}

fn dots_component() -> Element {
    rsx! { HealthDots { ready: 2, total: 3 } }
}

fn dots_component_none() -> Element {
    rsx! { HealthDots { ready: 0, total: 0 } }
}

#[test]
fn loading_state_renders_loading_text() {
    let html = support::mount_html(table_loading, || {});
    assert!(html.contains("Loading…"), "got: {html}");
}

#[test]
fn error_state_renders_message() {
    let html = support::mount_html(table_error, || {});
    assert!(html.contains("table-error"), "got: {html}");
    assert!(html.contains("cluster unreachable"), "got: {html}");
}

#[test]
fn ready_with_no_rows_shows_default_empty_message() {
    let html = support::mount_html(table_empty_default, || {});
    assert!(html.contains("table-empty"), "got: {html}");
    assert!(html.contains("No resources"), "got: {html}");
}

#[test]
fn ready_with_no_rows_shows_custom_empty_message() {
    let html = support::mount_html(table_empty_custom, || {});
    assert!(html.contains("Nothing in this namespace"), "got: {html}");
    assert!(!html.contains("No resources"), "got: {html}");
}

#[test]
fn populated_table_renders_headers_rows_and_actions() {
    let html = support::mount_html(table_populated, || {});
    // Header cells.
    assert!(html.contains("Name"), "got: {html}");
    assert!(html.contains("Namespace"), "got: {html}");
    // Rows + their cell text.
    assert!(html.contains("nginx"), "got: {html}");
    assert!(html.contains("redis"), "got: {html}");
    assert!(html.contains("Running"), "got: {html}");
    assert!(html.contains("Pending"), "got: {html}");
    // Namespace chips ("All" + the row namespaces).
    assert!(html.contains("All"), "got: {html}");
    assert!(html.contains("chip active"), "got: {html}");
    // Wired row-action buttons.
    assert!(html.contains("Delete"), "got: {html}");
    assert!(html.contains("Edit"), "got: {html}");
    assert!(html.contains("Scale"), "got: {html}");
    assert!(html.contains("row-action"), "got: {html}");
}

#[test]
fn health_dot_cells_render_without_panicking() {
    let html = support::mount_html(table_health_dot_rows, || {});
    assert!(html.contains("health-dots"), "got: {html}");
    assert!(html.contains("250m"), "got: {html}");
}

#[test]
fn empty_health_dot_cell_falls_back_to_dash() {
    let html = support::mount_html(table_empty_dot_rows, || {});
    assert!(html.contains("—"), "got: {html}");
}

#[test]
fn health_dots_component_renders_ok_and_err_dots() {
    let html = support::mount_html(dots_component, || {});
    assert!(html.contains("dot ok"), "got: {html}");
    assert!(html.contains("dot err"), "got: {html}");
}

#[test]
fn health_dots_component_zero_total_renders_empty() {
    let html = support::mount_html(dots_component_none, || {});
    assert!(html.contains("health-dots"), "got: {html}");
}
