//! Filter bar for the resources list: faceted narrowing by namespace, health
//! and kind, plus clear-all.
//!
//! The bar does not invent a second filter path. Namespace facets write
//! [`crate::runtime::NAMESPACE_SELECTION`] — the store the namespace strip and
//! the table's own row scoping already read — and the health/kind facets write
//! the [`FacetSelection`] signal [`crate::components::resource_table::ResourceTable`]
//! owns and applies.

use dioxus::prelude::*;

use crate::components::resource_table::{CellExtras, HealthDot, ResourceRow};
use crate::components::status_badge::StatusKind;
use crate::runtime;

/// Coarse health bucket, derived from the status pills and health dots a row
/// already carries. Facet-sized: a list has three useful buckets, not ten
/// statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Health {
    Healthy,
    Warning,
    Error,
    Unknown,
}

impl Health {
    /// The bucket a status pill falls into (via its palette class, so a new
    /// status kind lands in the right bucket without touching this match).
    pub fn from_status(status: StatusKind) -> Self {
        match status.class() {
            "status-ok" => Health::Healthy,
            "status-warn" => Health::Warning,
            "status-err" => Health::Error,
            _ => Health::Unknown,
        }
    }

    /// Chip label.
    pub fn label(self) -> &'static str {
        match self {
            Health::Healthy => "Healthy",
            Health::Warning => "Warning",
            Health::Error => "Error",
            Health::Unknown => "Unknown",
        }
    }

    /// Stable value for `data-value` and order of severity.
    fn severity(self) -> u8 {
        match self {
            Health::Healthy => 0,
            Health::Warning => 1,
            Health::Unknown => 2,
            Health::Error => 3,
        }
    }
}

fn worse(left: Health, right: Health) -> Health {
    if left.severity() >= right.severity() {
        left
    } else {
        right
    }
}

/// A row's health: the worst signal among its status pills and health dots.
/// `None` when the row carries no health signal at all, which keeps such a row
/// out of every health facet.
pub fn row_health(row: &ResourceRow) -> Option<Health> {
    let mut worst: Option<Health> = None;
    for cell in &row.cells {
        let mut from_cell: Option<Health> = cell.status.map(Health::from_status);
        if let CellExtras::HealthDots(dots) = &cell.extras {
            if !dots.is_empty() {
                let dots_health = if dots.iter().all(|dot| *dot == HealthDot::Ok) {
                    Health::Healthy
                } else {
                    Health::Error
                };
                from_cell = Some(match from_cell {
                    Some(health) => worse(health, dots_health),
                    None => dots_health,
                });
            }
        }
        if let Some(health) = from_cell {
            worst = Some(match worst {
                Some(current) => worse(current, health),
                None => health,
            });
        }
    }
    worst
}

/// Active health/kind facets. Namespace is not here: it lives in
/// [`crate::runtime::NAMESPACE_SELECTION`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FacetSelection {
    pub health: Vec<Health>,
    pub kinds: Vec<String>,
}

impl FacetSelection {
    /// No facet is narrowing anything.
    pub fn is_empty(&self) -> bool {
        self.health.is_empty() && self.kinds.is_empty()
    }

    /// Drop every facet.
    pub fn clear(&mut self) {
        self.health.clear();
        self.kinds.clear();
    }

    /// Add `health` to the selection, or remove it.
    pub fn toggle_health(&mut self, health: Health) {
        if let Some(index) = self.health.iter().position(|value| *value == health) {
            self.health.remove(index);
        } else {
            self.health.push(health);
        }
    }

    /// Add `kind` to the selection, or remove it.
    pub fn toggle_kind(&mut self, kind: &str) {
        if let Some(index) = self.kinds.iter().position(|value| value == kind) {
            self.kinds.remove(index);
        } else {
            self.kinds.push(kind.to_string());
        }
    }

    /// Would `row` survive the selection? Values inside one facet are OR-ed,
    /// facets are AND-ed — the usual list-filter reading.
    pub fn matches(&self, row: &ResourceRow) -> bool {
        let health_ok = self.health.is_empty()
            || row_health(row).is_some_and(|health| self.health.contains(&health));
        let kind_ok = self.kinds.is_empty()
            || row
                .kind
                .as_deref()
                .is_some_and(|kind| self.kinds.iter().any(|selected| selected == kind));
        health_ok && kind_ok
    }
}

/// The rows `selection` keeps.
pub fn filter_rows(rows: &[ResourceRow], selection: &FacetSelection) -> Vec<ResourceRow> {
    rows.iter()
        .filter(|row| selection.matches(row))
        .cloned()
        .collect()
}

/// A facet option and how many rows sit behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetCount<T> {
    pub value: T,
    pub count: usize,
}

