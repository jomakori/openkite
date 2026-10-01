//! Headless mounts of the app shell (OKT-154).
//!
//! The shell is props in, markup out, so these mount the crate's `AppShell`
//! with the states the hosts actually reach: a sidebar with entries and one
//! without, the sidebar footer connected and disconnected, the breadcrumbs a
//! route resolves to, and the chrome a desktop-profile host renders against
//! the stand-in a browser-profile host gets instead. tests/shell.rs pins the
//! model these props come from; tests/capability_gate.rs pins the predicates.

mod support;

use std::sync::Mutex;

use dioxus::prelude::*;
use openkite_api::capability::Capabilities;
use openkite_ui::components::shell::{AppShell, ClusterInfo, ShellIcon, TopBarAction};
use openkite_ui::plugin_api::RegistrationStore;
use openkite_ui::runtime::set_published_capabilities;
use openkite_ui::shell::{
    core_nav, status_bar_model, ShellNavItem, ShellSection, ShellState, StatusBarEntry,
};

// The host descriptor is process-global, exactly like tests/capability_gate.rs:
// hold this guard in every test that publishes a profile so parallel test
// threads cannot gate on each other's state.
static HOST_PROFILE: Mutex<()> = Mutex::new(());

/// Publish the desktop profile (in-process gateway, native window chrome) and
/// hold the lock for the mount.
fn desktop_host() -> std::sync::MutexGuard<'static, ()> {
    let guard = HOST_PROFILE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    set_published_capabilities(Some(Capabilities::in_process()));
    guard
}

/// Publish the browser profile (server-side gateway, no window chrome) and
/// hold the lock.
fn browser_host() -> std::sync::MutexGuard<'static, ()> {
    let guard = HOST_PROFILE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    set_published_capabilities(Some(Capabilities::server_side()));
    guard
}

