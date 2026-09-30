//! Headless mounts of the app shell (OKT-154).
//!
//! The shell is props in, markup out, so these mount the crate's `AppShell`
//! with the states the hosts actually reach: a sidebar with entries and one
//! without, the status footer connected and disconnected, a plugin section
//! with its accent, and the host-only chrome a desktop-profile host supplies
//! against the note a browser-profile host gets instead. tests/shell.rs pins
//! the model these props come from; tests/capability_gate.rs pins the
//! predicates.

mod support;

use std::sync::Mutex;

use dioxus::prelude::*;
use openkite_api::capability::Capabilities;
use openkite_ui::components::shell::{AppShell, NamespaceChip};
use openkite_ui::plugin_api::RegistrationStore;
use openkite_ui::runtime::set_published_capabilities;
use openkite_ui::shell::{
    core_nav, status_bar_model, ShellNavItem, ShellSection, ShellState, StatusBarEntry,
};

// The host descriptor is process-global, exactly like tests/capability_gate.rs:
// hold this guard in every test that publishes a profile so parallel test
// threads cannot gate on each other's state.
static HOST_PROFILE: Mutex<()> = Mutex::new(());

/// Publish the desktop profile (native window chrome: menu bar, title bar,
/// in-window overlays) and hold the lock for the mount.
fn desktop_host() -> std::sync::MutexGuard<'static, ()> {
    let guard = HOST_PROFILE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    set_published_capabilities(Some(Capabilities::in_process()));
    guard
}

/// Publish the browser profile (no native window chrome) and hold the lock.
fn browser_host() -> std::sync::MutexGuard<'static, ()> {
    let guard = HOST_PROFILE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    set_published_capabilities(Some(Capabilities::server_side()));
    guard
}

/// The desktop's sidebar: the flat core navigation, then one plugin section
/// with an accent and the nav divider above it.
fn desktop_sections() -> Vec<ShellSection> {
    vec![
        ShellSection {
            label: String::new(),
            accent: None,
            divider: false,
            items: core_nav(true),
        },
        ShellSection {
            label: "argocd".into(),
            accent: Some("var(--accent)".into()),
            divider: true,
            items: vec![ShellNavItem {
                label: "Applications".into(),
                route: "/argocd/apps".into(),
                plugin: Some("argocd".into()),
            }],
        },
    ]
}

/// The status entries a connected cluster produces.
fn connected_entries() -> Vec<StatusBarEntry> {
    status_bar_model(
        &ShellState {
            cluster: Some("prod".into()),
            connected: true,
            ..ShellState::default()
        },
        &RegistrationStore::new(),
        "1.2.3",
    )
}

/// The status entries a host without a cluster produces.
fn disconnected_entries() -> Vec<StatusBarEntry> {
    status_bar_model(&ShellState::default(), &RegistrationStore::new(), "")
}

/// The shell as the desktop mounts it: entries, chips, status, the host's own
/// native chrome in the host slot, and the router callbacks the desktop owns.
fn desktop_shell() -> Element {
    rsx! {
        AppShell {
            sections: desktop_sections(),
            current_route: "/workloads",
            namespaces: vec![
                NamespaceChip { label: "default".into(), active: true, context: false },
                NamespaceChip { label: "team-a".into(), active: false, context: false },
            ],
            status: connected_entries(),
            on_navigate: Some(EventHandler::new(|route: String| {
                let _ = route;
            })),
            on_toggle_namespace: Some(EventHandler::new(|namespace: String| {
                let _ = namespace;
            })),
            chrome: rsx! { div { "data-native-chrome": "desktop", "host chrome" } },
            div { "data-outlet": "workloads", "route outlet" }
        }
    }
}

/// The shell as the browser host mounts it: the shared sidebar model, the
/// context chip it cannot toggle, no client router and no native chrome.
fn browser_shell() -> Element {
    rsx! {
        AppShell {
            sections: vec![
                ShellSection {
                    label: "Overview".into(),
                    accent: None,
                    divider: false,
                    items: vec![ShellNavItem {
                        label: "Cluster".into(),
                        route: "/cluster".into(),
                        plugin: None,
                    }],
                },
            ],
            current_route: String::new(),
            namespaces: vec![NamespaceChip { label: "in-cluster".into(), active: true, context: true }],
            status: disconnected_entries(),
            "data-surface": "app",
            div { "data-outlet": "overview", "route outlet" }
        }
    }
}

/// A shell with no navigation and no status entries at all.
fn empty_shell() -> Element {
    rsx! {
        AppShell {
            sections: Vec::<ShellSection>::new(),
            current_route: String::new(),
            namespaces: Vec::<NamespaceChip>::new(),
            status: Vec::<StatusBarEntry>::new(),
            div { "data-outlet": "empty", "route outlet" }
        }
    }
}

