//! Virtualized, sortable, filterable resource table.
//!
//! Split into pure logic (sort / filter / windowing) — generic and unit-tested —
//! and the Dioxus components that wrap it. The workload view consumes
//! `ResourceRow`/`ResourceTable`; the remaining exports are shared building
//! blocks.
#![allow(dead_code)]
#![allow(non_snake_case)]

use std::cmp::Ordering;
use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;

use dioxus::html::{Key, ModifiersInteraction};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::column_picker::{column_prefs, ColumnPicker};
use crate::components::filter_bar::{filter_rows, FacetSelection, FilterBar};
use crate::components::namespace_bar::selection_matches;
use crate::components::status_badge::{StatusKind, StatusPill};

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Case-insensitive substring match against a trimmed query.
pub fn matches_query(text: &str, query: &str) -> bool {
    let needle = query.trim().to_lowercase();
    needle.is_empty() || text.to_lowercase().contains(&needle)
}

/// Scope rows to the console's namespace selection; an empty selection shows
/// every row, and cluster-scoped rows (no namespace) stay visible.
pub fn namespace_filter(rows: &[ResourceRow], selected: &[String]) -> Vec<ResourceRow> {
    rows.iter()
        .filter(|row| selection_matches(selected, row.namespace.as_deref()))
        .cloned()
        .collect()
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub text: String,
    pub status: Option<StatusKind>,
    pub sort: SortKey,
    /// Per-cell rich render payload, e.g. the pod health-dot row. Empty means
    /// plain text — `render_table_cell` falls back to text/status rendering.
    pub extras: CellExtras,
}

/// Rich per-cell render payload. The health-dot row is the only shape so far;
/// future kinds (sparkline, progress bar) extend this enum.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum CellExtras {
    /// No extra rendering: plain text or a status pill.
    #[default]
    Plain,
    /// One dot per container: green when the container is ready.
    HealthDots(Vec<HealthDot>),
}

/// One container's readiness dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthDot {
    Ok,
    Err,
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
            extras: CellExtras::Plain,
        }
    }

    /// Status cell rendered as a pill; sorts by its label.
    pub fn status(label: &str, kind: StatusKind) -> Self {
        let text = label.to_string();
        let sort = SortKey::Text(text.clone());
        Self {
            text,
            sort,
            status: Some(kind),
            extras: CellExtras::Plain,
        }
    }

    /// Numeric cell; a non-finite key is stored finite, so the snapshot stays JSON.
    pub fn number(text: impl Into<String>, value: f64) -> Self {
        let value = if value.is_finite() { value } else { f64::MAX };
        Self {
            text: text.into(),
            sort: SortKey::Number(value),
            status: None,
            extras: CellExtras::Plain,
        }
    }

    /// Container health-dot row; text carries "ready/total", or an em dash
    /// when the pod has no container statuses to report.
    pub fn health_dots(dots: Vec<HealthDot>) -> Self {
        let ready = dots.iter().filter(|d| matches!(d, HealthDot::Ok)).count();
        let text = if dots.is_empty() {
            "—".to_string()
        } else {
            format!("{ready}/{}", dots.len())
        };
        Self {
            text,
            sort: SortKey::Number(ready as f64),
            status: None,
            extras: CellExtras::HealthDots(dots),
        }
    }
}

/// A concrete, display-ready table row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceRow {
    pub id: String,
    pub namespace: Option<String>,
    /// The resource kind this row represents, when the list knows it. Rows
    /// mixing kinds (a workloads view) can be filtered by it; rows that do not
    /// carry one never match a kind facet.
    #[serde(default)]
    pub kind: Option<String>,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnDef {
    pub key: String,
    pub label: String,
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

/// Reason the bulk bar shows while nothing is selected. Exported so hosts and
/// tests quote the same sentence instead of inventing a second one.
pub const BULK_EMPTY_REASON: &str = "Select at least one row to run a bulk action.";

/// Multi-row selection state for the resource table.
///
/// One model per table. Rows never carry their own checked flag, so the
/// selection has a single source of truth and survives virtualization: a row
/// scrolled out of the window keeps its pick.
///
/// Every read that drives the UI (`count_in` / `all_in` / `some_in` /
/// `selected_in`) is evaluated against the ids the table is showing *right
/// now*, so a pick made under one filter can never be silently counted — or
/// acted on — under another.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectionModel {
    selected: HashSet<String>,
    /// Row the last single-row toggle landed on; the origin of shift ranges.
    anchor: Option<String>,
}

