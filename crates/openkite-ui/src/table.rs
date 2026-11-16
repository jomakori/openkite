//! Pure model for the resource table: columns, rows, sorting, filtering, the virtual
//! window, selection and the row-interaction contract.
//!
//! Rendering lives in [`crate::table_view`]; everything here is testable without a DOM,
//! which is what lets the contract be checked across every catalogue kind.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

/// One table column. `id` matches the row cell key and the sort key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    pub id: &'static str,
    pub label: &'static str,
}

/// A table row: identity plus the cell values keyed by column id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub name: String,
    pub namespace: Option<String>,
    pub cells: BTreeMap<String, String>,
    /// Child rows shown when the row is expanded (Deployment -> ReplicaSets, ...).
    pub kids: Vec<Row>,
}

impl Row {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Row {
            id: id.into(),
            name: name.into(),
            namespace: None,
            cells: BTreeMap::new(),
            kids: Vec::new(),
        }
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    pub fn with_cell(mut self, id: impl Into<String>, value: impl Into<String>) -> Self {
        self.cells.insert(id.into(), value.into());
        self
    }

    pub fn with_kids(mut self, kids: Vec<Row>) -> Self {
        self.kids = kids;
        self
    }

    /// Cell text for a column, falling back to the row's own identity fields.
    pub fn cell(&self, column: &str) -> &str {
        match column {
            "name" => &self.name,
            "ns" | "namespace" => self.namespace.as_deref().unwrap_or_default(),
            other => self
                .cells
                .get(other)
                .map(String::as_str)
                .unwrap_or_default(),
        }
    }
}

pub const DEFAULT_COLUMNS: &[Column] = &[
    Column {
        id: "name",
        label: "Name",
    },
    Column {
        id: "status",
        label: "Status",
    },
    Column {
        id: "age",
        label: "Age",
    },
];

/// Columns for a catalogue kind. Kind keys are the catalogue's, not the prototype's
/// internal aliases; the generated table is keyed the same way.
pub fn columns(kind: &str) -> &'static [Column] {
    crate::table_columns::KIND_COLUMNS
        .iter()
        .find(|(name, _)| *name == kind)
        .map(|(_, cols)| *cols)
        .unwrap_or(DEFAULT_COLUMNS)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub fn toggled(self) -> Self {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }

    pub fn aria(self) -> &'static str {
        match self {
            SortDir::Asc => "ascending",
            SortDir::Desc => "descending",
        }
    }

    pub fn mark(self) -> &'static str {
        match self {
            SortDir::Asc => "↑",
            SortDir::Desc => "↓",
        }
    }
}

/// Sort column plus direction. The prototype defaults to name ascending.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sort {
    pub column: String,
    pub dir: SortDir,
}

impl Default for Sort {
    fn default() -> Self {
        Sort {
            column: "name".to_string(),
            dir: SortDir::Asc,
        }
    }
}

impl Sort {
    pub fn new(column: impl Into<String>, dir: SortDir) -> Self {
        Sort {
            column: column.into(),
            dir,
        }
    }

