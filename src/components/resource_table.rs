//! Virtualized, sortable, filterable resource table.
//!
//! Split into pure logic (sort / filter / windowing) — generic and unit-tested —
//! and the Dioxus components that wrap it. The workload view consumes
//! `ResourceRow`/`ResourceTable`; the remaining exports are shared building
//! blocks.

// Several building blocks have no consumer yet.
#![allow(dead_code)]
#![allow(non_snake_case)]

use std::cmp::Ordering;
use std::ops::Range;

use dioxus::prelude::*;

use crate::components::status_badge::{StatusBadge, StatusKind};

/// Fixed row height for virtualization, in pixels.
pub const ROW_HEIGHT: f64 = 36.0;

/// Rows rendered beyond the viewport on each side to hide scroll jank.
pub const OVERSCAN: usize = 8;

/// Sort direction for a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    /// Flip to the opposite direction.
    pub fn toggle(self) -> Self {
        match self {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        }
    }
}

/// Normalized sort key so text and numeric columns order sanely.
#[derive(Debug, Clone, PartialEq)]
pub enum SortKey {
    Text(String),
    Number(f64),
}

/// Compare two sort keys: numbers order before text; text is case-insensitive.
pub fn compare_sort_keys(a: &SortKey, b: &SortKey) -> Ordering {
    match (a, b) {
        (SortKey::Number(x), SortKey::Number(y)) => x.total_cmp(y),
        (SortKey::Number(_), SortKey::Text(_)) => Ordering::Less,
        (SortKey::Text(_), SortKey::Number(_)) => Ordering::Greater,
        (SortKey::Text(x), SortKey::Text(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
    }
}

/// Stable-sort `rows` by `key_of` in `direction`. Rows with equal keys keep
/// their relative order (`slice::sort_by` is stable).
pub fn sort_by_key<T, F>(rows: &mut [T], key_of: F, direction: SortDirection)
where
    F: Fn(&T) -> SortKey,
{
    rows.sort_by(|a, b| {
        let ord = compare_sort_keys(&key_of(a), &key_of(b));
        match direction {
            SortDirection::Ascending => ord,
            SortDirection::Descending => ord.reverse(),
        }
    });
}

/// Case-insensitive substring match. A blank query matches everything.
pub fn matches_query(text: &str, query: &str) -> bool {
    let needle = query.trim().to_lowercase();
    needle.is_empty() || text.to_lowercase().contains(&needle)
}

/// Visible index range of a virtualized list given the scroll offset.
pub fn visible_range(scroll_top: f64, viewport_height: f64, total_rows: usize) -> Range<usize> {
    if total_rows == 0 || viewport_height <= 0.0 {
        return 0..0;
    }
    let first_visible = (scroll_top / ROW_HEIGHT).floor() as usize;
    let visible = (viewport_height / ROW_HEIGHT).ceil() as usize + 1;
    let start = first_visible.saturating_sub(OVERSCAN);
    let end = (first_visible + visible + OVERSCAN).min(total_rows);
    start..end
}

/// A single table cell: display text, optional status badge, and a sort key.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub text: String,
    pub status: Option<StatusKind>,
    pub sort: SortKey,
}

impl Cell {
    /// Plain-text cell; sorts by its text.
    pub fn text(text: impl Into<String>) -> Self {
        let text = text.into();
        let sort = SortKey::Text(text.clone());
        Self {
            text,
            sort,
            status: None,
        }
    }

    /// Status cell rendered as a badge; sorts by its label.
    pub fn status(label: &str, kind: StatusKind) -> Self {
        let text = label.to_string();
        let sort = SortKey::Text(text.clone());
        Self {
            text,
            sort,
            status: Some(kind),
        }
    }

    /// Numeric cell; sorts numerically, displays `text`.
    pub fn number(text: impl Into<String>, value: f64) -> Self {
        Self {
            text: text.into(),
            sort: SortKey::Number(value),
            status: None,
        }
    }
}

/// A concrete, display-ready table row.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceRow {
    pub id: String,
    pub namespace: Option<String>,
    pub cells: Vec<Cell>,
}

impl ResourceRow {
    /// Concatenated text of every cell + namespace, for filtering.
    pub fn search_text(&self) -> String {
        let mut out = String::new();
        if let Some(namespace) = &self.namespace {
            out.push_str(namespace);
            out.push(' ');
        }
        for cell in &self.cells {
            out.push_str(&cell.text);
            out.push(' ');
        }
        out
    }
}

