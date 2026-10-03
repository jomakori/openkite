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
use openkite_ui::components::shell::{AppShell, ClusterInfo, ShellIcon, Sidebar, TopBarAction};
use openkite_ui::plugin_api::RegistrationStore;
use openkite_ui::runtime::{set_published_capabilities, PushMode};
use openkite_ui::shell::{
    core_sections, core_sections_with_counts, status_bar_model, NavCounts, ShellNavItem,
    ShellSection, ShellState, StatusBarEntry,
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

/// The desktop's sidebar: the core sections, then the plugin slot's one
/// section, carrying the contributor's accent.
fn desktop_sections() -> Vec<ShellSection> {
    let mut sections = core_sections();
    sections.push(ShellSection {
        label: "argocd".into(),
        accent: Some("var(--accent)".into()),
        items: vec![ShellNavItem {
            label: "Applications".into(),
            route: "/argocd/apps".into(),
            plugin: Some("argocd".into()),
            badge: Some("18".into()),
        }],
    });
    sections
}

/// The desktop's sidebar with the counts the reflectors publish.
fn counted_sections() -> Vec<ShellSection> {
    core_sections_with_counts(&NavCounts {
        nodes: Some(8),
        pods: Some(124),
        deployments: Some(37),
        services: Some(29),
        config_maps: Some(46),
    })
}

/// The shell as a connected desktop mounts it: the reference's sections with
/// the live counts the reflectors published.
fn counted_shell() -> Element {
    rsx! {
        AppShell {
            sections: counted_sections(),
            current_route: "/workloads",
            cluster: Some(ClusterInfo {
                label: "prod-us-east-1".into(),
                detail: None,
                connected: true,
            }),
            status: connected_entries(),
            div { "data-outlet": "counted", "route outlet" }
        }
    }
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
        PushMode::Push,
    )
}

/// The status entries a host without a cluster produces.
fn disconnected_entries() -> Vec<StatusBarEntry> {
    status_bar_model(
        &ShellState::default(),
        &RegistrationStore::new(),
        "",
        PushMode::Polling,
    )
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
    for title in [
        ">Cluster<",
        ">Workloads<",
        // The SSR pass escapes `&` as `&#38;` in a text node.
        ">Config &#38; Storage<",
        ">argocd<",
    ] {
        assert!(html.contains(title), "section title {title}: {html}");
    }
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
fn core_sections_render_their_entries_and_no_badge_without_counts() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    for entry in [
        ">Overview<",
        ">Nodes<",
        ">Pods<",
        ">Deployments<",
        ">Services<",
        ">ConfigMaps<",
        ">Storage<",
        ">Network<",
    ] {
        assert!(html.contains(entry), "core entry {entry}: {html}");
    }
    // Disconnected (no counts published): the plugin's own badge is the only
    // one in the sidebar — the shell renders no zero.
    assert_eq!(
        html.matches("class=\"nav-badge\"").count(),
        1,
        "only the plugin's badge draws: {html}"
    );
    assert!(html.contains(">18<"), "the plugin's count: {html}");
    assert!(
        !html.contains(">0<"),
        "a missing count is absent, never zero: {html}"
    );
}

#[test]
fn live_counts_reach_their_own_rows() {
    let _host = desktop_host();
    let html = support::mount_html(counted_shell, || {});

    for badge in [">8<", ">124<", ">37<", ">29<", ">46<"] {
        assert!(html.contains(badge), "count badge {badge}: {html}");
    }
    // One badge per row the reference badges: Nodes, Pods, Deployments,
    // Services and ConfigMaps. Storage and Network draw none.
    assert_eq!(
        html.matches("class=\"nav-badge\"").count(),
        5,
        "one badge per positive count: {html}"
    );
}

#[test]
fn the_plugin_slot_renders_a_section_the_shell_does_not_own() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    // The Argo CD section is the plugin's, and the shell pins the reference's
    // variant class on it: `.nav-section.argo` is the shell's rule.
    assert_eq!(
        html.matches("class=\"nav-section argo\"").count(),
        1,
        "exactly the plugin's section carries the argo variant: {html}"
    );
    assert!(
        html.contains("data-plugin=\"argocd\""),
        "the slot names its contributor: {html}"
    );
    assert!(
        html.contains(">Applications<") && html.contains("href=\"/argocd/apps\""),
        "the plugin's entries render inside the shell's section: {html}"
    );
    assert!(
        openkite_ui::MAIN_CSS.contains(".nav-section.argo"),
        "the shell styles the variant the plugin does not own"
    );
}

#[test]
fn the_current_route_marks_exactly_one_entry() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    // /workloads carries three entries (Pods, Deployments, Services); the
    // route's owner — its first carrier — is the one marked current.
    assert_eq!(
        html.matches("class=\"nav-item active\"").count(),
        1,
        "one current entry per route: {html}"
    );
    let active_at = html
        .find("class=\"nav-item active\"")
        .expect("an active entry");
    let entry = &html[active_at..];
    let entry = &entry[..entry.find("</a>").expect("the anchor closes")];
    assert!(entry.contains(">Pods<"), "the route's owner: {entry}");
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
    let section = trail.find(">Workloads<").expect("section crumb");
    let current = trail.find(">Pods<").expect("route crumb");
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
        html.contains("style=\"background: var(--success)\""),
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
        html.contains("style=\"background: var(--success)\""),
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
        html.contains("style=\"background: var(--danger)\""),
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