/// The desktop's sidebar: the core navigation block, then one plugin section
/// with the contributor's accent.
fn desktop_sections() -> Vec<ShellSection> {
    vec![
        ShellSection {
            label: "Overview".into(),
            accent: None,
            items: core_nav(true),
        },
        ShellSection {
            label: "argocd".into(),
            accent: Some("var(--accent)".into()),
            items: vec![ShellNavItem {
                label: "Applications".into(),
                route: "/argocd/apps".into(),
                plugin: Some("argocd".into()),
                badge: Some("18".into()),
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

/// The shell as the desktop mounts it: entries with a badge, the cluster
/// button, the action row the host owns, the avatar of the user it names, the
/// host's own native chrome in the host slot, and the router callbacks.
fn desktop_shell() -> Element {
    rsx! {
        AppShell {
            sections: desktop_sections(),
            current_route: "/workloads",
            cluster: Some(ClusterInfo {
                label: "prod-us-east-1".into(),
                detail: Some("v1.29.4".into()),
                connected: true,
            }),
            status: connected_entries(),
            identity: Some("Eda Kite".into()),
            actions: vec![
                TopBarAction {
                    icon: ShellIcon::Search,
                    label: "Command palette".into(),
                    on_click: EventHandler::new(|_: ()| {}),
                },
                TopBarAction {
                    icon: ShellIcon::Settings,
                    label: "Settings".into(),
                    on_click: EventHandler::new(|_: ()| {}),
                },
            ],
            on_navigate: Some(EventHandler::new(|route: String| {
                let _ = route;
            })),
            on_switch_cluster: Some(EventHandler::new(|_: ()| {})),
            chrome: rsx! { div { "data-native-chrome": "desktop", "host chrome" } },
            div { "data-outlet": "workloads", "route outlet" }
        }
    }
}

/// The shell as the browser host mounts it: the shared sidebar model, a
/// server-side cluster button it cannot switch, no client router, no named
/// user and no native chrome.
fn browser_shell() -> Element {
    rsx! {
        AppShell {
            sections: vec![ShellSection {
                label: "Overview".into(),
                accent: None,
                items: vec![ShellNavItem {
                    label: "Cluster".into(),
                    route: "/cluster".into(),
                    plugin: None,
                    badge: None,
                }],
            }],
            current_route: String::new(),
            cluster: Some(ClusterInfo {
                label: "in-cluster".into(),
                detail: None,
                connected: false,
            }),
            status: disconnected_entries(),
            "data-surface": "app",
            div { "data-outlet": "overview", "route outlet" }
        }
    }
}

/// A shell with no navigation, no cluster and no status entries at all.
fn empty_shell() -> Element {
    rsx! {
        AppShell {
            sections: Vec::<ShellSection>::new(),
            current_route: String::new(),
            status: Vec::<StatusBarEntry>::new(),
            div { "data-outlet": "empty", "route outlet" }
        }
    }
}

#[test]
fn the_frame_renders_the_design_structure() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    assert!(html.contains("class=\"app\""), "frame: {html}");
    assert!(html.contains("class=\"sidebar\""), "sidebar: {html}");
    assert!(html.contains("class=\"main\""), "main column: {html}");
    assert!(html.contains("class=\"topbar\""), "top bar: {html}");
    assert!(
        html.contains("class=\"view active\""),
        "route outlet: {html}"
    );
    assert!(
        html.contains("class=\"sidebar-backdrop\""),
        "the design's drawer backdrop: {html}"
    );
    assert!(
        html.contains("class=\"pull-indicator\""),
        "the design's pull indicator: {html}"
    );
    assert!(
        html.contains("data-outlet=\"workloads\""),
        "the host's outlet still mounts inside the view: {html}"
    );
    assert!(
        !html.contains("app-shell") && !html.contains("main-col") && !html.contains("ns-chips"),
        "none of the desktop's old shell vocabulary survives: {html}"
    );
}

#[test]
fn the_brand_renders_the_mark_and_the_wordmark() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    assert!(html.contains("class=\"brand\""), "got: {html}");
    assert!(html.contains("class=\"brand-mark\""), "got: {html}");
    assert!(
        html.contains("viewBox=\"0 0 32 32\""),
        "the mark is the design's own glyph: {html}"
    );
    assert!(html.contains("class=\"brand-word\""), "got: {html}");
    assert!(
        html.contains("Open<strong>Kite</strong>"),
        "the wordmark splits like the design's: {html}"
    );
}

#[test]
fn nav_sections_carry_their_title_badge_and_accent() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    assert!(html.contains("class=\"nav-section\""), "got: {html}");
    assert!(html.contains("class=\"nav-title\""), "got: {html}");
    assert!(html.contains(">Overview<"), "core title: {html}");
    assert!(html.contains(">argocd<"), "plugin title: {html}");
    assert!(
        html.contains("style=\"color: var(--accent)\""),
        "the contributor's accent reaches the title and its entries: {html}"
    );
    assert!(
        html.contains("class=\"nav-item plugin\""),
        "plugin entries keep the plugin class: {html}"
    );
    assert!(
        html.contains("class=\"nav-badge\""),
        "the count badge: {html}"
    );
    assert!(html.contains(">18<"), "badge text: {html}");
    assert!(
        !html.contains("nav-divider") && !html.contains("nav-section-label"),
        "the old divider/label vocabulary is gone: {html}"
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
        !html.contains("nav-item") && !html.contains("nav-section"),
        "an empty model renders no sections and no entries: {html}"
    );
    assert!(
        html.contains("class=\"sidebar-footer\""),
        "the footer renders even with nothing to report: {html}"
    );
    assert!(
        html.contains("data-outlet=\"empty\""),
        "the frame still mounts the outlet: {html}"
    );
}

#[test]
fn breadcrumbs_resolve_cluster_section_then_route() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    let at = html.find("class=\"breadcrumbs\"").expect("breadcrumbs");
    let trail = &html[at..];
    let trail_end = trail
        .find("class=\"topbar-actions\"")
        .unwrap_or(trail.len());
    let trail = &trail[..trail_end];
    assert!(
        trail.contains("class=\"current\""),
        "the route is the current crumb: {trail}"
    );
    let cluster = trail.find("prod-us-east-1").expect("cluster crumb");
    let section = trail.find(">Overview<").expect("section crumb");
    let current = trail.find(">Workloads<").expect("route crumb");
    assert!(
        cluster < section && section < current,
        "crumbs read cluster → section → route: {trail}"
    );
}

#[test]
fn the_browser_hosts_breadcrumbs_are_the_cluster_it_serves() {
    let _host = browser_host();
    let html = support::mount_html(browser_shell, || {});

    assert!(html.contains("class=\"breadcrumbs\""), "got: {html}");
    assert!(html.contains(">in-cluster<"), "cluster crumb: {html}");
    assert!(
        !html.contains("class=\"current\""),
        "a host with no resolved route marks no crumb current: {html}"
    );
}

#[test]
fn the_cluster_button_carries_the_context_and_its_state() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    assert!(html.contains("class=\"cluster-btn\""), "got: {html}");
    assert!(
        html.contains("data-cluster=\"prod-us-east-1\""),
        "the button names the cluster: {html}"
    );
    assert!(html.contains("class=\"cluster-text\""), "got: {html}");
    assert!(
        html.contains("<small>v1.29.4</small>"),
        "detail line: {html}"
    );
    assert!(
        html.contains("class=\"status-line\""),
        "the button's state line: {html}"
    );
    assert!(
        html.contains("style=\"background: var(--green)\""),
        "a connected cluster paints green: {html}"
    );
    assert!(
        !html.contains("data-unsupported=\"cluster-switch\""),
        "an in-process host can switch contexts: {html}"
    );
}

