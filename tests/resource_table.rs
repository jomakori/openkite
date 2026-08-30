//! Integration tests for the `ResourceTable` public surface.
//!
//! Re-asserts the pure-logic helpers (`sort_by_key`, `compare_sort_keys`,
//! `visible_range`, `matches_query`, `Cell`, `ResourceRow::search_text`)
//! from the public API and pins the `StatusKind::pill_class` mapping
//! added by the design-system re-skin.

use openkite::components::resource_table::{
    compare_sort_keys, matches_query, sort_by_key, visible_range, Cell, ResourceRow, SortDirection,
    SortKey, OVERSCAN, ROW_HEIGHT,
};
use openkite::components::status_badge::StatusKind;

fn row_with_namespace(namespace: Option<&str>, cells: Vec<Cell>) -> ResourceRow {
    ResourceRow {
        id: "1".into(),
        namespace: namespace.map(str::to_string),
        cells,
    }
}

#[test]
fn sort_by_key_orders_numbers_both_directions() {
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
fn compare_sort_keys_orders_numbers_before_text() {
    use std::cmp::Ordering;
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
fn compare_sort_keys_text_is_case_insensitive() {
    use std::cmp::Ordering;
    assert_eq!(
        compare_sort_keys(
            &SortKey::Text("apple".into()),
            &SortKey::Text("APPLE".into())
        ),
        Ordering::Equal
    );
}

#[test]
fn visible_range_is_bounded_for_large_lists() {
    let range = visible_range(0.0, 600.0, 10_000);
    assert!(range.end - range.start < 50, "window must be bounded");
    assert!(range.end <= 10_000);
}

#[test]
fn visible_range_is_empty_without_rows() {
    assert_eq!(visible_range(0.0, 600.0, 0), 0..0);
    assert_eq!(visible_range(0.0, 0.0, 10), 0..0);
}

#[test]
fn visible_range_scales_with_scroll_offset() {
    let range = visible_range(360.0, 600.0, 1000);
    assert!(range.start > 0);
    assert!(range.start <= 10 + OVERSCAN);
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
    let row = row_with_namespace(
        Some("default"),
        vec![
            Cell::text("nginx"),
            Cell::status("Running", StatusKind::Running),
        ],
    );
    let haystack = row.search_text();
    assert!(haystack.contains("default"));
    assert!(haystack.contains("nginx"));
    assert!(haystack.contains("Running"));
}

#[test]
fn row_height_is_a_positive_constant() {
    // Compile-time invariants, kept here so a failure points at this test.
    const _: () = assert!(ROW_HEIGHT > 0.0);
    const _: () = assert!(OVERSCAN == 8);
}

#[test]
fn status_kind_pill_classes_cover_every_variant() {
    let all = [
        StatusKind::Running,
        StatusKind::Ready,
        StatusKind::Pending,
        StatusKind::Succeeded,
        StatusKind::Failed,
        StatusKind::CrashLoop,
        StatusKind::Unknown,
        StatusKind::OutOfSync,
        StatusKind::Degraded,
        StatusKind::Suspended,
    ];
    for kind in all {
        let class = kind.pill_class();
        assert!(
            matches!(class, "success" | "warn" | "danger" | "muted"),
            "{kind:?} mapped to unrecognized pill variant `{class}`"
        );
    }
}

#[test]
fn pill_class_mapping_matches_legacy_class() {
    // Legacy classes map ok→success, warn→warn, err→danger, muted→muted.
    // A drift in one prefix without the other is a regression in the
    // design-system re-skin.
    assert_eq!(StatusKind::Running.pill_class(), "success");
    assert_eq!(StatusKind::Ready.pill_class(), "success");
    assert_eq!(StatusKind::Succeeded.pill_class(), "success");
    assert_eq!(StatusKind::Pending.pill_class(), "warn");
    assert_eq!(StatusKind::OutOfSync.pill_class(), "warn");
    assert_eq!(StatusKind::Failed.pill_class(), "danger");
    assert_eq!(StatusKind::CrashLoop.pill_class(), "danger");
    assert_eq!(StatusKind::Degraded.pill_class(), "danger");
    assert_eq!(StatusKind::Unknown.pill_class(), "muted");
    assert_eq!(StatusKind::Suspended.pill_class(), "muted");

    assert_eq!(StatusKind::Running.class(), "status-ok");
    assert_eq!(StatusKind::Pending.class(), "status-warn");
    assert_eq!(StatusKind::Failed.class(), "status-err");
    assert_eq!(StatusKind::Unknown.class(), "status-muted");
}