impl SelectionModel {
    /// Empty selection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed a selection (SSR parity, restored views).
    pub fn from_ids(ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            selected: ids.into_iter().collect(),
            anchor: None,
        }
    }

    /// True when nothing at all is selected.
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Total picks, including rows the current filter hides.
    pub fn count(&self) -> usize {
        self.selected.len()
    }

    /// Whether `id` is selected.
    pub fn contains(&self, id: &str) -> bool {
        self.selected.contains(id)
    }

    /// All picks, sorted — deterministic for tests and bulk payloads.
    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.selected.iter().cloned().collect();
        ids.sort();
        ids
    }

    /// Flip one row and make it the anchor of a following shift range.
    pub fn toggle(&mut self, id: &str) {
        if !self.selected.remove(id) {
            self.selected.insert(id.to_string());
        }
        self.anchor = Some(id.to_string());
    }

    /// Shift-range: select everything between the anchor and `id` inside
    /// `order`. With no anchor, or rows outside the current view, this is a
    /// plain toggle. The anchor stays put so a second shift-click re-ranges.
    pub fn extend_to(&mut self, id: &str, order: &[String]) {
        let Some(anchor) = self.anchor.clone() else {
            return self.toggle(id);
        };
        let from = order.iter().position(|row| *row == anchor);
        let to = order.iter().position(|row| *row == id);
        let (Some(from), Some(to)) = (from, to) else {
            return self.toggle(id);
        };
        let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
        for row in &order[lo..=hi] {
            self.selected.insert(row.clone());
        }
    }

    /// Select every row of the current result set. Rows outside `order` are
    /// left alone, so "select all" never reaches past the active filter.
    pub fn select_all(&mut self, order: &[String]) {
        for id in order {
            self.selected.insert(id.clone());
        }
        self.anchor = order.last().cloned();
    }

    /// Deselect every row of the current result set.
    pub fn clear_all(&mut self, order: &[String]) {
        for id in order {
            self.selected.remove(id);
        }
        if self
            .anchor
            .as_ref()
            .is_some_and(|anchor| !self.selected.contains(anchor))
        {
            self.anchor = None;
        }
    }

    /// Drop the whole selection (Escape, filter change).
    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    /// Picks that are inside `order` — what the bar reports.
    pub fn count_in(&self, order: &[String]) -> usize {
        order
            .iter()
            .filter(|id| self.selected.contains(id.as_str()))
            .count()
    }

    /// Whether every row of `order` is checked (false for an empty view).
    pub fn all_in(&self, order: &[String]) -> bool {
        !order.is_empty() && order.iter().all(|id| self.selected.contains(id.as_str()))
    }

    /// Whether any row of `order` is checked.
    pub fn some_in(&self, order: &[String]) -> bool {
        order.iter().any(|id| self.selected.contains(id.as_str()))
    }

    /// Picks inside `order`, in view order — the payload of a bulk action.
    pub fn selected_in(&self, order: &[String]) -> Vec<String> {
        order
            .iter()
            .filter(|id| self.selected.contains(id.as_str()))
            .cloned()
            .collect()
    }
}

/// What a keydown means for the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKey {
    /// Check/uncheck the focused row.
    Toggle,
    /// Extend from the anchor to the focused row.
    Extend,
    /// Drop the whole selection.
    Clear,
    /// Not a selection key.
    Ignore,
}

/// Map a keydown to a selection intent.
///
/// Pure on purpose: the keyboard contract (space/x toggles, shift+either
/// extends, Escape clears) is testable without a DOM.
pub fn selection_key(key: &str, shift: bool) -> SelectionKey {
    match key.to_ascii_lowercase().as_str() {
        "escape" => SelectionKey::Clear,
        " " | "spacebar" | "x" => {
            if shift {
                SelectionKey::Extend
            } else {
                SelectionKey::Toggle
            }
        }
        _ => SelectionKey::Ignore,
    }
}