#[test]
fn a_host_that_cannot_switch_contexts_renders_the_button_marked() {
    let _host = browser_host();
    let html = support::mount_html(browser_shell, || {});

    assert!(
        html.contains("data-unsupported=\"cluster-switch\""),
        "the server-side host declares what the button cannot do: {html}"
    );
    assert!(
        html.contains("data-cluster=\"in-cluster\""),
        "it still names the cluster it serves: {html}"
    );
}

#[test]
fn the_topbar_actions_and_avatar_are_what_the_host_publishes() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    assert!(html.contains("class=\"topbar-actions\""), "got: {html}");
    assert_eq!(
        html.matches("class=\"icon-btn\"").count(),
        2,
        "one button per action the host published: {html}"
    );
    assert!(
        html.contains("aria-label=\"Command palette\"") && html.contains("aria-label=\"Settings\""),
        "each action carries its accessible name: {html}"
    );
    assert!(html.contains("class=\"avatar\""), "avatar: {html}");
    assert!(html.contains(">EK<"), "the identity's initials: {html}");
}

#[test]
fn a_host_with_no_actions_renders_an_empty_action_row() {
    let _host = browser_host();
    let html = support::mount_html(browser_shell, || {});

    assert!(html.contains("class=\"topbar-actions\""), "got: {html}");
    assert!(
        !html.contains("class=\"icon-btn\""),
        "the browser host wires no top-bar action: {html}"
    );
    assert!(
        !html.contains("class=\"avatar\""),
        "and names no user, so no avatar: {html}"
    );
}

#[test]
fn the_sidebar_footer_renders_a_connected_cluster() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    assert!(html.contains("class=\"sidebar-footer\""), "got: {html}");
    assert!(
        html.contains("class=\"version\""),
        "the undotted entry is the host's build: {html}"
    );
    assert!(html.contains(">v1.2.3<"), "got: {html}");
    assert!(html.contains("prod · Connected"), "got: {html}");
    assert!(
        html.contains("style=\"background: var(--green)\""),
        "the connected dot paints green: {html}"
    );
    assert!(
        !html.contains("class=\"status\"") && !html.contains("status-entry"),
        "the desktop's bottom status bar vocabulary is gone: {html}"
    );
}

#[test]
fn the_sidebar_footer_renders_a_disconnected_host() {
    let _host = browser_host();
    let html = support::mount_html(browser_shell, || {});

    assert!(html.contains("no cluster · Disconnected"), "got: {html}");
    assert!(
        html.contains("style=\"background: var(--red)\""),
        "the disconnected dot paints red: {html}"
    );
    assert!(
        !html.contains("class=\"version\""),
        "a host with no version renders no version slot: {html}"
    );
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
        html.contains("class=\"icon-btn menu-toggle\""),
        "the window's menu toggle is part of the top bar: {html}"
    );
    assert!(
        !html.contains("data-native-chrome=\"unsupported\""),
        "a host with the capability never renders the stand-in: {html}"
    );
}

#[test]
fn a_browser_profile_host_states_the_chrome_it_cannot_provide() {
    let _host = browser_host();
    let html = support::mount_html(browser_shell, || {});

    assert!(
        html.contains("data-native-chrome=\"unsupported\""),
        "the browser host says which chrome is missing: {html}"
    );
    assert!(
        html.contains("class=\"eyebrow\""),
        "the note is labelled with the design's micro-label: {html}"
    );
    assert!(
        html.contains("data-disabled=\"1\""),
        "an unavailable entry is marked disabled, not silently dropped: {html}"
    );
    assert!(
        !html.contains("menu-toggle"),
        "a host with no window menu gets no menu toggle: {html}"
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
    assert!(html.contains("class=\"app\""), "got: {html}");
    assert!(
        html.contains("data-outlet=\"overview\""),
        "the browser host's own page still mounts: {html}"
    );
}