    /// Clicking the active column flips direction; a new column starts ascending.
    pub fn clicked(&self, column: &str) -> Self {
        if self.column == column {
            Sort {
                column: self.column.clone(),
                dir: self.dir.toggled(),
            }
        } else {
            Sort {
                column: column.to_string(),
                dir: SortDir::Asc,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
    Comfortable,
    Compact,
}

impl Density {
    pub fn toggle(self) -> Self {
        match self {
            Density::Comfortable => Density::Compact,
            Density::Compact => Density::Comfortable,
        }
    }

    /// Row height in CSS pixels, matching the prototype's density classes.
    pub fn row_height(self) -> f64 {
        match self {
            Density::Comfortable => 52.0,
            Density::Compact => 36.0,
        }
    }

    pub fn class(self) -> &'static str {
        match self {
            Density::Comfortable => "comfortable",
            Density::Compact => "compact",
        }
    }
}

/// Table state. One state per rendered table (the shell keeps one per active kind).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableState {
    pub query: String,
    pub namespace: Option<String>,
    pub sort: Sort,
    pub density: Density,
    pub page_size: usize,
    pub page: usize,
    pub selected: BTreeSet<String>,
    pub hidden: BTreeSet<String>,
    pub expanded: BTreeSet<String>,
}

impl Default for TableState {
    fn default() -> Self {
        TableState {
            query: String::new(),
            namespace: None,
            sort: Sort::default(),
            density: Density::Comfortable,
            page_size: 100,
            page: 0,
            selected: BTreeSet::new(),
            hidden: BTreeSet::new(),
            expanded: BTreeSet::new(),
        }
    }
}

impl TableState {
    /// Columns actually rendered: the first column is always shown, as in the prototype.
    pub fn visible_columns(&self, kind: &str) -> Vec<Column> {
        columns(kind)
            .iter()
            .enumerate()
            .filter(|(index, col)| *index == 0 || !self.hidden.contains(col.id))
            .map(|(_, col)| *col)
            .collect()
    }

    pub fn toggle_hidden(&mut self, column: &str) {
        if column == "name" {
            return;
        }
        if !self.hidden.remove(column) {
            self.hidden.insert(column.to_string());
        }
    }

    pub fn toggle_expanded(&mut self, row_id: &str) {
        if !self.expanded.remove(row_id) {
            self.expanded.insert(row_id.to_string());
        }
    }

    /// Expand every row that has children.
    pub fn expand_all(&mut self, rows: &[Row]) {
        self.expanded = rows
            .iter()
            .filter(|row| !row.kids.is_empty())
            .map(|row| row.id.clone())
            .collect();
    }

    pub fn toggle_selected(&mut self, row_id: &str) {
        if !self.selected.remove(row_id) {
            self.selected.insert(row_id.to_string());
        }
    }

    pub fn select_all(&mut self, rows: &[&Row]) {
        for row in rows {
            self.selected.insert(row.id.clone());
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    pub fn all_selected(&self, rows: &[&Row]) -> bool {
        !rows.is_empty() && rows.iter().all(|row| self.selected.contains(&row.id))
    }

    pub fn any_selected(&self, rows: &[&Row]) -> bool {
        rows.iter().any(|row| self.selected.contains(&row.id))
    }

    pub fn indeterminate(&self, rows: &[&Row]) -> bool {
        self.any_selected(rows) && !self.all_selected(rows)
    }

    /// Rows matching the query and namespace chip, sorted by [`Self::sort`].
    pub fn visible<'a>(&self, rows: &'a [Row]) -> Vec<&'a Row> {
        let query = self.query.trim().to_lowercase();
        let mut out: Vec<&Row> = rows
            .iter()
            .filter(|row| {
                self.namespace
                    .as_ref()
                    .is_none_or(|ns| row.namespace.as_deref() == Some(ns.as_str()))
            })
            .filter(|row| query.is_empty() || row_matches(row, &query))
            .collect();
        sort_rows(&mut out, &self.sort);
        out
    }

    /// Page slice for the pager, clamped to the available rows.
    pub fn page_slice(&self, total: usize) -> Range<usize> {
        let size = self.page_size.max(1);
        let pages = total.div_ceil(size).max(1);
        let page = self.page.min(pages - 1);
        let start = page * size;
        start..(start + size).min(total)
    }

