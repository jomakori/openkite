//! Render proof for the inspector: tab set, quick actions and panel markup.

use dioxus::prelude::*;
use openkite_ui::inspector::{
    action_bar, tabs_for, Container, Detail, Event, Gauge, InspectorData, InspectorState, Manifest,
    ResourceLink, SchemaNode, Tab,
};
use openkite_ui::inspector_view::Inspector;
use openkite_ui::nav::CATALOGUE;
use openkite_ui::table::columns;

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    kind: String,
    kind_label: String,
    state: InspectorState,
    data: InspectorData,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_close = use_callback(|_: ()| {});
    let on_tab = use_callback(|_: Tab| {});
    let on_action = use_callback(|_: String| {});
    let on_goto = use_callback(|_: String| {});
    rsx! {
        Inspector {
            name: "api-7d9c4f8b6-2xqzt".to_string(),
            kind_label: props.kind_label.clone(),
            kind: props.kind.clone(),
            namespace: Some("prod".to_string()),
            state: props.state.clone(),
            data: props.data.clone(),
            on_close,
            on_tab,
            on_action,
            on_goto,
        }
    }
}

fn render(kind: &str, state: InspectorState, data: InspectorData) -> String {
    let mut dom = VirtualDom::new_with_props(
        Harness,
        HarnessProps {
            kind: kind.to_string(),
            kind_label: "Pod".to_string(),
            state,
            data,
        },
    );
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn open_on(kind: &str, tab: Tab) -> InspectorState {
    let mut state = InspectorState::default();
    state.open("pod/prod/api-7d9", kind);
    assert!(state.show_tab(kind, tab), "{kind} has no {tab:?} tab");
    state
}

fn sample_detail() -> Detail {
    Detail {
        props: vec![
            ("Status".to_string(), "Running".to_string()),
            ("Node".to_string(), "node-a".to_string()),
        ],
        labels: vec![("app".to_string(), "api".to_string())],
        annotations: vec![("checksum/config".to_string(), "9f2c".to_string())],
        conditions: vec![
            ("Ready".to_string(), true),
            ("ContainersReady".to_string(), false),
        ],
    }
}

#[test]
fn tab_strip_matches_the_rule_for_every_kind() {
    let state = open_on("pods", Tab::Overview);
    for kind in CATALOGUE.iter() {
        let html = render(kind.key, state.clone(), InspectorData::default());
        let ids: Vec<&str> = html
            .split("data-tab=\"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .collect();
        let expected: Vec<&str> = tabs_for(kind.key).into_iter().map(Tab::id).collect();
        assert_eq!(ids, expected, "tab strip drifted for {}", kind.key);
    }
}

#[test]
fn exactly_one_tab_is_selected() {
    let html = render(
        "pods",
        open_on("pods", Tab::Metrics),
        InspectorData::default(),
    );
    assert_eq!(html.matches("aria-selected=\"true\"").count(), 1, "{html}");
    assert!(html.contains("id=\"insp-tab-metrics\""), "{html}");
    assert!(
        html.contains("aria-controls=\"insp-panel-metrics\""),
        "{html}"
    );
}

#[test]
fn closed_inspector_is_hidden_and_off_the_scrim() {
    let html = render("pods", InspectorState::default(), InspectorData::default());
    assert!(html.contains("data-open=\"false\""), "{html}");
    assert!(html.contains("aria-hidden=\"true\""), "{html}");
    assert!(html.contains("data-part=\"scrim\""), "{html}");
}

#[test]
fn yaml_is_pending_until_the_manifest_is_fetched() {
    let pending = render("pods", open_on("pods", Tab::Yaml), InspectorData::default());
    assert!(pending.contains("data-yaml=\"pending\""), "{pending}");
    assert!(pending.contains("Fetching manifest"), "{pending}");
    assert!(!pending.contains("data-yaml=\"loaded\""), "{pending}");

    let data = InspectorData {
        manifest: Some(Manifest::new("kind: Pod\nmetadata:\n  name: api\n")),
        ..InspectorData::default()
    };
    let loaded = render("pods", open_on("pods", Tab::Yaml), data);
    assert!(loaded.contains("data-yaml=\"loaded\""), "{loaded}");
    assert!(loaded.contains("kind: Pod"), "{loaded}");
    assert!(loaded.contains("3 lines"), "{loaded}");
}

#[test]
fn list_columns_never_carry_a_manifest() {
    for kind in CATALOGUE.iter() {
        let keys: Vec<&str> = columns(kind.key).iter().map(|c| c.id).collect();
        assert!(
            !keys.iter().any(|k| *k == "yaml" || *k == "manifest"),
            "{} carries manifest data in the list: {keys:?}",
            kind.key
        );
    }
}

#[test]
fn quick_actions_match_the_bar_split() {
    let bar = action_bar("pods");
    let html = render(
        "pods",
        open_on("pods", Tab::Overview),
        InspectorData::default(),
    );
    for action in bar.left.iter().chain(bar.right.iter()) {
        assert!(
            html.contains(&format!("data-act=\"{}\"", action.id)),
            "missing {} in the bar: {html}",
            action.id
        );
    }
    assert!(html.contains("data-menu=\"actions\""), "{html}");
    assert!(
        !html.contains("menu-wrap open"),
        "menu starts closed: {html}"
    );
    assert!(html.contains("role=\"menuitem\""), "{html}");
    assert_eq!(
        html.matches("menu-item danger").count(),
        1,
        "delete is the only dangerous tucked action: {html}"
    );
}

#[test]
fn overview_renders_props_conditions_and_labels() {
    let data = InspectorData::with_detail(sample_detail());
    let html = render("pods", open_on("pods", Tab::Overview), data);
    assert!(html.contains("data-panel=\"overview\""), "{html}");
    assert!(html.contains("<dt>Status</dt>"), "{html}");
    assert!(html.contains("class=\"cond ok\""), "{html}");
    assert!(html.contains("class=\"cond warn\""), "{html}");
    assert!(html.contains("class=\"label-chip\""), "{html}");
    assert!(html.contains("app"), "{html}");
}

#[test]
fn containers_panel_renders_one_card_per_container() {
    let data = InspectorData {
        containers: vec![
            Container {
                name: "api".to_string(),
                image: "ghcr.io/jomakori/openkite:latest".to_string(),
                ports: "8080/TCP".to_string(),
                restarts: 2,
                requests: "100m / 128Mi".to_string(),
                limits: "500m / 512Mi".to_string(),
                liveness: "/healthz".to_string(),
                mounts: "/var/run/secrets".to_string(),
            },
            Container {
                name: "sidecar".to_string(),
                ..Container::default()
            },
        ],
        ..InspectorData::default()
    };
    let html = render("pods", open_on("pods", Tab::Containers), data);
    assert_eq!(html.matches("class=\"ctr-card\"").count(), 2, "{html}");
    assert!(html.contains("2 restarts"), "{html}");
    assert!(html.contains("/healthz"), "{html}");
}

#[test]
fn metrics_panel_uses_gauges() {
    let data = InspectorData {
        gauges: vec![
            Gauge {
                label: "CPU",
                value: "180m / 500m".to_string(),
                pct: 36,
            },
            Gauge {
                label: "Memory",
                value: "420Mi / 512Mi".to_string(),
                pct: 140,
            },
        ],
        ..InspectorData::default()
    };
    let html = render("pods", open_on("pods", Tab::Metrics), data);
    assert_eq!(html.matches("class=\"meter\"").count(), 2, "{html}");
    assert!(html.contains("width:36%"), "{html}");
    assert!(html.contains("width:100%"), "clamped: {html}");
}

#[test]
fn resource_links_are_followable() {
    let data = InspectorData {
        resources: vec![ResourceLink {
            id: "deployment/prod/api".to_string(),
            label: "Deployment/api".to_string(),
            status: Some("3/3".to_string()),
        }],
        ..InspectorData::default()
    };
    let html = render("crds", open_on("crds", Tab::Resources), data);
    assert!(html.contains("data-goto=\"deployment/prod/api\""), "{html}");
    assert!(html.contains("Deployment/api"), "{html}");
}

#[test]
fn events_panel_marks_warnings() {
    let data = InspectorData {
        events: vec![
            Event {
                kind: "Normal",
                reason: "Scheduled".to_string(),
                message: "Assigned to node-a".to_string(),
                age: "2m".to_string(),
                count: 1,
                span: String::new(),
            },
            Event {
                kind: "Warning",
                reason: "BackOff".to_string(),
                message: "Back-off restarting failed container".to_string(),
                age: "40s".to_string(),
                count: 12,
                span: "2m".to_string(),
            },
        ],
        ..InspectorData::default()
    };
    let html = render("pods", open_on("pods", Tab::Events), data);
    assert_eq!(html.matches("class=\"ev warn\"").count(), 1, "{html}");
    assert!(
        html.contains("Back-off restarting failed container"),
        "{html}"
    );
}

#[test]
fn schema_panel_falls_back_to_one_field() {
    let html = render(
        "crds",
        open_on("crds", Tab::Schema),
        InspectorData::default(),
    );
    assert_eq!(html.matches("class=\"schema-row\"").count(), 1, "{html}");
    assert!(html.contains("No schema published"), "{html}");
}

#[test]
fn nested_schema_rows_indent_by_depth() {
    let data = InspectorData {
        schema: vec![SchemaNode::new("spec", "object", "Desired state")
            .with_children(vec![SchemaNode::new("replicas", "integer", "Pod count")])],
        ..InspectorData::default()
    };
    let html = render("crds", open_on("crds", Tab::Schema), data);
    assert!(html.contains("data-depth=\"0\""), "{html}");
    assert!(html.contains("data-depth=\"1\""), "{html}");
    assert!(html.contains("Pod count"), "{html}");
}
