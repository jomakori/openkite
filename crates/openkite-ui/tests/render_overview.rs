//! Board layout rules and the rendered overview markup.

use dioxus::prelude::*;
use openkite_ui::overview::{
    ArgoRow, BoardAction, CapacityGauge, EventRow, Interest, Lane, NodeRow, OverviewData,
    OverviewLayout, Size, WidgetId,
};
use openkite_ui::overview_view::Overview;

fn layout() -> OverviewLayout {
    OverviewLayout::default()
}

fn data() -> OverviewData {
    let pods = vec![
        openkite_ui::Row::new("pod/prod/api-1", "api-1")
            .with_namespace("prod")
            .with_cell("status", "Running")
            .with_cell("restarts", "0"),
        openkite_ui::Row::new("pod/prod/api-2", "api-2")
            .with_namespace("prod")
            .with_cell("status", "CrashLoopBackOff")
            .with_cell("restarts", "7"),
        openkite_ui::Row::new("pod/prod/w-1", "w-1")
            .with_namespace("prod")
            .with_cell("status", "Pending")
            .with_cell("restarts", "0"),
    ];
    OverviewData::from_counts("prod-eu", &counts()).with_pods(pods, 3)
}

fn counts() -> openkite_ui::Counts {
    openkite_ui::Counts::new()
        .with("pods", 3)
        .with("cpu_pct", 42)
        .with("mem_pct", 61)
}

fn rich_data() -> OverviewData {
    let mut d = data();
    d.nodes = vec![
        NodeRow {
            id: "node/n1".to_string(),
            name: "n1".to_string(),
            cpu_pct: 30,
            mem_pct: 55,
            note: "Ready".to_string(),
            degraded: false,
        },
        NodeRow {
            id: "node/n2".to_string(),
            name: "n2".to_string(),
            cpu_pct: 88,
            mem_pct: 91,
            note: "MemoryPressure".to_string(),
            degraded: true,
        },
    ];
    d.argo = vec![
        ArgoRow {
            id: "argo/web".to_string(),
            name: "web".to_string(),
            sync: "Synced".to_string(),
            health: "Healthy".to_string(),
        },
        ArgoRow {
            id: "argo/api".to_string(),
            name: "api".to_string(),
            sync: "OutOfSync".to_string(),
            health: "Degraded".to_string(),
        },
    ];
    d.events = vec![EventRow {
        target: Some("pod/prod/api-2".to_string()),
        kind: "Warning".to_string(),
        reason: "BackOff".to_string(),
        object: "pod/api-2".to_string(),
        message: "Back-off restarting failed container".to_string(),
        age: "2m".to_string(),
    }];
    d.namespaces = vec![("prod".to_string(), 12), ("kube-system".to_string(), 4)];
    d.capacity = vec![CapacityGauge {
        label: "CPU requests".to_string(),
        value: "18.4 / 32 cores".to_string(),
        pct: 57,
        tone: "warn",
    }];
    d
}

/// Handlers must be built inside the render runtime, so props stay handler-free here.
#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    layout: OverviewLayout,
    data: OverviewData,
    namespaces: Vec<String>,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_action = use_callback(|_: BoardAction| {});
    rsx! {
        Overview {
            layout: props.layout.clone(),
            data: props.data.clone(),
            namespaces: props.namespaces.clone(),
            on_action,
        }
    }
}

