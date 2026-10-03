//! The Pod inventory surface (OKT-162): the reference's `#view-workloads`
//! table rendered headlessly plus the pure mapping helpers.

mod support;

use dioxus::prelude::*;
use openkite_ui::components::pod_inventory::{
    pod_columns, pod_status_kind, PodInventory, PodRow, EMPTY_MESSAGE, PODS_PER_PAGE,
};
use openkite_ui::components::resource_table::{
    page_count, pager_slots, showing_label, HealthDot, TableStatus,
};
use openkite_ui::components::status_badge::StatusKind;
use openkite_ui::runtime::set_namespace_selection;

use support::mount_html;

fn pod(name: &str, namespace: &str, restarts: i32, phase: &str) -> PodRow {
    PodRow {
        name: name.to_string(),
        namespace: namespace.to_string(),
        health: vec![HealthDot::Ok, HealthDot::Err],
        restarts,
        controller: format!("Deployment/{name}"),
        node: "ip-10-0-4-18".to_string(),
        qos: "Burstable".to_string(),
        age: "2d".to_string(),
        phase: phase.to_string(),
    }
}

fn inventory(rows: Vec<PodRow>, status: TableStatus) -> Element {
    rsx! { PodInventory { rows: rows, status: status } }
}

fn populated() -> Element {
    inventory(
        vec![
            pod("checkout-api-7c9f8d6b4f-x2k9m", "default", 0, "Running"),
            pod(
                "payments-worker-6b8d9c7f5d-9nq4r",
                "default",
                14,
                "CrashLoopBackOff",
            ),
        ],
        TableStatus::Ready,
    )
}

fn two_namespaces() -> Element {
    inventory(
        vec![
            pod("alpha-pod", "default", 0, "Running"),
            pod("beta-pod", "kube-system", 0, "Running"),
        ],
        TableStatus::Ready,
    )
}

fn paged() -> Element {
    let rows = (0..PODS_PER_PAGE + 5)
        .map(|i| pod(&format!("pod-{i:02}"), "default", 0, "Running"))
        .collect();
    inventory(rows, TableStatus::Ready)
}

fn empty() -> Element {
    inventory(Vec::new(), TableStatus::Ready)
}

fn loading() -> Element {
    inventory(Vec::new(), TableStatus::Loading)
}

fn failed() -> Element {
    inventory(
        Vec::new(),
        TableStatus::Error("cluster unreachable".to_string()),
    )
}

#[test]
fn pod_columns_match_the_reference_order() {
    let labels: Vec<&str> = pod_columns().iter().map(|column| column.label).collect();
    assert_eq!(
        labels,
        [
            "Name",
            "Namespace",
            "Health",
            "Restarts",
            "Controller",
            "Node",
            "QoS",
            "Age",
            "Status",
        ]
    );
}

#[test]
fn crashloop_reasons_map_to_the_crashloop_status() {
    assert_eq!(pod_status_kind("Running"), StatusKind::Running);
    assert_eq!(pod_status_kind("CrashLoopBackOff"), StatusKind::CrashLoop);
    assert_eq!(pod_status_kind("Pending"), StatusKind::Pending);
    assert_eq!(pod_status_kind("Succeeded"), StatusKind::Succeeded);
    assert_eq!(pod_status_kind("Failed"), StatusKind::Failed);
    assert_eq!(pod_status_kind(""), StatusKind::Unknown);
    assert_ne!(pod_status_kind("CrashLoopBackOff"), StatusKind::Running);
}

#[test]
fn pod_row_paints_the_reference_cell_classes() {
    let resource = pod("payments-worker", "default", 14, "CrashLoopBackOff").to_resource_row();
    assert_eq!(resource.id, "default/payments-worker");
    assert_eq!(resource.namespace.as_deref(), Some("default"));
    let classes: Vec<&str> = resource
        .cells
        .iter()
        .map(|cell| cell.class.as_str())
        .collect();
    assert_eq!(classes[0], "resource-name");
    assert_eq!(classes[1], "namespace");
    assert_eq!(classes[3], "restarts warn");
    assert_eq!(classes[4], "controller");
    assert_eq!(classes[5], "node");
    assert_eq!(classes[6], "qos");
    assert_eq!(classes[7], "qos");
    assert_eq!(resource.cells[8].status, Some(StatusKind::CrashLoop));
    assert_eq!(resource.cells[8].text, "CrashLoopBackOff");
}

