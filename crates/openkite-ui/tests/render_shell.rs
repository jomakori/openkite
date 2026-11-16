//! Render proof for the shell: the markup a browser receives, not just a compile.

use dioxus::prelude::*;
use openkite_ui::nav::{coverage_gaps, CATALOGUE};
use openkite_ui::{AppShell, AppShellProps, Connection, Counts};

fn render(active: &str, counts: Counts) -> String {
    let mut dom = VirtualDom::new_with_props(
        AppShell,
        AppShellProps {
            counts,
            active_path: active.to_string(),
            cluster: "prod-east".to_string(),
            connection: Connection::Live,
            children: rsx! { div { class: "page-body-stub", "page" } },
        },
    );
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn shell_renders_every_surface_anchor() {
    let html = render(
        "/workloads/pods",
        Counts::new().with("pods", 12).with("secrets", 3),
    );

    for anchor in [
        "class=\"app\"",
        "class=\"sidebar\"",
        "class=\"brand\"",
        "class=\"brand-mark\"",
        "class=\"brand-word\"",
        "class=\"cluster-btn\"",
        "class=\"nav\"",
        "class=\"nav-section\"",
        "class=\"nav-title\"",
        "class=\"nav-item",
        "class=\"nav-badge\"",
        "class=\"topbar\"",
        "class=\"breadcrumbs\"",
        "class=\"crumb-cluster\"",
        "class=\"crumb-trail\"",
        "class=\"dot ok\"",
        "class=\"main\"",
    ] {
        assert!(html.contains(anchor), "shell is missing {anchor}");
    }
}

#[test]
fn shell_renders_one_nav_row_per_catalogue_kind() {
    let html = render("/overview", Counts::new());
    for kind in CATALOGUE {
        assert!(
            html.contains(&format!(
                "class=\"nav-item\" href=\"/{}/{}\"",
                kind.group.to_lowercase().replace(' ', "-"),
                kind.key
            )),
            "missing nav row for {}",
            kind.key
        );
    }
}

#[test]
fn badges_come_from_counts_not_from_rows() {
    let counts = Counts::new().with("pods", 12).with("secrets", 3);
    let html = render("/workloads/pods", counts);
    assert!(html.contains(">12<"), "pod badge missing");
    assert!(html.contains(">3<"), "secret badge missing");
    assert!(
        html.contains(">0<"),
        "kinds without a frame must render zero"
    );
}

#[test]
fn counts_frame_alone_keeps_nav_complete() {
    // A shell fed only counts must still show every kind and route.
    let html = render("/overview", Counts::new().with("pods", 7));
    assert!(coverage_gaps(
        CATALOGUE,
        &openkite_ui::build_nav(CATALOGUE, openkite_ui::VIEWS)
    )
    .is_empty());
    for kind in CATALOGUE {
        assert!(html.contains(kind.label), "missing label {}", kind.label);
    }
}

#[test]
fn active_route_is_marked_and_breadcrumbs_follow_it() {
    let html = render("/workloads/pods", Counts::new());
    assert!(
        html.contains("aria-current=\"page\""),
        "active row not marked"
    );
    assert!(html.contains(">Pods<"), "breadcrumb leaf missing");
    assert!(html.contains(">Workloads<"), "breadcrumb section missing");
}