fn render(layout: OverviewLayout, data: OverviewData) -> String {
    let mut dom = VirtualDom::new_with_props(
        Harness,
        HarnessProps {
            layout,
            data,
            namespaces: vec![],
        },
    );
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn render_with_namespaces(layout: OverviewLayout, data: OverviewData, ns: Vec<String>) -> String {
    let mut dom = VirtualDom::new_with_props(
        Harness,
        HarnessProps {
            layout,
            data,
            namespaces: ns,
        },
    );
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn every_widget_is_rendered_with_its_grip() {
    let html = render(layout(), rich_data());
    for id in WidgetId::ALL {
        assert!(
            html.contains(&format!("data-w=\"{}\"", id.id())),
            "missing widget {}: {html}",
            id.id()
        );
        assert!(
            html.contains(&format!("data-grip=\"{}\"", id.id())),
            "missing grip for {}",
            id.id()
        );
    }
}

#[test]
fn full_width_widgets_stand_alone_and_halves_share_a_band() {
    let html = render(layout(), rich_data());
    assert!(html.contains("ov-w full"), "{html}");
    assert!(html.contains("ov-w half"), "{html}");
    assert!(html.contains("data-lane=\"L\""), "{html}");
    assert!(html.contains("data-lane=\"R\""), "{html}");
    assert!(html.contains("data-band=\"true\""), "{html}");
}

#[test]
fn hiding_a_widget_drops_it_and_its_interest() {
    let mut layout = layout();
    layout.hide(WidgetId::Nodes);
    let html = render(layout.clone(), rich_data());
    assert!(!html.contains("data-w=\"nodes\""), "{html}");
    // The attention widget still reads nodes, so the interest only goes once it is hidden too.
    assert!(layout.interests().contains(&Interest::Nodes));
    layout.hide(WidgetId::Attention);
    assert!(
        !layout.interests().contains(&Interest::Nodes),
        "still asks for nodes"
    );
    assert!(
        layout.interests().contains(&Interest::Pods),
        "pods still needed"
    );
}

#[test]
fn hiding_every_row_widget_leaves_only_capacity() {
    let mut layout = layout();
    for id in [
        WidgetId::Attention,
        WidgetId::Usage,
        WidgetId::Problems,
        WidgetId::Restarts,
        WidgetId::Nodes,
        WidgetId::Health,
        WidgetId::Argo,
        WidgetId::Events,
    ] {
        layout.hide(id);
    }
    assert_eq!(layout.interests(), vec![]);
    assert!(layout.visible() == vec![WidgetId::Capacity]);
}

#[test]
fn sizes_default_per_widget_and_toggle_to_the_other_width() {
    let mut layout = layout();
    assert_eq!(layout.size(WidgetId::Attention), Size::Full);
    assert_eq!(layout.size(WidgetId::Problems), Size::Half);
    layout.toggle_size(WidgetId::Problems);
    assert_eq!(layout.size(WidgetId::Problems), Size::Full);
    assert_eq!(
        layout.lanes.get(&WidgetId::Problems),
        None,
        "full widgets have no lane"
    );
}

#[test]
fn moving_a_widget_reorders_the_saved_order() {
    let mut layout = layout();
    let before = layout.order.clone();
    assert!(layout.move_by(WidgetId::Usage, -1));
    assert_eq!(layout.order[0], WidgetId::Usage, "{:?}", layout.order);
    assert_eq!(layout.order[1], before[0]);
}

#[test]
fn moving_past_the_edge_is_refused() {
    let mut layout = layout();
    assert!(!layout.move_by(WidgetId::Attention, -1));
    assert!(!layout.move_by(WidgetId::Events, 1));
}

#[test]
fn swapping_lanes_moves_a_half_widget_to_the_other_column() {
    let mut layout = layout();
    layout.swap_lane(WidgetId::Problems);
    assert_eq!(layout.lanes.get(&WidgetId::Problems), Some(&Lane::Right));
    layout.swap_lane(WidgetId::Problems);
    assert_eq!(layout.lanes.get(&WidgetId::Problems), Some(&Lane::Left));
}

#[test]
fn a_saved_layout_round_trips() {
    let mut layout = layout();
    layout.hide(WidgetId::Events);
    layout.toggle_size(WidgetId::Problems);
    layout.swap_lane(WidgetId::Restarts);
    layout.threshold = 5;
    layout.move_by(WidgetId::Capacity, -1);

    let raw = layout.encode();
    let back = OverviewLayout::decode(&raw);
    assert_eq!(back.threshold, 5);
    assert_eq!(back.hidden, layout.hidden);
    assert_eq!(back.order, layout.order);
    assert_eq!(back.size(WidgetId::Problems), Size::Full);
    assert_eq!(
        back.lanes.get(&WidgetId::Restarts),
        layout.lanes.get(&WidgetId::Restarts)
    );
}

#[test]
fn a_damaged_saved_layout_falls_back_to_defaults() {
    let layout = OverviewLayout::decode("not-a-number;order=nope,bogus;size=x:zzz");
    assert_eq!(layout.order, WidgetId::ALL.to_vec());
    assert_eq!(layout.threshold, 3);
    assert!(layout.hidden.is_empty());
}

#[test]
fn reset_keeps_edit_mode_but_restores_the_defaults() {
    let mut layout = layout();
    layout.hide(WidgetId::Nodes);
    layout.editing = true;
    layout.reset();
    assert!(layout.editing);
    assert!(layout.hidden.is_empty());
    assert_eq!(layout.order, WidgetId::ALL.to_vec());
    assert_eq!(layout.threshold, 3);
}

#[test]
fn customize_toggles_the_edit_bar_and_the_tray() {
    let mut layout = layout();
    layout.hide(WidgetId::Argo);
    let idle = render(layout.clone(), rich_data());
    assert!(!idle.contains("ov-edit-bar"), "{idle}");
    assert!(!idle.contains("ov-tray"), "{idle}");

    layout.editing = true;
    let editing = render(layout.clone(), rich_data());
    assert!(editing.contains("ov-edit-bar"), "{editing}");
    assert!(editing.contains("data-ovw=\"hide\""), "{editing}");
    assert!(editing.contains("data-ovw=\"add\""), "{editing}");
    assert!(editing.contains("data-id=\"argo\""), "{editing}");
}

#[test]
fn problems_widget_lists_failing_pods_and_links_into_the_inspector() {
    let html = render(layout(), data());
    assert!(html.contains("CrashLoopBackOff"), "{html}");
    assert!(html.contains("data-goto=\"pod/prod/api-2\""), "{html}");
    assert!(
        !html.contains("data-goto=\"pod/prod/api-1\""),
        "running pod listed"
    );
}

#[test]
fn restarts_widget_respects_the_threshold() {
    let html = render(layout(), data());
    assert!(html.contains("api-2"), "{html}");
    assert!(html.contains("7×"), "{html}");
    // w-1 is pending, so the problems widget lists it once; the restarts widget must not.
    assert_eq!(
        html.matches("data-goto=\"pod/prod/w-1\"").count(),
        1,
        "{html}"
    );
}

#[test]
fn an_empty_board_says_so_instead_of_rendering_blank_cards() {
    let d = OverviewData {
        cluster: "prod-eu".to_string(),
        ..OverviewData::default()
    };
    let html = render(layout(), d);
    assert!(html.contains("No pending or failing pods"), "{html}");
    assert!(html.contains("No events yet"), "{html}");
    assert!(
        html.contains("No pods in the selected namespaces"),
        "{html}"
    );
}

#[test]
fn the_namespace_note_appears_only_with_a_filter() {
    let plain = render(layout(), rich_data());
    assert!(!plain.contains("ov-ns-note"), "{plain}");

    let filtered = render_with_namespaces(layout(), rich_data(), vec!["prod".to_string()]);
    assert!(filtered.contains("ov-ns-note"), "{filtered}");
    assert!(filtered.contains("Clear namespace filter"), "{filtered}");
}

#[test]
fn the_everywhere_widgets_cover_the_data_they_claim() {
    let html = render(layout(), rich_data());
    assert!(html.contains("BackOff"), "{html}");
    assert!(html.contains("OutOfSync"), "{html}");
    assert!(html.contains("MemoryPressure"), "{html}");
    assert!(html.contains("18.4 / 32 cores"), "{html}");
    assert!(html.contains("Failing pods"), "{html}");
}