/// The options each facet offers for a row set, counted. Facets with no rows
/// offer nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FacetCounts {
    pub namespaces: Vec<FacetCount<String>>,
    pub health: Vec<FacetCount<Health>>,
    pub kinds: Vec<FacetCount<String>>,
}

impl FacetCounts {
    /// Count the facets of `rows`. Health and kind counts include every row, so
    /// toggling one facet never hides the chips needed to undo it.
    pub fn from_rows(rows: &[ResourceRow]) -> Self {
        let mut namespaces: Vec<FacetCount<String>> = Vec::new();
        let mut health: Vec<FacetCount<Health>> = Vec::new();
        let mut kinds: Vec<FacetCount<String>> = Vec::new();
        for row in rows {
            if let Some(namespace) = &row.namespace {
                bump(&mut namespaces, namespace.clone());
            }
            if let Some(bucket) = row_health(row) {
                bump(&mut health, bucket);
            }
            if let Some(kind) = &row.kind {
                bump(&mut kinds, kind.clone());
            }
        }
        namespaces.sort_by(|left, right| left.value.cmp(&right.value));
        health.sort_by_key(|option| option.value.severity());
        kinds.sort_by(|left, right| left.value.cmp(&right.value));
        Self {
            namespaces,
            health,
            kinds,
        }
    }
}

fn bump<T: PartialEq>(options: &mut Vec<FacetCount<T>>, value: T) {
    match options.iter_mut().find(|option| option.value == value) {
        Some(option) => option.count += 1,
        None => options.push(FacetCount { value, count: 1 }),
    }
}

fn chip_class(active: bool) -> &'static str {
    if active {
        "chip active"
    } else {
        "chip"
    }
}

