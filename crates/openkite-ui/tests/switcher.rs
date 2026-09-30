//! The cluster switcher surface (OKT-136 migration).
//!
//! The switcher moved out of `openkite-desktop` into the shared crate. The
//! desktop's `tests/switcher.rs` covered the pure filter/cursor helpers and
//! `tests/switcher_mount.rs` the overlay states; both live here now, plus the
//! case the migration exists for: a host without the cluster registry gets
//! explicit chrome instead of a list it cannot connect.
//!
//! The connect itself is the host's half and is not mounted here — the panel
//! hands the chosen context to `on_switch`.

mod support;

use std::sync::Mutex;

use dioxus::prelude::*;
use openkite_api::capability::Capabilities;
use openkite_ui::components::switcher::{
    advance_index, filter_contexts, ClusterSwitcher, SWITCHER_ERROR, SWITCHER_OPEN, SWITCHER_QUERY,
};
use openkite_ui::runtime::{set_context, set_contexts, set_gateway, set_published_capabilities};

// The capability slots and the context globals are process-global; hold this
// guard in every test that reads or writes them so parallel test threads
// cannot see each other's host.
static STATE: Mutex<()> = Mutex::new(());

fn state_lock() -> std::sync::MutexGuard<'static, ()> {
    STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Publish a descriptor and clear the context list for one test.
fn host(caps: Capabilities) {
    set_gateway(None);
    set_published_capabilities(Some(caps));
}

fn names() -> Vec<String> {
    ["dev", "staging-eu", "prod", "prod-us", "sandbox"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn switcher() -> Element {
    rsx! {
        ClusterSwitcher { on_switch: |_: String| {} }
        div { "shell" }
    }
}

#[test]
fn blank_query_returns_all_in_kubeconfig_order() {
    let got = filter_contexts(&names(), "");
    assert_eq!(got, names(), "blank query must preserve kubeconfig order");
}

#[test]
fn whitespace_only_query_returns_all() {
    let got = filter_contexts(&names(), "   \t ");
    assert_eq!(got, names());
}

#[test]
fn filter_is_case_insensitive_substring() {
    let got = filter_contexts(&names(), "PROD");
    assert_eq!(got, vec!["prod".to_string(), "prod-us".to_string()]);
}

#[test]
fn filter_orders_by_match_position() {
    let got = filter_contexts(&names(), "prod");
    assert_eq!(got, vec!["prod".to_string(), "prod-us".to_string()]);
}

#[test]
fn no_hits_yields_empty() {
    assert!(filter_contexts(&names(), "zzz").is_empty());
}

#[test]
fn advance_wraps_both_directions() {
    assert_eq!(advance_index(Some(0), 5, 1), Some(1));
    assert_eq!(advance_index(Some(0), 5, -1), Some(4));
    assert_eq!(advance_index(Some(4), 5, 1), Some(0));
}

#[test]
fn advance_clamps_stale_selection() {
    assert_eq!(advance_index(Some(9), 2, 1), Some(0));
    assert_eq!(advance_index(Some(9), 2, -1), Some(0));
}

#[test]
fn advance_empty_list_is_none() {
    assert_eq!(advance_index(Some(0), 0, 1), None);
    assert_eq!(advance_index(None, 0, -1), None);
}

#[test]
fn advance_none_selection_starts_at_zero() {
    assert_eq!(advance_index(None, 3, 1), Some(1));
    assert_eq!(advance_index(None, 3, 0), Some(0));
}

/// Closed is the default: the overlay contributes nothing to the tree.
#[test]
fn closed_switcher_renders_nothing() {
    let _guard = state_lock();
    host(Capabilities::in_process());

    let html = support::mount_html(switcher, || {});
    assert!(html.contains("shell"), "got: {html}");
    assert!(!html.contains("switcher"), "got: {html}");
}

#[test]
fn open_with_no_contexts_shows_empty_state() {
    let _guard = state_lock();
    host(Capabilities::in_process());

    let html = support::mount_html(switcher, || {
        *SWITCHER_OPEN.write() = true;
    });
    assert!(html.contains("switcher-backdrop"), "got: {html}");
    assert!(html.contains("no matching context"), "got: {html}");
}

#[test]
fn open_lists_contexts_and_marks_connected() {
    let _guard = state_lock();
    host(Capabilities::in_process());

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
    let _guard = state_lock();
    host(Capabilities::in_process());

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
    let _guard = state_lock();
    host(Capabilities::in_process());

    let html = support::mount_html(switcher, || {
        set_contexts(vec!["dev".into()]);
        *SWITCHER_ERROR.write() = Some("connect failed".into());
        *SWITCHER_OPEN.write() = true;
    });
    assert!(html.contains("switcher-error"), "got: {html}");
    assert!(html.contains("connect failed"), "got: {html}");
}

/// A host that cannot switch clusters says so: the overlay keeps its frame and
/// explains itself rather than listing contexts that would connect to nothing.
#[test]
fn open_without_the_capability_renders_unsupported_chrome() {
    let _guard = state_lock();
    host(Capabilities::server_side());

    let html = support::mount_html(switcher, || {
        set_contexts(vec!["dev".into(), "prod".into()]);
        *SWITCHER_OPEN.write() = true;
    });
    assert!(html.contains("switcher-backdrop"), "got: {html}");
    assert!(
        html.contains("Cluster switching needs the desktop host."),
        "got: {html}"
    );
    assert!(!html.contains("switcher-row"), "got: {html}");
    assert!(!html.contains("dev"), "got: {html}");
}