/// A bulk action offered by the host for the current selection.
#[derive(Clone, PartialEq)]
pub struct BulkAction {
    pub id: &'static str,
    pub label: &'static str,
    /// Renders with the danger treatment (destructive).
    pub danger: bool,
    /// `None` renders the button disabled: the host has nothing to run.
    pub on_run: Option<EventHandler<Vec<String>>>,
}

impl BulkAction {
    /// A destructive action wired to `on_run`.
    pub fn danger(
        id: &'static str,
        label: &'static str,
        on_run: Option<EventHandler<Vec<String>>>,
    ) -> Self {
        Self {
            id,
            label,
            danger: true,
            on_run,
        }
    }

    /// A non-destructive action wired to `on_run`.
    pub fn new(
        id: &'static str,
        label: &'static str,
        on_run: Option<EventHandler<Vec<String>>>,
    ) -> Self {
        Self {
            id,
            label,
            danger: false,
            on_run,
        }
    }
}

/// Heights the bulk bar occupies. The bar is always mounted at this height so
/// checking a row never reflows the table underneath it.
pub const BULK_BAR_HEIGHT: f64 = 52.0;

/// Live bulk-selection bar: how many rows are picked, what can be done with
/// them, and a way out.
///
/// Rendered even with nothing selected — the bar's space is reserved, and the
/// actions carry a disabled state plus the reason, instead of the bar popping
/// in and shoving the rows down.
#[component]
pub fn BulkSelectionBar(
    /// Rows selected inside the current view.
    count: usize,
    #[props(default)] actions: Vec<BulkAction>,
    /// Selected ids handed to an action.
    #[props(default)]
    selected: Vec<String>,
    /// Override for [`BULK_EMPTY_REASON`].
    #[props(default)]
    reason: Option<String>,
    #[props(default)] on_clear: Option<EventHandler<()>>,
) -> Element {
    let reason = reason.unwrap_or_else(|| BULK_EMPTY_REASON.to_string());
    let idle = count == 0;
    rsx! {
        div {
            class: if idle { "bulk-bar" } else { "bulk-bar active" },
            "data-active": if idle { "false" } else { "true" },
            role: "toolbar",
            "aria-label": "Bulk selection",
            span {
                class: "bulk-bar-count",
                "aria-live": "polite",
                "{count} selected"
            }
            if idle {
                span { class: "bulk-bar-reason", "{reason}" }
            }
            div { class: "bulk-bar-actions",
                button {
                    class: "btn btn-secondary",
                    r#type: "button",
                    disabled: idle,
                    title: if idle { reason.clone() } else { String::new() },
                    onclick: move |_| {
                        if let Some(handler) = on_clear {
                            handler.call(());
                        }
                    },
                    "Clear"
                }
                for action in actions {
                    {
                        let payload = selected.clone();
                        let unwired = action.on_run.is_none();
                        let blocked = idle || unwired;
                        let why = if idle {
                            reason.clone()
                        } else if unwired {
                            "No bulk handler is wired on this host.".to_string()
                        } else {
                            String::new()
                        };
                        rsx! {
                            button {
                                key: "{action.id}",
                                class: if action.danger { "btn btn-danger" } else { "btn btn-secondary" },
                                r#type: "button",
                                disabled: blocked,
                                title: "{why}",
                                onclick: move |_| {
                                    if let Some(handler) = action.on_run {
                                        handler.call(payload.clone());
                                    }
                                },
                                "{action.label}"
                            }
                        }
                    }
                }
            }
        }
    }
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
    /// The row id the detail pane is showing, so the list keeps a visible
    /// selection while the pane is open (the row-selection path sets it).
    #[props(default)]
    selected_row: Option<String>,
    #[props(default)] row_actions: Option<RowActions>,
    #[props(default)] on_row_click: Option<EventHandler<ResourceRow>>,
    #[props(default = 600.0)] height: f64,
    /// Bulk actions the host offers for the current selection. With none, the
    /// bar still renders (space reserved) and states why the actions are off.
    #[props(default)]
    bulk_actions: Vec<BulkAction>,
    /// Initial selection, for SSR parity and restored views.
    #[props(default)]
    selected_ids: Vec<String>,
) -> Element {
    let sort = use_signal(|| None::<(usize, SortDirection)>);
    let mut query = use_signal(String::new);
    // Health/kind facets for this table; the filter bar writes it, the row
    // pipeline below reads it.
    let facets = use_signal(FacetSelection::default);
    // Column visibility: seeded from the session store, updated by the picker.
    let prefs = use_signal(column_prefs);
    let mut selection = use_signal({
        let seed = selected_ids;
        move || SelectionModel::from_ids(seed)
    });

    // Every branch carries `data-state`, so the state a surface is in can be
    // read from the DOM rather than inferred from a screenshot (G4).
    match status {
        TableStatus::Loading => rsx! {
            div { class: "table-state", "data-state": "loading", "Loading…" }
        },
        TableStatus::Error(message) => rsx! {
            div { class: "table-state table-error", "data-state": "error", "{message}" }
        },
        TableStatus::Ready => {
            let selected: Vec<String> = crate::runtime::NAMESPACE_SELECTION.read().clone();
            // Namespace scoping is shared with the namespace strip; health/kind
            // facets are this table's own. One pipeline, in toolbar order:
            // scope → facets → text query → sort.
            let scoped: Vec<ResourceRow> = namespace_filter(&rows, &selected);
            let active: FacetSelection = facets.read().clone();
            let mut view: Vec<ResourceRow> = filter_rows(&scoped, &active)
                .into_iter()
                .filter(|row| matches_query(&row.search_text(), &query()))
                .collect();
            if let Some((column, direction)) = sort() {
                sort_by_key(&mut view, |row| row.cells[column].sort.clone(), direction);
            }
            // Column visibility is session state; `visible` holds indices into
            // `columns`, so a hidden column keeps its sort index and its width.
            let mut visible = prefs.read().visible_indices(&columns);
            if visible.is_empty() {
                visible = (0..columns.len()).collect();
            }
            let message = empty_message
                .clone()
                .unwrap_or_else(|| "No resources".to_string());
            // The panel paints the state the table is actually in: the surface
            // above reads the first `data-state` node, so a hardcoded "ready"
            // would hide an empty result behind a ready panel.
            let state = if view.is_empty() { "empty" } else { "ready" };

            // The selection is read against the rows on screen, never against
            // the raw snapshot: the bar can only count — and its actions can
            // only touch — rows the current filter still shows.
            let order: Rc<Vec<String>> = Rc::new(view.iter().map(|row| row.id.clone()).collect());
            let picked = selection.read().selected_in(&order);
            let all_checked = selection.read().all_in(&order);
            let order_for_head = order.clone();
            let mut selection_for_head = selection;

            rsx! {
                div {
                    class: "panel",
                    "data-state": state,
                    onkeydown: move |event: KeyboardEvent| {
                        if selection_key(&key_name(event.key()), event.modifiers().shift())
                            == SelectionKey::Clear
                        {
                            selection.write().clear();
                        }
                    },
                    div { class: "table-wrap",
                        div { class: "resource-table",
                            div { class: "table-toolbar",
                                div { class: "search-field",
                                    input {
                                        class: "table-filter",
                                        placeholder: "Filter…",
                                        value: "{query}",
                                        oninput: move |event| {
                                            // A filter change re-scopes what "selected"
                                            // means, so the picks go with it.
                                            selection.write().clear();
                                            query.set(event.value());
                                        },
                                    }
                                }
                                ColumnPicker { columns: columns.clone(), prefs }
                            }
                            FilterBar { rows: rows.clone(), scoped: scoped.clone(), selection: facets }
                            BulkSelectionBar {
                                count: picked.len(),
                                selected: picked.clone(),
                                actions: bulk_actions.clone(),
                                on_clear: move |_| {
                                    selection_for_head.write().clear();
                                },
                            }
                            if view.is_empty() {
                                div { class: "table-state table-empty", "data-state": "empty", "{message}" }
                            } else {
                                div { class: "table-header table-row",
                                    div { class: "table-cell table-select table-select-head",
                                        input {
                                            r#type: "checkbox",
                                            class: "row-check",
                                            "aria-label": "Select all rows in this view",
                                            checked: all_checked,
                                            onclick: move |event| {
                                                event.stop_propagation();
                                                let mut model = selection_for_head.write();
                                                if model.all_in(&order_for_head) {
                                                    model.clear_all(&order_for_head);
                                                } else {
                                                    model.select_all(&order_for_head);
                                                }
                                            },
                                        }
                                    }
                                    for i in visible.iter().copied() {
                                        { header_cell(i, &columns[i], sort) }
                                    }
                                }
                                TableBody {
                                    view,
                                    columns: columns.clone(),
                                    visible: visible.clone(),
                                    row_actions: row_actions.clone(),
                                    on_row_click,
                                    selected_row: selected_row.clone(),
                                    height,
                                    selection: selection_for_head,
                                    order: order.clone(),
                                }
                            }
                        }
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
    let key = column.key.clone();
    let label = column.label.clone();
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

/// Column geometry shared by every row in a body: widths and labels in table
/// order, plus the indices the column picker has left visible.
struct RowLayout<'a> {
    widths: &'a [Option<u32>],
    labels: &'a [String],
    visible: &'a [usize],
}

/// The scrollable, virtualized body of the table. Holds scroll state so that
/// scrolling re-renders only the visible window, never the full sorted list.
#[component]
fn TableBody(
    view: Vec<ResourceRow>,
    columns: Vec<ColumnDef>,
    /// Indices of the columns the picker has left visible, in table order.
    visible: Vec<usize>,
    row_actions: Option<RowActions>,
    on_row_click: Option<EventHandler<ResourceRow>>,
    selected_row: Option<String>,
    height: f64,
    /// The one selection model, shared by every row and the bulk bar.
    selection: Signal<SelectionModel>,
    /// Ids of the rows in the current view, in view order.
    order: Rc<Vec<String>>,
) -> Element {
    let mut scroll_top = use_signal(|| 0.0f64);

    let total = view.len();
    let range = visible_range(scroll_top(), height, total);
    let start = range.start;
    let slice = &view[range];
    let total_height = total as f64 * ROW_HEIGHT;
    let widths: Vec<Option<u32>> = columns.iter().map(|column| column.width).collect();
    let labels: Vec<String> = columns.iter().map(|column| column.label.clone()).collect();
    let layout = RowLayout {
        widths: &widths,
        labels: &labels,
        visible: &visible,
    };

    rsx! {
        div {
            class: "table-body",
            style: "height: {height}px; overflow-y: auto; position: relative;",
            onscroll: move |event| scroll_top.set(event.scroll_top()),
            div { style: "height: {total_height}px; position: relative;",
                for (offset, row) in slice.iter().enumerate() {
                    {
                        render_table_row(
                            row,
                            (start + offset) as f64 * ROW_HEIGHT,
                            &layout,
                            row_actions.clone(),
                            on_row_click,
                            selected_row.clone(),
                            selection,
                            order.clone(),
                        )
                    }
                }
            }
        }
    }
}

/// Keys a row answers to, as the strings [`selection_key`] understands.
fn key_name(key: Key) -> String {
    match key {
        Key::Character(text) => text.to_string(),
        Key::Escape => "escape".to_string(),
        _ => String::new(),
    }
}

/// Render a single virtualized table row (absolute-positioned).
#[allow(clippy::too_many_arguments)]
fn render_table_row(
    row: &ResourceRow,
    top: f64,
    layout: &RowLayout<'_>,
    row_actions: Option<RowActions>,
    on_row_click: Option<EventHandler<ResourceRow>>,
    selected_row: Option<String>,
    mut selection: Signal<SelectionModel>,
    order: Rc<Vec<String>>,
) -> Element {
    let row_id = row.id.clone();
    let row_for_click = row.clone();
    let handler = on_row_click;
    // The list keeps its selection: the row the pane is showing stays marked
    // while the pane is open, and the table itself is never remounted.
    let is_selected = selected_row.as_deref() == Some(row.id.as_str());
    let checked = selection.read().contains(&row_id);
    let class = match (is_selected, checked) {
        (true, true) => "table-row selected checked",
        (true, false) => "table-row selected",
        (false, true) => "table-row checked",
        (false, false) => "table-row",
    };
    let id_for_key = row_id.clone();
    let id_for_check = row_id.clone();
    let order_for_key = order;
    rsx! {
        div {
            key: "{row_id}",
            class: "{class}",
            "aria-selected": "{is_selected}",
            "aria-checked": "{checked}",
            "data-row-id": "{row_id}",
            "data-checked": "{checked}",
            role: "row",
            tabindex: "0",
            style: "position: absolute; top: {top}px; height: {ROW_HEIGHT}px; left: 0; right: 0;",
            onclick: move |_| {
                if let Some(h) = handler {
                    h.call(row_for_click.clone());
                }
            },
            onkeydown: move |event: KeyboardEvent| {
                let intent = selection_key(&key_name(event.key()), event.modifiers().shift());
                if intent == SelectionKey::Ignore {
                    return;
                }
                event.prevent_default();
                match intent {
                    SelectionKey::Clear => selection.write().clear(),
                    SelectionKey::Toggle => selection.write().toggle(&id_for_key),
                    SelectionKey::Extend => {
                        selection.write().extend_to(&id_for_key, &order_for_key)
                    }
                    SelectionKey::Ignore => {}
                }
            },
            div { class: "table-cell table-select",
                input {
                    r#type: "checkbox",
                    class: "row-check",
                    "aria-label": "Select {row_id}",
                    checked: checked,
                    onclick: move |event| {
                        // Ticking a box must not also open the detail pane.
                        event.stop_propagation();
                        selection.write().toggle(&id_for_check);
                    },
                }
            }
            for i in layout.visible.iter().copied() {
                {
                    render_table_cell(
                        &row.cells[i],
                        i,
                        layout.widths.get(i).copied().flatten(),
                        layout.labels.get(i).map(String::as_str),
                    )
                }
            }
            if let Some(actions) = row_actions {
                { render_row_actions(&row_id, &actions) }
            }
        }
    }
}

/// Render a single table cell (plain text, status pill, or rich extra).
fn render_table_cell(
    cell: &Cell,
    index: usize,
    width: Option<u32>,
    label: Option<&str>,
) -> Element {
    let style = width.map(|w| format!("width: {w}px")).unwrap_or_default();
    let label = label.unwrap_or_default();
    let dot_classes: Vec<&'static str> = match &cell.extras {
        CellExtras::HealthDots(dots) => dots
            .iter()
            .map(|dot| match dot {
                HealthDot::Ok => "dot ok",
                HealthDot::Err => "dot err",
            })
            .collect(),
        CellExtras::Plain => Vec::new(),
    };
    match &cell.extras {
        CellExtras::HealthDots(_) => rsx! {
            div {
                key: "{index}",
                class: "table-cell health-dots",
                "data-label": "{label}",
                style: "{style}",
                if dot_classes.is_empty() {
                    span { "—" }
                } else {
                    for class in dot_classes {
                        span { class: "{class}" }
                    }
                }
            }
        },
        CellExtras::Plain => match cell.status {
            Some(kind) => rsx! {
                div {
                    key: "{index}",
                    class: "table-cell",
                    "data-label": "{label}",
                    style: "{style}",
                    StatusPill { status: kind }
                }
            },
            None => rsx! {
                div {
                    key: "{index}",
                    class: "table-cell",
                    "data-label": "{label}",
                    style: "{style}",
                    span { "{cell.text}" }
                }
            },
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

/// Inline readiness indicator: a row of small dots (`.health-dots` /
/// `.dot.ok` / `.dot.err`) for the per-instance status column. The first
/// `ready` dots are green; the rest are red.
#[component]
pub fn HealthDots(ready: u32, total: u32) -> Element {
    let dots: Vec<&'static str> = (0..total)
        .map(|i| if i < ready { "ok" } else { "err" })
        .collect();
    rsx! {
        div { class: "health-dots",
            for cls in dots {
                span { class: "dot {cls}" }
            }
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
            kind: None,
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