#[test]
fn sidebar_renders_its_entries_with_the_current_one_active() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    for (label, route) in [
        ("Cluster", "/cluster"),
        ("Workloads", "/workloads"),
        ("Logs", "/logs"),
        ("Terminal", "/terminal"),
        ("Config", "/config"),
    ] {
        assert!(
            html.contains(&format!(">{label}<")),
            "nav label {label}: {html}"
        );
        assert!(
            html.contains(&format!("href=\"{route}\"")),
            "href {route}: {html}"
        );
    }
    assert!(
        html.contains("class=\"nav-item active\""),
        "the current route must mark its entry: {html}"
    );
    assert_eq!(
        html.matches("class=\"nav-item active\"").count(),
        1,
        "exactly one entry is active: {html}"
    );
    assert!(html.contains("class=\"sidebar\""), "got: {html}");
    assert!(html.contains("class=\"brand\""), "got: {html}");
    assert!(
        html.contains("data-outlet=\"workloads\""),
        "the host's route outlet renders inside the frame: {html}"
    );
}

#[test]
fn a_plugin_section_renders_its_accent_divider_and_entries() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    assert!(html.contains("class=\"nav-divider\""), "got: {html}");
    assert!(html.contains("class=\"nav-section\""), "got: {html}");
    assert!(
        html.contains("class=\"nav-section-label\""),
        "the plugin section keeps its header: {html}"
    );
    assert!(html.contains(">argocd<"), "section label: {html}");
    assert!(
        html.contains("style=\"color: var(--accent)\""),
        "the contributor's accent reaches the label and its entries: {html}"
    );
    assert!(
        html.contains("class=\"nav-item plugin\""),
        "plugin entries carry the plugin class: {html}"
    );
    assert!(html.contains(">Applications<"), "entry label: {html}");
    assert!(
        html.contains("href=\"/argocd/apps\""),
        "plugin entry href: {html}"
    );
}

#[test]
fn sidebar_renders_without_entries() {
    let _host = desktop_host();
    let html = support::mount_html(empty_shell, || {});

    assert!(html.contains("class=\"sidebar\""), "got: {html}");
    assert!(html.contains("class=\"brand\""), "got: {html}");
    assert!(html.contains("class=\"nav\""), "got: {html}");
    assert!(
        !html.contains("nav-item"),
        "an empty model renders no entries: {html}"
    );
    assert!(
        !html.contains("nav-section"),
        "an empty model renders no sections: {html}"
    );
    assert!(
        html.contains("data-outlet=\"empty\""),
        "the frame still mounts the outlet: {html}"
    );
}

#[test]
fn top_bar_renders_namespace_chips_and_marks_the_selected_ones() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    assert!(html.contains("class=\"topbar\""), "got: {html}");
    assert!(html.contains("class=\"ns-chips\""), "got: {html}");
    assert!(
        html.contains("class=\"ns-chip active\""),
        "the selected namespace reads as selected: {html}"
    );
    assert!(html.contains(">default<"), "chip label: {html}");
    assert!(html.contains(">team-a<"), "chip label: {html}");
    assert!(
        html.contains("<button"),
        "a host that owns the selection gets buttons: {html}"
    );
}

#[test]
fn status_footer_renders_a_connected_cluster() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    assert!(html.contains("class=\"status\""), "got: {html}");
    assert_eq!(html.matches("class=\"status-entry\"").count(), 2);
    assert!(html.contains("prod · Connected"), "got: {html}");
    assert!(html.contains("v1.2.3"), "got: {html}");
    assert!(
        html.contains("style=\"background: var(--green)\""),
        "the connected dot paints green: {html}"
    );
    assert!(
        html.contains("style=\"display: none\""),
        "the version slot carries no dot: {html}"
    );
}

#[test]
fn status_footer_renders_a_disconnected_host() {
    let _host = browser_host();
    let html = support::mount_html(browser_shell, || {});

    assert!(html.contains("no cluster · Disconnected"), "got: {html}");
    assert!(
        html.contains("style=\"background: var(--red)\""),
        "the disconnected dot paints red: {html}"
    );
    assert_eq!(html.matches("class=\"status-entry\"").count(), 1);
}

#[test]
fn a_desktop_profile_host_renders_the_chrome_it_supplies() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    assert!(
        html.contains("data-native-chrome=\"desktop\""),
        "the host's own chrome renders in the frame: {html}"
    );
    assert!(
        !html.contains("class=\"native-chrome-unsupported\""),
        "a host with the capability never renders the stand-in: {html}"
    );
}

#[test]
fn a_browser_profile_host_states_the_chrome_it_cannot_provide() {
    let _host = browser_host();
    let html = support::mount_html(browser_shell, || {});

    assert!(
        html.contains("class=\"native-chrome-unsupported\""),
        "the browser host says which chrome is missing: {html}"
    );
    assert!(
        html.contains("data-native-chrome=\"unsupported\""),
        "the stand-in is marked as unsupported chrome: {html}"
    );
    assert!(
        !html.contains("data-native-chrome=\"desktop\""),
        "no host chrome leaks into a host that did not supply it: {html}"
    );
    assert!(
        html.contains("class=\"sidebar\""),
        "the layout half still renders: {html}"
    );
}

#[test]
fn a_browser_profile_host_keeps_the_attributes_its_host_marks_it_with() {
    let _host = browser_host();
    let html = support::mount_html(browser_shell, || {});

    assert!(html.contains("data-surface=\"app\""), "got: {html}");
    assert!(html.contains("class=\"app-shell\""), "got: {html}");
    assert!(
        html.contains("data-context=\"1\""),
        "the context chip the browser host cannot toggle is marked: {html}"
    );
    assert!(
        html.contains("<span") && !html.contains("<button"),
        "a host without a selectable namespace list renders spans: {html}"
    );
}
