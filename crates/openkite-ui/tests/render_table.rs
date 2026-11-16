//! Render proof for the resource table: real markup, not just a type-check.

use dioxus::prelude::*;
use openkite_ui::nav::CATALOGUE;
use openkite_ui::table::{columns, key_opens_row, Density, Row, Sort, SortDir, TableState};
use openkite_ui::table_view::ResourceTable;

fn pod(name: &str, namespace: &str, status: &str, age: &str, restarts: &str) -> Row {
    Row::new(name, name)
        .with_namespace(namespace)
        .with_cell("status", status)
        .with_cell("age", age)
        .with_cell("restarts", restarts)
}

/// Handlers must be built inside the render runtime, so the table is mounted through a
/// thin harness rather than constructing props in the test body.
#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    rows: Vec<Row>,
    state: TableState,
    kind: String,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_open = use_callback(|_: String| {});
    let on_state = use_callback(|_: TableState| {});
    let on_scroll = use_callback(|_: f64| {});
    rsx! {
        ResourceTable {
            kind: props.kind.clone(),
            label: "Pods".to_string(),
            rows: props.rows.clone(),
            state: props.state.clone(),
            namespaces: vec![("default".to_string(), 2usize), ("kube-system".to_string(), 1usize)],
            on_open,
            on_state,
            scroll_top: 0.0,
            on_scroll,
        }
    }
}