/// A column definition (cells are pre-built; no per-cell render closure).
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub key: &'static str,
    pub label: &'static str,
    pub width: Option<u32>,
    pub sortable: bool,
}

/// External readiness of the table's data source.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TableStatus {
    #[default]
    Ready,
    Loading,
    Error(String),
}

/// Per-row action callbacks, wired by the consuming view.
/// `EventHandler` does not implement `Debug`, so this stays `Clone + PartialEq`
/// only — exactly what Dioxus props require.
#[derive(Clone, PartialEq, Default)]
pub struct RowActions {
    pub on_delete: Option<EventHandler<String>>,
    pub on_edit: Option<EventHandler<String>>,
    pub on_scale: Option<EventHandler<String>>,
}

/// Flip sort state when a (sortable) column header is clicked.
fn toggle_sort(mut sort: Signal<Option<(usize, SortDirection)>>, column: usize) {
    let next = match sort() {
        Some((active, direction)) if active == column => (column, direction.toggle()),
        _ => (column, SortDirection::Ascending),
    };
    sort.set(Some(next));
}

/// Virtualized, sortable, filterable resource table.
#[component]
pub fn ResourceTable(
    columns: Vec<ColumnDef>,
    rows: Vec<ResourceRow>,
    #[props(default)] status: TableStatus,
    #[props(default)] empty_message: Option<String>,
    #[props(default)] row_actions: Option<RowActions>,
    #[props(default = 600.0)] height: f64,
) -> Element {
    let sort = use_signal(|| None::<(usize, SortDirection)>);
    let mut query = use_signal(String::new);
    let namespace = use_signal(|| None::<String>);

    match status {
        TableStatus::Loading => rsx! { div { class: "table-state", "Loading…" } },
        TableStatus::Error(message) => {
            rsx! { div { class: "table-state table-error", "{message}" } }
        }
        TableStatus::Ready => {
            let mut view: Vec<ResourceRow> = rows
                .iter()
                .filter(|row| {
                    let ns_ok = match namespace() {
                        Some(ns) => row.namespace.as_deref() == Some(ns.as_str()),
                        None => true,
                    };
                    ns_ok && matches_query(&row.search_text(), &query())
                })
                .cloned()
                .collect();
            if let Some((column, direction)) = sort() {
                sort_by_key(&mut view, |row| row.cells[column].sort.clone(), direction);
            }

            if view.is_empty() {
                let message = empty_message.unwrap_or_else(|| "No resources".to_string());
                return rsx! { div { class: "table-state table-empty", "{message}" } };
            }

            let mut namespaces: Vec<String> = rows
                .iter()
                .filter_map(|row| row.namespace.clone())
                .collect();
            namespaces.sort();
            namespaces.dedup();
            let chips: Vec<(String, bool)> = namespaces
                .iter()
                .map(|ns| (ns.clone(), namespace().as_deref() == Some(ns.as_str())))
                .collect();

            rsx! {
                div { class: "resource-table",
                    div { class: "table-toolbar",
                        input {
                            class: "table-filter",
                            placeholder: "Filter…",
                            value: "{query}",
                            oninput: move |event| query.set(event.value()),
                        }
                        for (label, active) in chips.into_iter() {
                            { namespace_chip(label, active, namespace) }
                        }
                    }
                    div { class: "table-header table-row",
                        for (i, column) in columns.iter().enumerate() {
                            { header_cell(i, column, sort) }
                        }
                    }
                    TableBody {
                        view,
                        columns: columns.clone(),
                        row_actions: row_actions.clone(),
                        height,
                    }
                }
            }
        }
    }
}

/// Render a single column header cell (with sort toggle + indicator).
fn header_cell(
    index: usize,
    column: &ColumnDef,
    sort: Signal<Option<(usize, SortDirection)>>,
) -> Element {
    let key = column.key;
    let label = column.label;
    let width = column.width;
    let sortable = column.sortable;
    let direction = sort().and_then(|(active, dir)| (active == index).then_some(dir));
    rsx! {
        div {
            key: "{key}",
            class: "table-cell",
            style: width.map(|w| format!("width: {w}px")).unwrap_or_default(),
            onclick: move |_| {
                if sortable {
                    toggle_sort(sort, index);
                }
            },
            span { "{label}" }
            if let Some(dir) = direction {
                span {
                    class: "sort-indicator",
                    if dir == SortDirection::Ascending { "▲" } else { "▼" }
                }
            }
        }
    }
}

