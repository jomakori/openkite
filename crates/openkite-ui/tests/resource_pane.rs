//! Headless mounts of the resource detail side pane (OKT-175).
//!
//! These pin the four things the ticket's acceptance turns on that a headless
//! render can answer:
//!
//! - the pane opens at the same stop for every kind (one `.inspector` node, the
//!   same chrome, no per-kind structure),
//! - a second selection REPLACES the pane instead of stacking a second one,
//! - re-clicking the same row closes it,
//! - the pane's contents are the row's own data — identity, status, metadata,
//!   controller owner and the pane's own actions.
//!
//! The ≤767px bottom sheet and the scrim are stylesheet behaviour: they are
//! pinned in the mount (the scrim node and the pane's bounds are in the markup)
//! and in `tests/design.rs` (the rules exist), because a headless render has no
//! viewport to resolve a media query against.
//!
//! Every test seeds the global in `setup`: the crate's globals are
//! process-wide, so a test that read another test's selection would pass or
//! fail by scheduling luck.

mod support;

use dioxus::prelude::*;
use openkite_ui::components::resource_pane::{
    clamp_pane_width, open_resource, toggle_resource_row, PaneAction, ResourceDetail, ResourcePane,
    ResourceRef, PANE_MAX_WIDTH, PANE_MIN_WIDTH,
};
use openkite_ui::components::resource_table::{
    Cell, ColumnDef, ResourceRow, ResourceTable, TableStatus,
};
use openkite_ui::components::status_badge::StatusKind;
use openkite_ui::runtime::{clear_resource_selection, selected_resource};

/// The kinds the ticket names, with their scope.
const KINDS: [(&str, Option<&str>); 6] = [
    ("Pod", Some("default")),
    ("Deployment", Some("default")),
    ("Service", Some("default")),
    ("Node", None),
    ("ConfigMap", Some("kube-system")),
    ("Application", Some("argocd")),
];

/// The chrome every kind must paint, identically.
const PANE_CHROME: [&str; 9] = [
    "inspector open",
    "inspector-resize",
    "inspector-header",
    "inspector-title",
    "inspector-body",
    "inspector-eyebrow",
    "inspector-actions",
    "kv-list",
    "resource-kind",
];

fn pane() -> Element {
    rsx! { ResourcePane {} }
}

fn detail(kind: &str, namespace: Option<&str>, name: &str) -> ResourceDetail {
    ResourceDetail::from_ref(ResourceRef::new(kind, namespace.map(str::to_string), name))
}

fn columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            key: "name".into(),
            label: "Name".into(),
            width: Some(180),
            sortable: true,
        },
        ColumnDef {
            key: "status".into(),
            label: "Status".into(),
            width: None,
            sortable: false,
        },
        ColumnDef {
            key: "controller".into(),
            label: "Controller".into(),
            width: None,
            sortable: false,
        },
        ColumnDef {
            key: "qos".into(),
            label: "QoS".into(),
            width: None,
            sortable: false,
        },
    ]
}

fn pod_row(id: &str) -> ResourceRow {
    ResourceRow {
        id: id.to_string(),
        namespace: Some("default".to_string()),
        cells: vec![
            Cell::text("checkout-api"),
            Cell::status("Running", StatusKind::Running),
            Cell::text("Deployment/checkout-api"),
            Cell::text("Burstable"),
        ],
    }
}

/// How many panes the render produced. Two panes at the same stop would be a
/// stack, so the number is asserted rather than assumed.
fn pane_count(html: &str) -> usize {
    html.matches("data-pane=\"resource\"").count()
}