/// The sidebar on its own, standing open the way the drawer's toggle leaves
/// it below 1025px.
fn open_drawer() -> Element {
    rsx! {
        Sidebar {
            sections: desktop_sections(),
            current_route: "/workloads",
            cluster: Some(ClusterInfo {
                label: "prod-us-east-1".into(),
                detail: None,
                connected: true,
            }),
            status: connected_entries(),
            open: true,
        }
    }
}

/// The same sidebar in the desktop frame, where it is part of the layout.
fn closed_drawer() -> Element {
    rsx! {
        Sidebar {
            sections: desktop_sections(),
            current_route: "/workloads",
            status: connected_entries(),
        }
    }
}

#[test]
fn the_bottom_bar_lists_the_sidebar_models_leading_destinations() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});
    let bar = bottom_nav_region(&html);

    assert!(html.contains("class=\"bottom-nav\""), "bar: {html}");
    assert!(bar.contains("class=\"bottom-tabs\""), "grid: {bar}");
    assert!(
        bar.contains("aria-label=\"Mobile navigation\""),
        "the bar is the design's own landmark: {bar}"
    );
    // The model's own leading routes, in order — not a second route list.
    assert!(
        bar.contains("<a class=\"bottom-tab\" href=\"/cluster\">"),
        "first destination tab: {bar}"
    );
    assert!(
        bar.contains("<a class=\"bottom-tab active\" href=\"/workloads\">"),
        "the current route's tab is the active one: {bar}"
    );
    assert!(
        bar.contains("<a class=\"bottom-tab\" href=\"/logs\">"),
        "third destination tab: {bar}"
    );
    assert!(
        !bar.contains("href=\"/terminal\"") && !bar.contains("href=\"/config\""),
        "destinations past the grid stay in the drawer: {bar}"
    );
}

/// The rendered bottom bar on its own, so assertions do not match the
/// drawer's copy of the same destinations.
fn bottom_nav_region(html: &str) -> &str {
    let start = html
        .find("class=\"bottom-nav\"")
        .expect("the bar is rendered");
    let rest = &html[start..];
    let end = rest.find("</nav>").map(|i| i + 6).unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn the_bottom_bar_carries_the_drawers_own_menu_tab() {
    let _host = desktop_host();
    let html = support::mount_html(desktop_shell, || {});

    assert!(
        html.contains("aria-label=\"Open navigation\""),
        "the Menu tab opens the drawer: {html}"
    );
    assert!(
        html.contains(">Menu<"),
        "the Menu tab is labelled like the design's: {html}"
    );
    assert!(
        html.contains("<button class=\"bottom-tab\""),
        "the Menu tab is a button, not a destination link: {html}"
    );
    assert!(
        html.contains("aria-expanded=\"false\""),
        "a closed drawer is reported closed: {html}"
    );
}

#[test]
fn a_browser_profile_host_gets_the_bar_from_its_own_sidebar_model() {
    let _host = browser_host();
    let html = support::mount_html(browser_shell, || {});

    assert!(html.contains("class=\"bottom-nav\""), "bar: {html}");
    assert!(
        html.contains("<a class=\"bottom-tab\" href=\"/cluster\">"),
        "the browser host's one destination: {html}"
    );
    assert!(
        !html.contains("href=\"/workloads\""),
        "tabs come from this host's model, not a fixed list: {html}"
    );
}

#[test]
fn the_open_drawer_is_a_modal_dialog() {
    let _host = desktop_host();
    let html = support::mount_html(open_drawer, || {});

    assert!(html.contains("class=\"sidebar open\""), "open: {html}");
    assert!(
        html.contains("role=\"dialog\""),
        "the open drawer is a dialog: {html}"
    );
    assert!(
        html.contains("aria-modal=\"true\""),
        "the open drawer is modal: {html}"
    );
    assert!(
        html.contains("aria-label=\"Navigation\""),
        "the dialog is named: {html}"
    );
}

#[test]
fn the_closed_sidebar_is_not_a_dialog() {
    let _host = desktop_host();
    let html = support::mount_html(closed_drawer, || {});

    assert!(
        html.contains("class=\"sidebar\""),
        "the frame's sidebar: {html}"
    );
    assert!(
        html.contains("role=\"complementary\""),
        "in the frame it stays the plain landmark: {html}"
    );
    assert!(
        html.contains("aria-modal=\"false\""),
        "not modal when it is part of the layout: {html}"
    );
    assert!(
        !html.contains("role=\"dialog\""),
        "no dialog role leaks into the desktop frame: {html}"
    );
}