/// Render a namespace filter chip.
fn namespace_chip(label: String, active: bool, namespace: Signal<Option<String>>) -> Element {
    let label_for_click = label.clone();
    rsx! {
        button {
            key: "{label}",
            class: if active { "ns-chip active" } else { "ns-chip" },
            onclick: move |_| {
                let mut namespace = namespace;
                let selected = namespace().as_deref() == Some(label_for_click.as_str());
                namespace.set(if selected { None } else { Some(label_for_click.clone()) });
            },
            "{label}"
        }
    }
}

/// The scrollable, virtualized body of the table. Holds scroll state so that
/// scrolling re-renders only the visible window, never the full sorted list.
#[component]
fn TableBody(
    view: Vec<ResourceRow>,
    columns: Vec<ColumnDef>,
    row_actions: Option<RowActions>,
    height: f64,
) -> Element {
    let mut scroll_top = use_signal(|| 0.0f64);

    let total = view.len();
    let range = visible_range(scroll_top(), height, total);
    let start = range.start;
    let slice = &view[range];
    let total_height = total as f64 * ROW_HEIGHT;
    let widths: Vec<Option<u32>> = columns.iter().map(|column| column.width).collect();

    rsx! {
        div {
            class: "table-body",
            style: "height: {height}px; overflow-y: auto; position: relative;",
            onscroll: move |event| scroll_top.set(event.scroll_top()),
            div { style: "height: {total_height}px; position: relative;",
                for (offset, row) in slice.iter().enumerate() {
                    { render_table_row(row, offset, start, &widths, row_actions.clone()) }
                }
            }
        }
    }
}

/// Render a single virtualized table row (absolute-positioned).
fn render_table_row(
    row: &ResourceRow,
    offset: usize,
    start: usize,
    widths: &[Option<u32>],
    row_actions: Option<RowActions>,
) -> Element {
    let top = (start + offset) as f64 * ROW_HEIGHT;
    let row_id = row.id.clone();
    rsx! {
        div {
            key: "{row_id}",
            class: "table-row",
            style: "position: absolute; top: {top}px; height: {ROW_HEIGHT}px; left: 0; right: 0;",
            for (i, cell) in row.cells.iter().enumerate() {
                { render_table_cell(cell, i, widths.get(i).copied().flatten()) }
            }
            if let Some(actions) = row_actions {
                { render_row_actions(&row_id, &actions) }
            }
        }
    }
}

/// Render a single table cell (plain text or status badge).
fn render_table_cell(cell: &Cell, index: usize, width: Option<u32>) -> Element {
    let style = width.map(|w| format!("width: {w}px")).unwrap_or_default();
    match cell.status {
        Some(kind) => rsx! {
            div {
                key: "{index}",
                class: "table-cell",
                style: "{style}",
                StatusBadge { status: kind }
            }
        },
        None => rsx! {
            div {
                key: "{index}",
                class: "table-cell",
                style: "{style}",
                span { "{cell.text}" }
            }
        },
    }
}

/// Render the per-row action buttons (delete/edit/scale) when wired.
fn render_row_actions(row_id: &str, actions: &RowActions) -> Element {
    rsx! {
        div { class: "table-cell table-actions",
            if let Some(handler) = &actions.on_delete {
                { render_action_button("Delete", "row-action danger", row_id, handler) }
            }
            if let Some(handler) = &actions.on_edit {
                { render_action_button("Edit", "row-action", row_id, handler) }
            }
            if let Some(handler) = &actions.on_scale {
                { render_action_button("Scale", "row-action", row_id, handler) }
            }
        }
    }
}