/// The class list on the pane's own `<aside>`, so the chrome can be compared
/// across kinds.
fn pane_classes(html: &str) -> Vec<String> {
    let anchor = html.find("data-pane=\"resource\"").expect("pane rendered");
    let start = html[..anchor]
        .rfind("<aside")
        .expect("the pane is an aside");
    let end = html[anchor..]
        .find('>')
        .map(|offset| anchor + offset)
        .expect("the tag closes");
    let open_tag = &html[start..=end];
    let class_at = open_tag.find("class=\"").expect("the pane has a class");
    let rest = &open_tag[class_at + "class=\"".len()..];
    let close = rest.find('"').expect("the class attribute closes");
    rest[..close]
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

#[test]
fn a_closed_pane_renders_nothing() {
    let html = support::mount_html(pane, clear_resource_selection);
    assert_eq!(pane_count(&html), 0, "got: {html}");
    assert!(!html.contains("inspector"), "got: {html}");
}

#[test]
fn every_kind_opens_at_the_same_stop() {
    let mut chrome: Vec<Vec<String>> = Vec::new();
    for (kind, namespace) in KINDS {
        let html = support::mount_html(pane, move || {
            open_resource(detail(kind, namespace, "web-1"));
        });
        assert_eq!(pane_count(&html), 1, "{kind} rendered a stack: {html}");
        assert_eq!(
            pane_classes(&html),
            vec!["inspector".to_string(), "open".to_string()],
            "{kind} changed the pane's own chrome"
        );
        for class in PANE_CHROME {
            assert!(html.contains(class), "{kind} is missing .{class}: {html}");
        }
        assert!(
            html.contains(&format!("data-kind=\"{kind}\"")),
            "{kind} did not reach the pane: {html}"
        );
        chrome.push(pane_classes(&html));
    }
    for pair in chrome.windows(2) {
        assert_eq!(
            pair[0], pair[1],
            "the pane's position must not depend on the kind"
        );
    }
}

#[test]
fn the_pane_paints_identity_status_metadata_owner_and_actions() {
    let html = support::mount_html(pane, || {
        let detail = detail("Pod", Some("default"), "checkout-api-7d9f")
            .with_status("Running", StatusKind::Running)
            .with_metadata("QoS", "Burstable")
            .with_metadata("Restarts", "2")
            .with_owner("Deployment/checkout-api");
        open_resource(detail);
    });
    for needle in [
        "checkout-api-7d9f",
        "namespace: default",
        "class=\"resource-kind\"",
        "Running",
        "Burstable",
        "Restarts",
        "Owner",
        "Deployment/checkout-api",
    ] {
        assert!(html.contains(needle), "pane missing {needle}: {html}");
    }
    for action in PaneAction::ALL {
        assert!(
            html.contains(&format!("data-action=\"{}\"", action.id())),
            "pane missing action {}: {html}",
            action.id()
        );
        assert!(html.contains(action.label()), "got: {html}");
    }
}

#[test]
fn a_cluster_scoped_kind_says_so_instead_of_inventing_a_namespace() {
    let html = support::mount_html(pane, || {
        open_resource(detail("Node", None, "ip-10-0-4-18"));
    });
    assert!(html.contains("cluster-scoped"), "got: {html}");
    assert!(!html.contains("namespace: default"), "got: {html}");
}

#[test]
fn a_second_selection_replaces_the_pane_in_place() {
    let html = support::mount_html(pane, || {
        open_resource(detail("Pod", Some("default"), "web-1"));
        open_resource(detail("Deployment", Some("default"), "checkout-api"));
        assert_eq!(
            selected_resource().map(|detail| detail.key()),
            Some("deployment/default/checkout-api".to_string()),
            "the slot holds one selection"
        );
    });
    assert_eq!(
        pane_count(&html),
        1,
        "a second click stacked a pane: {html}"
    );
    assert!(html.contains("checkout-api"), "got: {html}");
    assert!(!html.contains("web-1"), "the replaced pane is gone: {html}");
}

#[test]
fn a_row_click_opens_the_pane_with_the_rows_own_data() {
    let columns = columns();
    let row = pod_row("checkout-api-7d9f");
    let html = support::mount_html(pane, move || {
        toggle_resource_row("Pod", &columns, &row);
    });
    assert_eq!(pane_count(&html), 1, "got: {html}");
    for needle in [
        "checkout-api-7d9f",
        "namespace: default",
        "Running",
        "Deployment/checkout-api",
        "QoS",
        "Burstable",
    ] {
        assert!(html.contains(needle), "pane missing {needle}: {html}");
    }
}

#[test]
fn clicking_the_same_row_again_closes_the_pane() {
    let columns = columns();
    let row = pod_row("checkout-api-7d9f");
    let html = support::mount_html(pane, move || {
        toggle_resource_row("Pod", &columns, &row);
        assert!(selected_resource().is_some(), "the first click opens");
        toggle_resource_row("Pod", &columns, &row);
        assert!(selected_resource().is_none(), "the re-click closes");
    });
    assert_eq!(pane_count(&html), 0, "got: {html}");
}

#[test]
fn clicking_another_row_switches_without_closing() {
    let columns = columns();
    let first = pod_row("web-1");
    let second = pod_row("web-2");
    let html = support::mount_html(pane, move || {
        toggle_resource_row("Pod", &columns, &first);
        toggle_resource_row("Pod", &columns, &second);
        assert_eq!(
            selected_resource().map(|detail| detail.identity.name),
            Some("web-2".to_string())
        );
    });
    assert_eq!(pane_count(&html), 1, "switching must not stack: {html}");
    assert!(html.contains("web-2"), "got: {html}");
}

#[test]
fn the_scrim_and_the_panes_own_close_controls_are_rendered() {
    let html = support::mount_html(pane, || {
        open_resource(detail("Pod", Some("default"), "web-1"));
    });
    // The touch backdrop: a real control, shown by the stylesheet at ≤767px.
    assert!(
        html.contains("class=\"inspector-scrim show\""),
        "got: {html}"
    );
    assert!(html.contains("data-scrim=\"resource-pane\""), "got: {html}");
    assert!(
        html.contains("aria-label=\"Close resource details\""),
        "got: {html}"
    );
    // The × the pane owns, and the Escape target the key listener clicks.
    assert!(
        html.contains("class=\"icon-btn inspector-close\""),
        "got: {html}"
    );
    assert!(html.contains("data-close=\"resource-pane\""), "got: {html}");
    // The drag handle and the bounds the stylesheet's variable clamps to.
    assert!(html.contains("class=\"inspector-resize\""), "got: {html}");
    assert!(
        html.contains("data-resize=\"resource-pane\""),
        "got: {html}"
    );
    assert!(
        html.contains(&format!("data-pane-min=\"{PANE_MIN_WIDTH}\"")),
        "got: {html}"
    );
    assert!(
        html.contains(&format!("data-pane-max=\"{PANE_MAX_WIDTH}\"")),
        "got: {html}"
    );
}

#[test]
fn the_panes_actions_say_why_they_are_inert_until_a_host_wires_them() {
    let html = support::mount_html(pane, || {
        open_resource(detail("Pod", Some("default"), "web-1"));
    });
    assert!(html.contains("disabled"), "got: {html}");
    assert!(
        html.contains(PaneAction::ViewLogs.reason()),
        "an inert control states its reason: {html}"
    );
}

#[test]
fn a_wired_handler_enables_the_panes_actions() {
    fn wired() -> Element {
        rsx! {
            ResourcePane {
                on_action: Some(EventHandler::new(|_action: PaneAction| {})),
            }
        }
    }
    let html = support::mount_html(wired, || {
        open_resource(detail("Pod", Some("default"), "web-1"));
    });
    for action in PaneAction::ALL {
        assert!(
            html.contains(&format!("data-action=\"{}\"", action.id())),
            "got: {html}"
        );
    }
    assert!(
        !html.contains("disabled"),
        "a wired action is enabled: {html}"
    );
}

#[test]
fn the_table_keeps_the_selected_row_marked() {
    fn table() -> Element {
        rsx! {
            ResourceTable {
                columns: columns(),
                rows: vec![pod_row("web-1"), pod_row("web-2")],
                status: TableStatus::Ready,
                selected_row: Some("web-2".to_string()),
            }
        }
    }
    let html = support::mount_html(table, || {});
    assert!(html.contains("table-row selected"), "got: {html}");
    assert!(html.contains("aria-selected=\"true\""), "got: {html}");
    assert_eq!(
        html.matches("table-row selected").count(),
        1,
        "exactly one row is marked: {html}"
    );
}

#[test]
fn a_dragged_width_stays_inside_the_pane_range() {
    assert_eq!(clamp_pane_width(10.0), PANE_MIN_WIDTH);
    assert_eq!(clamp_pane_width(4096.0), PANE_MAX_WIDTH);
}