    pub fn page_count(&self, total: usize) -> usize {
        total.div_ceil(self.page_size.max(1)).max(1)
    }
}

/// Case-insensitive match across the row's identity fields and every cell value.
pub fn row_matches(row: &Row, query_lower: &str) -> bool {
    if row.name.to_lowercase().contains(query_lower) {
        return true;
    }
    if row
        .namespace
        .as_deref()
        .is_some_and(|ns| ns.to_lowercase().contains(query_lower))
    {
        return true;
    }
    row.cells
        .values()
        .any(|value| value.to_lowercase().contains(query_lower))
}

/// Sorts rows in place. Numeric cells compare numerically (so 9 sorts before 10 and
/// `3d` before `12d`), everything else compares case-insensitively as text.
pub fn sort_rows(rows: &mut [&Row], sort: &Sort) {
    let column = sort.column.as_str();
    rows.sort_by(|a, b| {
        let left = a.cell(column);
        let right = b.cell(column);
        let ordering = match (numeric_value(left), numeric_value(right)) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
            _ => left.to_lowercase().cmp(&right.to_lowercase()),
        };
        match sort.dir {
            SortDir::Asc => ordering,
            SortDir::Desc => ordering.reverse(),
        }
    });
}

/// Numeric interpretation of a cell: plain numbers, and Kubernetes age/duration text.
pub fn numeric_value(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = trimmed.parse::<f64>() {
        return Some(value);
    }
    duration_seconds(trimmed)
}

/// Parses compact durations (`45s`, `12m`, `3h`, `9d`, `2y`) into seconds.
pub fn duration_seconds(text: &str) -> Option<f64> {
    let (number, unit) = text.split_at(text.find(|c: char| !c.is_ascii_digit())?);
    if number.is_empty() {
        return None;
    }
    let value: f64 = number.parse().ok()?;
    let scale = match unit.trim() {
        "s" => 1.0,
        "m" => 60.0,
        "h" => 3_600.0,
        "d" => 86_400.0,
        "w" => 604_800.0,
        "y" => 31_536_000.0,
        _ => return None,
    };
    Some(value * scale)
}

/// Fractional rows rendered around the viewport so scrolling never shows a blank band.
const OVERSCAN: usize = 8;

/// Virtual window over `total` rows for a scroll offset, in rows.
pub fn virtual_window(
    total: usize,
    scroll_px: f64,
    viewport_px: f64,
    row_height: f64,
) -> Range<usize> {
    if total == 0 || row_height <= 0.0 {
        return 0..0;
    }
    let first = ((scroll_px / row_height).floor() as usize).saturating_sub(OVERSCAN);
    let visible = (viewport_px / row_height).ceil() as usize + 1;
    let end = (first + visible + OVERSCAN * 2).min(total);
    first.min(total)..end
}

/// What part of a row a click landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowPart {
    Body,
    Checkbox,
    NamespaceChip,
    ActionButton,
    DrillLink,
    OrbCount,
    TreeToggle,
}

/// What the click should do. Nested controls act on themselves only — the row must not
/// also open the inspector, which is the classic table interaction bug.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowEffect {
    OpenInspector,
    ToggleSelection,
    OpenRowMenu,
    Ignore,
}

/// Status pill flavour for a Kubernetes status string, matching the prototype's
/// `pill success|warn|err` classes.
pub fn pill_class(status: &str) -> &'static str {
    match status {
        "Running" | "Completed" | "Succeeded" | "Active" | "Bound" | "Ready" | "Healthy" => {
            "success"
        }
        "Pending" | "ContainerCreating" | "Terminating" | "Progressing" | "Suspended" => "warn",
        "Failed" | "Error" | "CrashLoopBackOff" | "Evicted" | "Unknown" | "Unhealthy" => "err",
        _ => "neutral",
    }
}

/// One dot per container in a `ready/total` value; `true` means ready, which mirrors the
/// prototype's `health-dots` rendering.
pub fn health_dots(ready: &str) -> Vec<bool> {
    let (up, total) = match ready.split_once('/') {
        Some((up, total)) => (
            up.trim().parse::<usize>().unwrap_or(0),
            total.trim().parse::<usize>().unwrap_or(0),
        ),
        None => {
            let single = ready.trim().parse::<usize>().unwrap_or(0);
            (single, single)
        }
    };
    let total = if total == 0 { up } else { total };
    (0..total).map(|index| index < up).collect()
}