/// Render a single row-action button with its callback.
fn render_action_button(
    label: &'static str,
    class: &'static str,
    row_id: &str,
    handler: &EventHandler<String>,
) -> Element {
    let id = row_id.to_string();
    let handler = *handler;
    rsx! {
        button {
            class: "{class}",
            onclick: move |_| handler.call(id.clone()),
            "{label}"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_direction_toggles() {
        assert_eq!(SortDirection::Ascending.toggle(), SortDirection::Descending);
        assert_eq!(SortDirection::Descending.toggle(), SortDirection::Ascending);
    }

    #[test]
    fn compare_sort_keys_orders_numbers_before_text() {
        assert_eq!(
            compare_sort_keys(&SortKey::Number(1.0), &SortKey::Number(2.0)),
            Ordering::Less
        );
        assert_eq!(
            compare_sort_keys(&SortKey::Number(1.0), &SortKey::Text("a".into())),
            Ordering::Less
        );
        assert_eq!(
            compare_sort_keys(&SortKey::Text("a".into()), &SortKey::Number(1.0)),
            Ordering::Greater
        );
    }

    #[test]
    fn compare_sort_keys_is_case_insensitive() {
        assert_eq!(
            compare_sort_keys(
                &SortKey::Text("apple".into()),
                &SortKey::Text("APPLE".into())
            ),
            Ordering::Equal
        );
        assert_eq!(
            compare_sort_keys(
                &SortKey::Text("Apple".into()),
                &SortKey::Text("banana".into())
            ),
            Ordering::Less
        );
    }

    #[test]
    fn sort_by_key_orders_both_directions() {
        #[derive(Debug, Clone, PartialEq)]
        struct Item {
            name: &'static str,
            cpu: f64,
        }
        let mut rows = vec![
            Item {
                name: "b",
                cpu: 2.0,
            },
            Item {
                name: "a",
                cpu: 1.0,
            },
            Item {
                name: "c",
                cpu: 3.0,
            },
        ];
        sort_by_key(
            &mut rows,
            |item| SortKey::Number(item.cpu),
            SortDirection::Ascending,
        );
        assert_eq!(
            rows.iter().map(|i| i.name).collect::<Vec<_>>(),
            ["a", "b", "c"]
        );

        sort_by_key(
            &mut rows,
            |item| SortKey::Number(item.cpu),
            SortDirection::Descending,
        );
        assert_eq!(
            rows.iter().map(|i| i.name).collect::<Vec<_>>(),
            ["c", "b", "a"]
        );
    }

    #[test]
    fn sort_by_key_is_stable() {
        #[derive(Debug, Clone, PartialEq)]
        struct Item {
            id: u32,
            group: u32,
        }
        let mut rows = vec![
            Item { id: 1, group: 0 },
            Item { id: 2, group: 0 },
            Item { id: 3, group: 0 },
        ];
        sort_by_key(
            &mut rows,
            |item| SortKey::Number(item.group as f64),
            SortDirection::Ascending,
        );
        assert_eq!(rows.iter().map(|i| i.id).collect::<Vec<_>>(), [1, 2, 3]);
    }

    #[test]
    fn matches_query_is_case_insensitive_substring() {
        assert!(matches_query("nginx-deployment", "nginx"));
        assert!(matches_query("nginx-deployment", "NGINX"));
        assert!(matches_query("nginx-deployment", ""));
        assert!(matches_query("nginx-deployment", "   "));
        assert!(!matches_query("nginx-deployment", "redis"));
    }

    #[test]
    fn visible_range_is_bounded_for_large_lists() {
        let range = visible_range(0.0, 600.0, 10_000);
        assert_eq!(range.start, 0);
        assert!(
            range.end - range.start < 50,
            "window must be bounded, got {}",
            range.end - range.start
        );
        assert!(range.end <= 10_000);
    }

    #[test]
    fn visible_range_scrolls_with_offset() {
        let range = visible_range(360.0, 600.0, 1000);
        assert!(range.start > 0);
        assert!(range.start <= 10 + OVERSCAN);
    }

    #[test]
    fn visible_range_is_empty_without_rows() {
        assert_eq!(visible_range(0.0, 600.0, 0), 0..0);
        assert_eq!(visible_range(0.0, 0.0, 10), 0..0);
    }

    #[test]
    fn cell_constructors_set_expected_sort_keys() {
        let text = Cell::text("nginx");
        assert_eq!(text.sort, SortKey::Text("nginx".into()));
        assert_eq!(text.status, None);

        let status = Cell::status("Running", StatusKind::Running);
        assert_eq!(status.sort, SortKey::Text("Running".into()));
        assert_eq!(status.status, Some(StatusKind::Running));

        let number = Cell::number("250m", 0.25);
        assert_eq!(number.sort, SortKey::Number(0.25));
    }

    #[test]
    fn search_text_includes_namespace_and_cells() {
        let row = ResourceRow {
            id: "1".into(),
            namespace: Some("default".into()),
            cells: vec![
                Cell::text("nginx"),
                Cell::status("Running", StatusKind::Running),
            ],
        };
        let haystack = row.search_text();
        assert!(haystack.contains("default"));
        assert!(haystack.contains("nginx"));
        assert!(haystack.contains("Running"));
    }
}