/// The filter bar. `rows` is the whole list (namespace facets and the
/// clear-all state), `scoped` is the list already narrowed by the selected
/// namespaces (health and kind facets count inside that scope).
#[component]
pub fn FilterBar(
    rows: Vec<ResourceRow>,
    scoped: Vec<ResourceRow>,
    selection: Signal<FacetSelection>,
) -> Element {
    let all_counts = FacetCounts::from_rows(&rows);
    let scoped_counts = FacetCounts::from_rows(&scoped);
    let selected: Vec<String> = runtime::selected_namespaces();
    let active: FacetSelection = selection.read().clone();
    let any_active = !active.is_empty() || !selected.is_empty();
    let namespace_all = selected.is_empty();

    rsx! {
        div { class: "list-filters", role: "group", "aria-label": "List filters",
            div { class: "facet",
                span { class: "facet-label", "namespace" }
                div { class: "chip-row",
                    button {
                        class: chip_class(namespace_all),
                        r#type: "button",
                        "data-facet": "namespace",
                        "data-value": "all",
                        "aria-pressed": if namespace_all { "true" } else { "false" },
                        onclick: move |_| runtime::clear_namespace_selection(),
                        "All"
                    }
                    for option in all_counts.namespaces {
                        {
                            let name = option.value.clone();
                            let active = selected.contains(&name);
                            let for_click = name.clone();
                            rsx! {
                                button {
                                    key: "ns-{name}",
                                    class: chip_class(active),
                                    r#type: "button",
                                    "data-facet": "namespace",
                                    "data-value": "{name}",
                                    "aria-pressed": if active { "true" } else { "false" },
                                    onclick: move |_| runtime::toggle_namespace(for_click.clone()),
                                    "{name}"
                                    span { class: "facet-count", "{option.count}" }
                                }
                            }
                        }
                    }
                }
            }
            if !scoped_counts.health.is_empty() {
                div { class: "facet",
                    span { class: "facet-label", "health" }
                    div { class: "chip-row",
                        for option in scoped_counts.health {
                            {
                                let active = active.health.contains(&option.value);
                                rsx! {
                                    button {
                                        key: "health-{option.value.label()}",
                                        class: chip_class(active),
                                        r#type: "button",
                                        "data-facet": "health",
                                        "data-value": "{option.value.label()}",
                                        "aria-pressed": if active { "true" } else { "false" },
                                        onclick: move |_| selection.write().toggle_health(option.value),
                                        "{option.value.label()}"
                                        span { class: "facet-count", "{option.count}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !scoped_counts.kinds.is_empty() {
                div { class: "facet",
                    span { class: "facet-label", "kind" }
                    div { class: "chip-row",
                        for option in scoped_counts.kinds {
                            {
                                let active = active.kinds.contains(&option.value);
                                let for_click = option.value.clone();
                                rsx! {
                                    button {
                                        key: "kind-{option.value}",
                                        class: chip_class(active),
                                        r#type: "button",
                                        "data-facet": "kind",
                                        "data-value": "{option.value}",
                                        "aria-pressed": if active { "true" } else { "false" },
                                        onclick: move |_| selection.write().toggle_kind(&for_click),
                                        "{option.value}"
                                        span { class: "facet-count", "{option.count}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            button {
                class: "btn btn-secondary facet-clear",
                r#type: "button",
                disabled: !any_active,
                onclick: move |_| {
                    runtime::clear_namespace_selection();
                    *selection.write() = FacetSelection::default();
                },
                "Clear all"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::resource_table::{Cell, SortKey};

    fn row(
        name: &str,
        namespace: &str,
        kind: Option<&str>,
        status: Option<StatusKind>,
    ) -> ResourceRow {
        ResourceRow {
            id: format!("{namespace}/{name}"),
            namespace: Some(namespace.to_string()),
            kind: kind.map(str::to_string),
            cells: vec![Cell {
                text: name.to_string(),
                status,
                sort: SortKey::Text(name.to_string()),
                extras: CellExtras::Plain,
            }],
        }
    }

    fn workloads() -> Vec<ResourceRow> {
        vec![
            row("web-1", "default", Some("Pod"), Some(StatusKind::Running)),
            row("web-2", "default", Some("Pod"), Some(StatusKind::CrashLoop)),
            row(
                "db-0",
                "data",
                Some("StatefulSet"),
                Some(StatusKind::Pending),
            ),
            row("job-1", "data", Some("Job"), Some(StatusKind::Succeeded)),
        ]
    }

    fn names(rows: &[ResourceRow]) -> Vec<String> {
        rows.iter().map(|row| row.id.clone()).collect()
    }

    #[test]
    fn health_facet_narrows_rows_and_or_es_across_values() {
        let rows = workloads();
        let mut selection = FacetSelection::default();
        selection.toggle_health(Health::Error);
        assert_eq!(
            names(&filter_rows(&rows, &selection)),
            vec!["default/web-2"]
        );

        selection.toggle_health(Health::Healthy);
        assert_eq!(
            names(&filter_rows(&rows, &selection)),
            vec!["default/web-1", "default/web-2", "data/job-1"]
        );
    }

    #[test]
    fn kind_facet_narrows_rows() {
        let rows = workloads();
        let mut selection = FacetSelection::default();
        selection.toggle_kind("Pod");
        assert_eq!(
            names(&filter_rows(&rows, &selection)),
            vec!["default/web-1", "default/web-2"]
        );
    }

    #[test]
    fn facets_are_and_ed() {
        let rows = workloads();
        let mut selection = FacetSelection::default();
        selection.toggle_kind("Pod");
        selection.toggle_health(Health::Healthy);
        assert_eq!(
            names(&filter_rows(&rows, &selection)),
            vec!["default/web-1"]
        );
    }

    #[test]
    fn clear_all_restores_every_row() {
        let rows = workloads();
        let mut selection = FacetSelection::default();
        selection.toggle_health(Health::Error);
        selection.toggle_kind("Pod");
        assert_eq!(filter_rows(&rows, &selection).len(), 1);

        selection.clear();
        assert!(selection.is_empty());
        assert_eq!(filter_rows(&rows, &selection).len(), rows.len());
    }

    #[test]
    fn row_health_takes_the_worst_signal() {
        let mut mixed = row("mixed", "default", Some("Pod"), Some(StatusKind::Running));
        mixed.cells.push(Cell {
            text: String::new(),
            status: None,
            sort: SortKey::Text(String::new()),
            extras: CellExtras::HealthDots(vec![HealthDot::Ok, HealthDot::Err]),
        });
        assert_eq!(row_health(&mixed), Some(Health::Error));
        assert_eq!(row_health(&row("plain", "default", None, None)), None);
    }

    #[test]
    fn facets_count_rows_per_value() {
        let counts = FacetCounts::from_rows(&workloads());
        assert_eq!(
            counts.namespaces,
            vec![
                FacetCount {
                    value: "data".to_string(),
                    count: 2
                },
                FacetCount {
                    value: "default".to_string(),
                    count: 2
                },
            ]
        );
        assert_eq!(
            counts
                .kinds
                .iter()
                .map(|option| (option.value.as_str(), option.count))
                .collect::<Vec<_>>(),
            vec![("Job", 1), ("Pod", 2), ("StatefulSet", 1)]
        );
        assert_eq!(
            counts
                .health
                .iter()
                .map(|option| (option.value, option.count))
                .collect::<Vec<_>>(),
            vec![
                (Health::Healthy, 2),
                (Health::Warning, 1),
                (Health::Error, 1)
            ]
        );
    }

    #[test]
    fn rows_without_a_kind_never_match_a_kind_facet() {
        let rows = vec![row("bare", "default", None, None)];
        let mut selection = FacetSelection::default();
        selection.toggle_kind("Pod");
        assert!(filter_rows(&rows, &selection).is_empty());
        assert_eq!(filter_rows(&rows, &FacetSelection::default()).len(), 1);
    }
}