/// Keyboard activation: Enter or Space on a focused row opens the inspector, exactly
/// like a click on the row body.
pub fn key_opens_row(key: &str) -> bool {
    matches!(key, "Enter" | " " | "Space" | "Spacebar")
}

pub fn effect_for(part: RowPart) -> RowEffect {
    match part {
        RowPart::Body => RowEffect::OpenInspector,
        RowPart::Checkbox => RowEffect::ToggleSelection,
        RowPart::ActionButton => RowEffect::OpenRowMenu,
        RowPart::NamespaceChip | RowPart::DrillLink | RowPart::OrbCount | RowPart::TreeToggle => {
            RowEffect::Ignore
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rows(count: usize) -> Vec<Row> {
        (0..count)
            .map(|i| {
                Row::new(format!("id-{i}"), format!("pod-{i}"))
                    .with_namespace(if i % 2 == 0 { "default" } else { "kube-system" })
                    .with_cell("status", if i % 3 == 0 { "Running" } else { "Pending" })
                    .with_cell("restarts", i.to_string())
                    .with_cell("age", format!("{}d", i % 30 + 1))
            })
            .collect()
    }

    #[test]
    fn every_catalogue_kind_has_columns_starting_with_name() {
        for kind in crate::nav::CATALOGUE.iter().map(|kind| kind.key) {
            let cols = columns(kind);
            assert!(!cols.is_empty(), "{kind} has no columns");
            assert!(
                &["name", "object"].contains(&cols[0].id),
                "{kind} must start with an identity column, got {}",
                cols[0].id
            );
        }
    }

    #[test]
    fn name_column_cannot_be_hidden() {
        let mut state = TableState::default();
        state.toggle_hidden("name");
        state.toggle_hidden("restarts");
        let visible = state.visible_columns("pods");
        assert_eq!(visible[0].id, "name");
        assert!(!visible.iter().any(|col| col.id == "restarts"));
    }

    #[test]
    fn filter_is_case_insensitive_and_covers_cells() {
        let rows = sample_rows(6);
        let mut state = TableState {
            query: "RUNNING".to_string(),
            ..TableState::default()
        };
        let visible = state.visible(&rows);
        assert_eq!(visible.len(), 2);
        state.query = "kube-system".to_string();
        assert_eq!(state.visible(&rows).len(), 3);
        let _ = rows;
    }

    #[test]
    fn namespace_chip_narrows_rows() {
        let rows = sample_rows(6);
        let mut state = TableState {
            namespace: Some("default".to_string()),
            ..TableState::default()
        };
        assert_eq!(state.visible(&rows).len(), 3);
        state.namespace = None;
        assert_eq!(state.visible(&rows).len(), 6);
    }

    #[test]
    fn numeric_columns_sort_numerically_both_ways() {
        let rows = sample_rows(12);
        let mut state = TableState {
            sort: Sort::new("restarts", SortDir::Asc),
            ..TableState::default()
        };
        let ascending: Vec<&str> = state
            .visible(&rows)
            .iter()
            .map(|row| row.name.as_str())
            .collect();
        assert_eq!(ascending.first(), Some(&"pod-0"));
        state.sort = Sort::new("restarts", SortDir::Desc);
        let descending: Vec<&str> = state
            .visible(&rows)
            .iter()
            .map(|row| row.name.as_str())
            .collect();
        assert_eq!(descending.first(), Some(&"pod-11"));
    }

    #[test]
    fn age_sorts_by_duration_not_lexically() {
        assert!(duration_seconds("9d") < duration_seconds("12d"));
        let rows = vec![
            Row::new("a", "a").with_cell("age", "12d"),
            Row::new("b", "b").with_cell("age", "9d"),
            Row::new("c", "c").with_cell("age", "2h"),
        ];
        let state = TableState {
            sort: Sort::new("age", SortDir::Asc),
            ..TableState::default()
        };
        let names: Vec<&str> = state
            .visible(&rows)
            .iter()
            .map(|row| row.name.as_str())
            .collect();
        assert_eq!(names, vec!["c", "b", "a"]);
    }

    #[test]
    fn clicking_a_sort_header_toggles_direction() {
        let sort = Sort::default();
        let clicked = sort.clicked("name");
        assert_eq!(clicked.dir, SortDir::Desc);
        let other = clicked.clicked("status");
        assert_eq!(other, Sort::new("status", SortDir::Asc));
    }

    #[test]
    fn selection_reports_all_any_and_indeterminate() {
        let rows = sample_rows(4);
        let refs: Vec<&Row> = rows.iter().collect();
        let mut state = TableState::default();
        assert!(!state.all_selected(&refs));
        assert!(!state.indeterminate(&refs));
        state.toggle_selected("id-0");
        assert!(state.indeterminate(&refs));
        state.select_all(&refs);
        assert!(state.all_selected(&refs));
        assert!(!state.indeterminate(&refs));
        state.clear_selection();
        assert!(!state.any_selected(&refs));
    }

    #[test]
    fn pager_clamps_and_slices() {
        let mut state = TableState {
            page_size: 10,
            ..TableState::default()
        };
        assert_eq!(state.page_count(95), 10);
        assert_eq!(state.page_slice(95), 0..10);
        state.page = 9;
        assert_eq!(state.page_slice(95), 90..95);
        state.page = 99;
        assert_eq!(state.page_slice(95), 90..95);
    }

    #[test]
    fn virtual_window_covers_viewport_plus_overscan_and_clamps() {
        let window = virtual_window(1_000, 5_200.0, 600.0, 52.0);
        assert!(window.start <= 100);
        assert!(window.end > window.start + 11);
        assert_eq!(virtual_window(0, 0.0, 600.0, 52.0), 0..0);
        let end = virtual_window(20, 0.0, 600.0, 52.0);
        assert_eq!(end.end, 20);
    }

    #[test]
    fn row_click_contract_separates_nested_controls() {
        assert_eq!(effect_for(RowPart::Body), RowEffect::OpenInspector);
        assert_eq!(effect_for(RowPart::Checkbox), RowEffect::ToggleSelection);
        assert_eq!(effect_for(RowPart::ActionButton), RowEffect::OpenRowMenu);
        assert_eq!(effect_for(RowPart::NamespaceChip), RowEffect::Ignore);
        assert_eq!(effect_for(RowPart::DrillLink), RowEffect::Ignore);
        assert_eq!(effect_for(RowPart::OrbCount), RowEffect::Ignore);
        assert_eq!(effect_for(RowPart::TreeToggle), RowEffect::Ignore);
        assert_eq!(pill_class("Running"), "success");
        assert_eq!(pill_class("Pending"), "warn");
        assert_eq!(pill_class("Failed"), "err");
        assert_eq!(health_dots("2/3"), vec![true, true, false]);
        assert_eq!(health_dots("1/1"), vec![true]);
        assert!(key_opens_row("Enter"));
        assert!(key_opens_row(" "));
        assert!(!key_opens_row("Tab"));
    }

    #[test]
    fn twenty_thousand_rows_filter_and_sort_quickly() {
        let rows = sample_rows(20_000);
        let state = TableState {
            query: "kube-system".to_string(),
            sort: Sort::new("restarts", SortDir::Desc),
            ..TableState::default()
        };
        let started = std::time::Instant::now();
        let visible = state.visible(&rows);
        let elapsed = started.elapsed();
        assert_eq!(visible.len(), 10_000);
        assert_eq!(visible[0].name, "pod-19999");
        // Debug builds are ~10x slower than release; the release figure is what the
        // acceptance criterion measures.
        assert!(
            elapsed.as_millis() < 500,
            "filter+sort of 20k rows took {elapsed:?}, expected well under the debug budget"
        );
    }
}