#[test]
fn zero_restarts_carry_no_warn_class() {
    let resource = pod("coredns", "kube-system", 0, "Running").to_resource_row();
    assert_eq!(resource.cells[3].class, "restarts");
    assert_eq!(resource.cells[8].status, Some(StatusKind::Running));
}

#[test]
fn populated_inventory_renders_every_reference_column_and_row() {
    let html = mount_html(populated, || {});
    for label in [
        "Name",
        "Namespace",
        "Health",
        "Restarts",
        "Controller",
        "Node",
        "QoS",
        "Age",
        "Status",
    ] {
        assert!(html.contains(label), "missing column `{label}`: {html}");
    }
    assert!(
        html.contains("checkout-api-7c9f8d6b4f-x2k9m"),
        "got: {html}"
    );
    assert!(html.contains("resource-name"), "got: {html}");
    assert!(html.contains("health-dots"), "got: {html}");
    assert!(html.contains("dot ok"), "got: {html}");
    assert!(html.contains("dot err"), "got: {html}");
    assert!(html.contains("panel-footer"), "got: {html}");
    assert!(html.contains("pager"), "got: {html}");
    assert!(html.contains("Showing 2 of 2 pods"), "got: {html}");
}

#[test]
fn crashloop_status_renders_the_danger_pill() {
    let html = mount_html(populated, || {});
    assert!(html.contains("CrashLoopBackOff"), "got: {html}");
    assert!(html.contains("pill danger"), "got: {html}");
    assert!(html.contains("restarts warn"), "got: {html}");
}

#[test]
fn empty_inventory_renders_the_empty_state() {
    let html = mount_html(empty, || {});
    assert!(html.contains("table-empty"), "got: {html}");
    assert!(html.contains(EMPTY_MESSAGE), "got: {html}");
}

#[test]
fn loading_inventory_renders_the_loading_state() {
    let html = mount_html(loading, || {});
    assert!(html.contains("Loading…"), "got: {html}");
}

#[test]
fn error_inventory_renders_the_error_state() {
    let html = mount_html(failed, || {});
    assert!(html.contains("table-error"), "got: {html}");
    assert!(html.contains("cluster unreachable"), "got: {html}");
}

#[test]
fn inventory_pages_show_one_page_and_a_real_pager() {
    let html = mount_html(paged, || {});
    assert!(html.contains("Showing 10 of 15 pods"), "got: {html}");
    assert!(html.contains("pager"), "got: {html}");
    assert!(html.contains(">2<"), "the second page button: {html}");
}

#[test]
fn namespace_selection_scopes_the_inventory() {
    let html = mount_html(two_namespaces, || {
        set_namespace_selection(vec!["kube-system".to_string()]);
    });
    assert!(html.contains("beta-pod"), "got: {html}");
    assert!(!html.contains("alpha-pod"), "got: {html}");
}

#[test]
fn the_new_cell_classes_are_in_the_stylesheet() {
    assert!(openkite_ui::MAIN_CSS.contains(".cell-value"));
    assert!(openkite_ui::MAIN_CSS.contains(".node"));
}

#[test]
fn pager_helpers_bound_their_windows() {
    assert_eq!(page_count(0, 10), 1);
    assert_eq!(page_count(10, 10), 1);
    assert_eq!(page_count(15, 10), 2);
    assert_eq!(page_count(5, 0), 1);
    assert_eq!(pager_slots(1, 3), vec![1, 2, 3]);
    assert_eq!(pager_slots(1, 16), vec![1, 2, 3, 4, 5]);
    assert_eq!(pager_slots(16, 16), vec![12, 13, 14, 15, 16]);
    assert_eq!(showing_label(10, 15, "pods"), "Showing 10 of 15 pods");
}