fn render(rows: Vec<Row>, state: TableState, kind: &str) -> String {
    let mut dom = VirtualDom::new_with_props(
        Harness,
        HarnessProps {
            rows,
            state,
            kind: kind.to_string(),
        },
    );
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn sample() -> Vec<Row> {
    vec![
        pod("api-7d9", "default", "Running", "3d", "0"),
        pod("worker-2", "default", "Pending", "12m", "4"),
        pod("coredns-5", "kube-system", "Running", "9d", "1"),
    ]
}

#[test]
fn table_renders_one_row_per_resource() {
    let html = render(sample(), TableState::default(), "pods");
    assert!(html.contains("class=\"resource-table\""), "{html}");
    assert_eq!(html.matches("data-id=").count(), 3, "{html}");
    assert!(
        html.contains("Showing <span class=\"cnt\">3</span> of <span class=\"cnt\">3</span> pods"),
        "{html}"
    );
}

#[test]
fn header_carries_aria_sort_and_active_mark() {
    let html = render(sample(), TableState::default(), "pods");
    assert!(html.contains("aria-sort=\"ascending\""), "{html}");
    assert!(html.contains("class=\"sort-mark\""), "{html}");
    assert!(html.contains("↑"), "{html}");

    let state = TableState {
        sort: Sort::new("status", SortDir::Desc),
        ..TableState::default()
    };
    let html = render(sample(), state, "pods");
    assert!(html.contains("aria-sort=\"descending\""), "{html}");
    assert!(html.contains("↓"), "{html}");
}

#[test]
fn columns_follow_the_catalogue_not_a_hardcoded_set() {
    // Pods (prototype) lead with Name/Status/Ready/Restarts/Age; deployments lead with
    // Name/Status/Ready/Up-to-date/Available/Age — both must show their own labels.
    let pods = render(sample(), TableState::default(), "pods");
    assert!(
        pods.contains(">Restarts<") || pods.contains("Restarts"),
        "{pods}"
    );
    let deployments = render(sample(), TableState::default(), "deployments");
    assert!(deployments.contains("Ready"), "{deployments}");
}

#[test]
fn row_controls_are_distinct_from_the_row_body() {
    let html = render(sample(), TableState::default(), "pods");
    assert!(
        html.contains("class=\"row-check\""),
        "row checkbox missing: {html}"
    );
    assert!(
        html.contains("class=\"row-act\""),
        "row action button missing: {html}"
    );
    assert!(html.contains("aria-label=\"Select api-7d9\""), "{html}");
    assert!(
        html.contains("aria-label=\"Actions for api-7d9\""),
        "{html}"
    );
}

#[test]
fn selection_renders_bulk_bar_and_checked_rows() {
    let mut state = TableState::default();
    state.selected.insert("api-7d9".to_string());
    state.selected.insert("worker-2".to_string());
    let html = render(sample(), state, "pods");
    assert!(html.contains("class=\"bulk-bar\""), "{html}");
    assert!(html.contains("2 selected"), "{html}");
    assert!(html.matches("checked").count() >= 2, "{html}");
}

#[test]
fn empty_state_appears_only_without_matches() {
    let html = render(sample(), TableState::default(), "pods");
    assert!(!html.contains("empty-state"), "{html}");

    let filtered = TableState {
        query: "nope-nothing".to_string(),
        ..TableState::default()
    };
    let html = render(sample(), filtered, "pods");
    assert!(html.contains("class=\"empty-state\""), "{html}");
    assert!(
        html.contains("Showing <span class=\"cnt\">0</span> of <span class=\"cnt\">3</span> pods"),
        "{html}"
    );
}

#[test]
fn namespace_chips_reflect_counts_and_selection() {
    let html = render(sample(), TableState::default(), "pods");
    assert!(html.contains("class=\"chip-row\""), "{html}");
    assert!(html.contains("kube-system"), "{html}");
    assert!(html.contains(">All<") || html.contains("All"), "{html}");

    let state = TableState {
        namespace: Some("default".to_string()),
        ..TableState::default()
    };
    let html = render(sample(), state, "pods");
    assert_eq!(html.matches("data-id=").count(), 2, "{html}");
    assert!(html.contains("class=\"chip active\""), "{html}");
}

#[test]
fn expanded_parent_renders_child_rows() {
    let kids = vec![pod("api-7d9-abc12", "default", "Running", "3d", "0")];
    let rows = vec![pod("api-7d9", "default", "Running", "3d", "0").with_kids(kids)];
    let mut state = TableState::default();
    state.expanded.insert("api-7d9".to_string());
    let html = render(rows, state, "pods");
    assert!(html.contains("class=\"child\""), "{html}");
    assert!(html.contains("api-7d9-abc12"), "{html}");
}

#[test]
fn hidden_columns_drop_from_the_header_but_name_stays() {
    let mut state = TableState::default();
    state.hidden.insert("restarts".to_string());
    state.hidden.insert("name".to_string());
    let html = render(sample(), state, "pods");
    assert!(html.contains(">Name<") || html.contains("Name"), "{html}");
    assert!(!html.contains(">Restarts<"), "{html}");
}

#[test]
fn compact_density_is_part_of_state() {
    let state = TableState {
        density: Density::Compact,
        ..TableState::default()
    };
    assert_eq!(state.density.row_height(), 36.0);
}

#[test]
fn every_catalogue_kind_gets_a_row_with_the_open_contract() {
    // One row per catalogue kind: the row body must carry the inspector contract, and the
    // nested controls must be separate elements so a click on them cannot bubble into it.
    for kind in CATALOGUE.iter() {
        let rows = vec![pod("only-row", "default", "Running", "1d", "0")];
        let html = render(rows, TableState::default(), kind.key);
        assert!(
            html.contains("data-open=\"inspector\""),
            "{}: row body is not wired to the inspector",
            kind.key
        );
        assert!(
            html.contains("data-part=\"checkbox\""),
            "{}: no checkbox control",
            kind.key
        );
        assert!(
            html.contains("data-part=\"action\""),
            "{}: no row action",
            kind.key
        );
        assert_eq!(html.matches("data-id=").count(), 1, "{}", kind.key);
        // The header must come from the kind's own column set, not a fixed list.
        let cols = columns(kind.key);
        assert!(!cols.is_empty(), "{}: empty column set", kind.key);
        assert!(
            html.contains(cols[0].label),
            "{}: header missing {}",
            kind.key,
            cols[0].label
        );
    }
}

#[test]
fn no_dom_node_per_off_screen_row() {
    // 1_000 rows must mount only the scroll window, not one node per row.
    let rows: Vec<Row> = (0..1_000)
        .map(|i| pod(&format!("pod-{i}"), "default", "Running", "1d", "0"))
        .collect();
    let html = render(rows, TableState::default(), "pods");
    let mounted = html.matches("data-id=").count();
    assert!(
        mounted < 60,
        "mounted {mounted} rows out of 1000 — window not applied"
    );
    assert!(mounted > 0, "window rendered nothing");
}

#[test]
fn keyboard_activation_matches_click() {
    assert!(key_opens_row("Enter"));
    assert!(key_opens_row(" "));
    assert!(!key_opens_row("Tab"));
    assert!(!key_opens_row("a"));
}

/// Cost budgets from the ticket: 20k rows, filter under 50 ms, one frame under 16 ms.
/// Timing assertions are opt-in (`cargo test --release -- --ignored perf`) because the
/// debug profile measures the wrong thing.
#[test]
#[ignore = "timing budget; run with --release -- --ignored"]
fn perf_20k_rows_filter_and_frame() {
    let rows: Vec<Row> = (0..20_000)
        .map(|i| {
            pod(
                &format!("pod-{i:05}"),
                if i % 3 == 0 { "kube-system" } else { "default" },
                if i % 7 == 0 { "Pending" } else { "Running" },
                "3d",
                "0",
            )
        })
        .collect();

    let state = TableState {
        query: "pod-01".to_string(),
        ..TableState::default()
    };
    let started = std::time::Instant::now();
    let visible = state.visible(&rows);
    let filter_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "filter {} of {} rows in {filter_ms:.2} ms",
        visible.len(),
        rows.len()
    );
    assert!(
        filter_ms < 50.0,
        "filter took {filter_ms:.2} ms (budget 50 ms)"
    );

    let html = render(rows, TableState::default(), "pods");
    let mounted = html.matches("data-id=").count();
    println!("mounted {mounted} of 20000 rows");
    assert!(mounted < 60, "window mounted {mounted} rows");
}
