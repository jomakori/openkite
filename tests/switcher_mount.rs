//! Headless mounts of the cluster switcher overlay (coverage roadmap B5).
//! tests/switcher.rs pins the pure filter/cursor helpers; these run the
//! `ClusterSwitcher`/`SwitcherPanel` render bodies across the open/closed,
//! empty-list, filtered, error, and connected-dot states.

mod support;

use dioxus::prelude::*;
use openkite::runtime::{set_context, set_contexts};
use openkite::switcher::{ClusterSwitcher, SWITCHER_ERROR, SWITCHER_OPEN, SWITCHER_QUERY};

fn switcher() -> Element {
    rsx! {
        ClusterSwitcher {}
        div { "shell" }
    }
}

fn open_setup() -> impl FnOnce() {
    || *SWITCHER_OPEN.write() = true
}

#[test]
fn closed_switcher_renders_nothing() {
    let html = support::mount_html(switcher, || {});
    assert!(html.contains("shell"), "got: {html}");
    assert!(!html.contains("switcher"), "got: {html}");
}

#[test]
fn open_with_no_contexts_shows_empty_state() {
    let html = support::mount_html(switcher, open_setup());
    assert!(html.contains("switcher-backdrop"), "got: {html}");
    assert!(html.contains("no matching context"), "got: {html}");
}

#[test]
fn open_lists_contexts_and_marks_connected() {
    let html = support::mount_html(switcher, || {
        set_contexts(vec!["dev".into(), "prod".into(), "staging".into()]);
        set_context(Some("prod".into()));
        *SWITCHER_OPEN.write() = true;
    });
    assert!(html.contains("dev"), "got: {html}");
    assert!(html.contains("prod"), "got: {html}");
    assert!(html.contains("staging"), "got: {html}");
    assert!(html.contains("switcher-connected-dot"), "got: {html}");
}

#[test]
fn query_filters_the_context_list() {
    let html = support::mount_html(switcher, || {
        set_contexts(vec!["dev".into(), "prod".into(), "staging".into()]);
        *SWITCHER_QUERY.write() = "staging".to_string();
        *SWITCHER_OPEN.write() = true;
    });
    assert!(html.contains("staging"), "got: {html}");
    assert!(!html.contains("dev"), "got: {html}");
    assert!(!html.contains("prod"), "got: {html}");
}

#[test]
fn switch_error_is_rendered_under_the_field() {
    let html = support::mount_html(switcher, || {
        set_contexts(vec!["dev".into()]);
        *SWITCHER_ERROR.write() = Some("connect failed".into());
        *SWITCHER_OPEN.write() = true;
    });
    assert!(html.contains("switcher-error"), "got: {html}");
    assert!(html.contains("connect failed"), "got: {html}");
}
