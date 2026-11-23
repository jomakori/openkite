//! Render proof that an identity's permissions decide what the page shows.

use dioxus::prelude::*;
use openkite_ui::access::{Permissions, Verb};
use openkite_ui::table::{Row, TableState};
use openkite_ui::table_view::ResourceTable;

fn pod(name: &str, namespace: &str) -> Row {
    Row::new(name, name)
        .with_namespace(namespace)
        .with_cell("status", "Running")
}

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    permissions: Permissions,
    selected: Vec<String>,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_open = use_callback(|_: String| {});
    let on_state = use_callback(|_: TableState| {});
    let on_scroll = use_callback(|_: f64| {});
    let state = TableState {
        selected: props.selected.iter().cloned().collect(),
        ..TableState::default()
    };
    rsx! {
        ResourceTable {
            kind: "pods".to_string(),
            label: "Pods".to_string(),
            rows: vec![pod("api-7d9", "default")],
            state: state.clone(),
            namespaces: vec![("default".to_string(), 1usize)],
            on_open,
            on_state,
            scroll_top: 0.0,
            on_scroll,
            permissions: props.permissions.clone(),
        }
    }
}

fn render(permissions: Permissions) -> String {
    render_selected(permissions, vec!["api-7d9".to_string()])
}

fn render_selected(permissions: Permissions, selected: Vec<String>) -> String {
    let mut dom = VirtualDom::new_with_props(
        Harness,
        HarnessProps {
            permissions,
            selected,
        },
    );
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn a_viewer_sees_the_table_without_write_controls() {
    let mut permissions = Permissions::deny_all("viewer cannot write");
    permissions.set(Verb::Get, true, None);
    permissions.set(Verb::List, true, None);

    let html = render(permissions);

    assert!(html.contains("api-7d9"), "the row is shown: {html}");
    assert!(
        !html.contains("btn-danger"),
        "no destructive control is rendered: {html}"
    );
    assert!(
        !html.contains(">Delete<"),
        "the delete action is absent, not disabled: {html}"
    );
}

#[test]
fn an_editor_sees_the_delete_action() {
    let html = render(Permissions::allow_all());

    assert!(html.contains("api-7d9"));
    assert!(
        html.contains(">Delete<"),
        "an identity that may delete gets the control: {html}"
    );
}

#[test]
fn a_denied_list_shows_the_reason_instead_of_the_table() {
    let mut permissions = Permissions::deny_all("pods is forbidden: User cannot list resource");
    permissions.set(Verb::Get, false, Some("pods is forbidden"));

    let html = render(permissions);

    assert!(
        html.contains("You cannot read Pods"),
        "the panel names the resource: {html}"
    );
    assert!(
        html.contains("User cannot list resource"),
        "the panel repeats the apiserver's reason: {html}"
    );
    assert!(
        !html.contains("api-7d9"),
        "no stale rows are shown behind the denial: {html}"
    );
}
