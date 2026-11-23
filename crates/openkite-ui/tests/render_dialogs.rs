//! The dialogs decide what reaches the apiserver, so the render proves the gates.

use dioxus::prelude::*;
use openkite_ui::actions_view::{
    confirmation_matches, DeleteDialog, DrainDialog, DrainRow, DrainToggles, Propagation, SkipKind,
};

#[derive(Props, Clone, PartialEq)]
struct DeleteProps {
    open: bool,
    typed: String,
    propagation: Propagation,
}

#[allow(non_snake_case)]
fn DeleteHarness(props: DeleteProps) -> Element {
    rsx! {
        DeleteDialog {
            open: props.open,
            kind: "Deployment",
            name: "api",
            typed: props.typed.clone(),
            propagation: props.propagation,
            on_typed: move |_| {},
            on_propagation: move |_| {},
            on_confirm: move |_| {},
            on_cancel: move |_| {},
        }
    }
}

fn delete_dialog(typed: &str, propagation: Propagation) -> String {
    let mut dom = VirtualDom::new_with_props(
        DeleteHarness,
        DeleteProps {
            open: true,
            typed: typed.to_owned(),
            propagation,
        },
    );
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn a_mistyped_name_leaves_the_delete_button_disabled() {
    let html = delete_dialog("ap", Propagation::Background);
    assert!(html.contains("data-action=\"confirm-delete\""), "{html}");
    assert!(
        html.contains("disabled"),
        "the button is inert until the name matches"
    );
    assert!(!confirmation_matches("api", "ap"));
}

#[test]
fn the_exact_name_enables_the_delete_button() {
    let html = delete_dialog("api", Propagation::Background);
    let confirm = html
        .split("data-action=\"confirm-delete\"")
        .nth(1)
        .expect("the confirm button is rendered");
    assert!(
        !confirm.contains("disabled"),
        "a matching name enables it: {confirm}"
    );
}

#[test]
fn the_dialog_names_the_workload_and_offers_all_three_policies() {
    let html = delete_dialog("api", Propagation::Foreground);
    assert!(html.contains("Delete Deployment api?"), "{html}");
    for choice in Propagation::ALL {
        assert!(html.contains(choice.id()), "{} is offered", choice.id());
        assert!(
            html.contains(choice.explanation()),
            "{} is explained",
            choice.id()
        );
    }
    let checked = html
        .split("value=\"foreground\"")
        .nth(1)
        .expect("the foreground radio is rendered");
    assert!(
        checked
            .split('>')
            .next()
            .unwrap_or_default()
            .contains("checked")
            || checked.contains("checked"),
        "the selected policy is the one carried"
    );
}

#[derive(Props, Clone, PartialEq)]
struct DrainProps {
    rows: Vec<DrainRow>,
    toggles: DrainToggles,
}

#[allow(non_snake_case)]
fn DrainHarness(props: DrainProps) -> Element {
    rsx! {
        DrainDialog {
            open: true,
            node: "worker-1",
            rows: props.rows.clone(),
            toggles: props.toggles,
            on_toggles: move |_| {},
            on_confirm: move |_| {},
            on_cancel: move |_| {},
        }
    }
}

fn drain_dialog(rows: Vec<DrainRow>, toggles: DrainToggles) -> String {
    let mut dom = VirtualDom::new_with_props(DrainHarness, DrainProps { rows, toggles });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn the_drain_preview_says_which_pods_stay_and_why() {
    let html = drain_dialog(
        vec![
            DrainRow::evicting("prod/api-7d9"),
            DrainRow::skipping("kube-system/fluentbit-1", SkipKind::DaemonSet),
        ],
        DrainToggles::default(),
    );
    assert!(html.contains("prod/api-7d9"), "{html}");
    assert!(html.contains(">protected<"), "the skip is badged: {html}");
    assert!(
        html.contains("daemonset"),
        "the badge names the owner: {html}"
    );
    assert!(
        html.contains("a DaemonSet runs one on every node"),
        "{html}"
    );
    assert!(
        html.contains("1 pod(s) are evicted"),
        "the count is stated: {html}"
    );
}

#[test]
fn a_drain_with_nothing_to_evict_cannot_be_confirmed() {
    let html = drain_dialog(
        vec![DrainRow::skipping(
            "kube-system/fluentbit-1",
            SkipKind::DaemonSet,
        )],
        DrainToggles::default(),
    );
    let confirm = html
        .split("data-action=\"confirm-drain\"")
        .nth(1)
        .expect("the confirm button is rendered");
    assert!(confirm.contains("disabled"), "{confirm}");
}

#[test]
fn each_drain_option_is_checked_from_the_state_it_was_given() {
    let html = drain_dialog(
        vec![DrainRow::evicting("prod/api-7d9")],
        DrainToggles {
            ignore_daemonsets: false,
            delete_empty_dir_data: true,
            force: false,
        },
    );
    assert!(html.contains("name=\"delete-emptydir-data\""), "{html}");
    let empty_dir = html
        .split("name=\"delete-emptydir-data\"")
        .nth(1)
        .expect("the emptyDir option is rendered");
    assert!(empty_dir.contains("checked"), "the given state wins");
}

#[test]
fn a_closed_dialog_renders_nothing() {
    let mut dom = VirtualDom::new_with_props(
        DeleteHarness,
        DeleteProps {
            open: false,
            typed: String::new(),
            propagation: Propagation::default(),
        },
    );
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    assert!(!html.contains("data-action=\"confirm-delete\""), "{html}");
}
