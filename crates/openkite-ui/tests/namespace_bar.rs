//! The namespace bar's logic, pinned without a cluster or a browser.

mod support;

use dioxus::prelude::*;
use openkite_ui::components::namespace_bar::{
    chip_rows, selection_matches, toggle_selection, visible_namespaces, NamespaceBar, ALL_LABEL,
};
use openkite_ui::components::resource_table::{
    namespace_filter, Cell, ColumnDef, ResourceRow, ResourceTable, TableStatus,
};
use openkite_ui::runtime::{
    clear_namespace_selection, selected_namespaces, set_namespace_options, set_namespace_selection,
};

use support::mount_html;

fn ns(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

/// Run `f` inside a throwaway Dioxus runtime so global signals are usable.
fn with_runtime<O>(f: impl FnOnce() -> O) -> O {
    fn stub() -> Element {
        rsx! { div {} }
    }
    let vdom = VirtualDom::new(stub);
    vdom.in_runtime(f)
}

fn bar_with_options() -> Element {
    rsx! {
        NamespaceBar {
            options: vec!["default".to_string(), "kube-system".to_string(), "argocd".to_string()]
        }
    }
}

fn bar_without_options() -> Element {
    rsx! { NamespaceBar { options: Vec::new() } }
}

fn table_with_rows() -> Element {
    rsx! {
        ResourceTable {
            columns: vec![
                ColumnDef { key: "name", label: "Name", width: None, sortable: false },
            ],
            rows: vec![
                ResourceRow { id: "a".into(), namespace: Some("default".into()), cells: vec![Cell::text("row-alpha")] },
                ResourceRow { id: "b".into(), namespace: Some("argocd".into()), cells: vec![Cell::text("row-beta")] },
                ResourceRow { id: "c".into(), namespace: None, cells: vec![Cell::text("row-gamma")] },
            ],
            status: TableStatus::Ready,
        }
    }
}

#[test]
fn selection_matches_treats_empty_selection_as_all_namespaces() {
    assert!(selection_matches(&[], Some("default")));
    assert!(selection_matches(&[], Some("kube-system")));
    assert!(selection_matches(&ns(&["argocd"]), Some("argocd")));
    assert!(!selection_matches(&ns(&["argocd"]), Some("default")));
    assert!(selection_matches(
        &ns(&["default", "argocd"]),
        Some("argocd")
    ));
}

#[test]
fn selection_matches_keeps_cluster_scoped_objects() {
    assert!(selection_matches(&ns(&["default"]), None));
    assert!(selection_matches(&[], None));
}

#[test]
fn toggle_selection_adds_then_removes_preserving_order() {
    let selection = toggle_selection(&[], "default");
    assert_eq!(selection, ns(&["default"]));
    let selection = toggle_selection(&selection, "argocd");
    assert_eq!(selection, ns(&["default", "argocd"]));
    let selection = toggle_selection(&selection, "default");
    assert_eq!(selection, ns(&["argocd"]));
}

#[test]
fn visible_namespaces_narrows_the_chip_list_case_insensitively() {
    let options = ns(&["default", "kube-system", "argocd"]);
    assert_eq!(visible_namespaces(&options, ""), options);
    assert_eq!(visible_namespaces(&options, "  "), options);
    assert_eq!(visible_namespaces(&options, "KUBE"), ns(&["kube-system"]));
    assert_eq!(
        visible_namespaces(&options, "e"),
        ns(&["default", "kube-system"])
    );
    assert_eq!(visible_namespaces(&options, "o"), ns(&["argocd"]));
    assert!(visible_namespaces(&options, "zzz").is_empty());
}

/// The headline guarantee: the search field narrows the *list*; the selection
/// survives any query.
#[test]
fn filtering_the_chip_list_never_changes_the_selection() {
    let options = ns(&["default", "kube-system", "argocd"]);
    let mut selection = ns(&["argocd"]);

    let rows = chip_rows(&options, "default", &selection);
    assert_eq!(
        rows,
        vec![("default".to_string(), false)],
        "only `default` is listed, and it is not selected"
    );
    assert_eq!(
        selection,
        ns(&["argocd"]),
        "typing never rewrites the selection"
    );
    assert!(
        selection_matches(&selection, Some("argocd")),
        "the filtered-out namespace is still scoped in"
    );

    let rows = chip_rows(&options, "", &selection);
    let active: Vec<&str> = rows
        .iter()
        .filter(|(_, active)| *active)
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(active, vec!["argocd"]);

    selection = toggle_selection(&selection, "default");
    assert_eq!(selection, ns(&["argocd", "default"]));
    selection = toggle_selection(&selection, "argocd");
    selection = toggle_selection(&selection, "default");
    assert!(
        selection.is_empty(),
        "untoggling every chip = all namespaces"
    );
}

#[test]
fn reset_returns_to_all_namespaces() {
    with_runtime(|| {
        set_namespace_options(ns(&["default", "kube-system"]));
        set_namespace_selection(ns(&["default", "kube-system"]));
        assert_eq!(selected_namespaces().len(), 2);

        clear_namespace_selection();
        assert!(
            selected_namespaces().is_empty(),
            "the × reset clears to the empty selection = all namespaces"
        );
        // And an empty selection matches everything.
        assert!(selection_matches(&selected_namespaces(), Some("anything")));
    });
}

#[test]
fn the_bar_renders_one_line_strip_with_search_circle_and_no_reset() {
    let html = mount_html(bar_with_options, || {});
    assert!(html.contains("class=\"ns-bar\""), "got: {html}");
    assert!(html.contains("chip-row"), "the strip: {html}");
    assert!(html.contains("ns-search"), "the search circle: {html}");
    assert!(
        html.contains(ALL_LABEL),
        "the first chip restores all namespaces: {html}"
    );
    assert!(
        html.contains("aria-expanded=\"false\""),
        "the inline filter starts closed: {html}"
    );
    assert!(
        !html.contains("ns-reset"),
        "the × reset must not render without a selection: {html}"
    );
    assert!(
        html.contains("aria-label=\"Namespace filter\""),
        "the strip is a labelled group: {html}"
    );
}

#[test]
fn a_selection_highlights_chips_and_pins_the_reset() {
    let html = mount_html(bar_with_options, || {
        set_namespace_selection(ns(&["argocd"]));
    });
    assert!(
        html.contains("aria-pressed=\"true\""),
        "the selected chip is pressed: {html}"
    );
    assert!(
        html.contains("data-ns=\"argocd\"") && html.contains("chip active"),
        "the selected chip carries the active class: {html}"
    );
    assert!(
        html.contains("chip-mark"),
        "the selected chip wears a check: {html}"
    );
    assert!(
        html.contains("ns-reset"),
        "the reset appears only while a selection exists: {html}"
    );
    assert!(
        html.contains("aria-label=\"Clear namespace filter\""),
        "the reset is labelled: {html}"
    );
    let strip = html.find("class=\"chip-row\"").expect("strip");
    let reset = html.find("ns-reset").expect("reset");
    assert!(reset > strip, "the reset trails the strip: {html}");
}

#[test]
fn the_bar_declares_the_unsupported_state_without_namespaces() {
    let html = mount_html(bar_without_options, || {});
    assert!(
        html.contains("data-empty=\"namespaces\""),
        "no namespace inventory is declared, not silently absent: {html}"
    );
    assert!(
        html.contains("data-unsupported=\"namespace-filter\""),
        "the design's unsupported declaration: {html}"
    );
}

/// An empty selection shows every row (cluster-scoped rows included);
/// selecting a namespace narrows the table without touching the selection.
#[test]
fn selection_scopes_a_resource_table() {
    let html = mount_html(table_with_rows, || {
        set_namespace_selection(ns(&["argocd"]));
    });
    assert!(
        html.contains("row-beta"),
        "the argocd row is in scope: {html}"
    );
    assert!(
        !html.contains("row-alpha"),
        "the default row is out of scope: {html}"
    );
    assert!(
        html.contains("row-gamma"),
        "the cluster-scoped row survives any selection: {html}"
    );

    let all = mount_html(table_with_rows, || {
        clear_namespace_selection();
    });
    assert!(all.contains("row-alpha") && all.contains("row-beta") && all.contains("row-gamma"));
}

#[test]
fn namespace_filter_matches_the_host_snapshot_semantics() {
    let rows = vec![
        ResourceRow {
            id: "a".into(),
            namespace: Some("default".into()),
            cells: vec![Cell::text("a")],
        },
        ResourceRow {
            id: "b".into(),
            namespace: Some("argocd".into()),
            cells: vec![Cell::text("b")],
        },
        ResourceRow {
            id: "c".into(),
            namespace: None,
            cells: vec![Cell::text("c")],
        },
    ];
    assert_eq!(namespace_filter(&rows, &[]).len(), 3, "all namespaces");
    let filtered = namespace_filter(&rows, &ns(&["argocd"]));
    let ids: Vec<&str> = filtered.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, vec!["b", "c"]);
}
