//! The app shell: the frame, sidebar, top bar and status footer both hosts
//! mount.
//!
//! The split is the point of the module (OKT-154). The layout and navigation
//! half is props in, markup out — the same sidebar, top bar, status footer and
//! frame render on the desktop and in the browser console, from this crate,
//! with no host type in sight. The host-only half — the native window chrome:
//! the menu-bar binding, the title-bar theme, the in-window overlays and the
//! `document::eval` key listeners — arrives as the [`AppShellProps::chrome`]
//! slot and is rendered only when the host advertises it through
//! [`crate::runtime::native_chrome_can_render`]. A host that cannot provide it
//! says so in the sidebar instead of leaving the frame empty.
//!
//! The markup is the design's (OKT-154): `.app` frame, `.sidebar` with
//! `.brand` / `.cluster-btn` / `.nav-section` / `.nav-item` /
//! `.sidebar-footer`, `.main` with `.topbar` (`(menu-toggle) · .breadcrumbs ·
//! .topbar-actions · .avatar`) over the routed `.view`. Classes the desktop
//! used to render (`app-shell`, `ns-chips`, `status-entry`, `main-col`, …) are
//! gone with their stylesheet rules.
//!
//! Navigation is the host's business too: an entry carries its href, and the
//! host passes [`AppShellProps::on_navigate`] when it has a client router (the
//! desktop). Without one the links are plain anchors and the browser navigates
//! — which is what the SSR console wants.

use dioxus::prelude::*;

use crate::components::crud_modal::CrudOverlay;
use crate::components::pod_detail::PodDetail;
use crate::components::secret_detail::SecretDetail;
use crate::runtime::{cluster_switch_can_render, native_chrome_can_render};
use crate::shell::{
    breadcrumbs, initials, status_rows, Crumb, ShellNavItem, ShellSection, StatusBarEntry,
};

/// The cluster the console is pointed at, as the sidebar's cluster button
/// shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterInfo {
    /// Context name, or the host's own label for the cluster it serves.
    pub label: String,
    /// Secondary line under the name (the host's build, region, …).
    pub detail: Option<String>,
    /// Live connection state, painted as the button's dot.
    pub connected: bool,
}

/// The glyph set the shell draws. The crate ships no icon font: each glyph is
/// the design's own inline SVG path, styled by `.icon`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellIcon {
    Menu,
    Search,
    Refresh,
    Settings,
    Chevron,
}

/// One icon button in the top bar's action row. The host owns what a click
/// does; the crate owns the design's icon button.
#[derive(Clone, PartialEq)]
pub struct TopBarAction {
    pub icon: ShellIcon,
    /// Accessible name — the design's icon buttons carry no text.
    pub label: String,
    pub on_click: EventHandler<()>,
}

/// The app frame: the overlays, the sidebar, the top bar, the route outlet and
/// the status footer.
///
/// `sections` are the sidebar blocks in order (empty renders no entries),
/// `current_route` is what the sidebar marks active and what the breadcrumbs
/// resolve (`""` = nothing), `cluster` the sidebar's cluster button, `status`
/// the sidebar footer entries, `actions` the top bar's icon buttons and
/// `identity` the name behind the avatar. `children` is the host's route
/// outlet; `chrome` is the host-only native chrome, rendered when the host
/// advertises it; `attributes` carries whatever else the host marks the frame
/// with (the browser host tags it as its app surface).
#[component]
pub fn AppShell(
    sections: Vec<ShellSection>,
    current_route: String,
    #[props(default)] cluster: Option<ClusterInfo>,
    status: Vec<StatusBarEntry>,
    #[props(default)] actions: Vec<TopBarAction>,
    #[props(default)] identity: Option<String>,
    // Where an entry click goes when the host owns a client router.
    #[props(default)] on_navigate: Option<EventHandler<String>>,
    // What the cluster button does when the host can switch contexts.
    #[props(default)] on_switch_cluster: Option<EventHandler<()>>,
    // Host-only native chrome: the menu-bar binding, the title-bar behaviour,
    // the in-window overlays and their key listeners. Rendered only when the
    // host advertises it.
    #[props(default)] chrome: Option<Element>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    // The host's route outlet.
    children: Element,
) -> Element {
    let crumbs = breadcrumbs(
        &current_route,
        cluster.as_ref().map(|cluster| cluster.label.as_str()),
        &sections,
    );
    // The design's drawer: the sidebar slides over the view below 1024px and
    // the top bar's menu toggle opens it. Above 1024px the stylesheet hides
    // the toggle and the sidebar is part of the frame, so the state is inert.
    let mut drawer_open = use_signal(|| false);
    let toggle_drawer = EventHandler::new(move |_: ()| {
        let next = !drawer_open();
        drawer_open.set(next);
    });
    rsx! {
        div { class: "app", ..attributes,
            if native_chrome_can_render() {
                {chrome}
            }
            PodDetail {}
            SecretDetail {}
            CrudOverlay {}
            Sidebar {
                sections: sections.clone(),
                current_route: current_route.clone(),
                cluster: cluster.clone(),
                status,
                open: drawer_open(),
                on_navigate,
                on_switch_cluster,
            }
            button {
                class: if drawer_open() { "sidebar-backdrop show" } else { "sidebar-backdrop" },
                r#type: "button",
                aria_label: "Close navigation",
                onclick: move |_| drawer_open.set(false),
            }
            div { class: "main",
                TopBar {
                    crumbs,
                    actions,
                    identity,
                    // The menu toggle is the window's own chrome: a host with
                    // no window menu does not get one.
                    menu_toggle: native_chrome_can_render().then_some(toggle_drawer),
                    drawer_open: drawer_open(),
                }
                PullIndicator {}
                section { class: "view active", {children} }
            }
        }
    }
}

/// The console sidebar: brand, the cluster button, navigation, and the footer
/// carrying the host's build and the live cluster state.
#[component]
pub fn Sidebar(
    sections: Vec<ShellSection>,
    current_route: String,
    #[props(default)] cluster: Option<ClusterInfo>,
    status: Vec<StatusBarEntry>,
    #[props(default)] open: bool,
    #[props(default)] on_navigate: Option<EventHandler<String>>,
    #[props(default)] on_switch_cluster: Option<EventHandler<()>>,
) -> Element {
    rsx! {
        aside { class: if open { "sidebar open" } else { "sidebar" },
            div { class: "brand",
                svg { class: "brand-mark", "viewBox": "0 0 32 32",
                    path { d: "M16 3l11.5 10L16 28 4.5 13 16 3z", fill: "var(--brand)" }
                    path {
                        d: "M12.2 15 16 13l6-4.8M12.9 20 16 13l6.3 5.2",
                        fill: "none",
                        stroke: "var(--surface)",
                        "stroke-width": "1.8",
                        "stroke-linecap": "round",
                        "stroke-linejoin": "round",
                    }
                    path {
                        d: "M16 28v4M11.5 29.2 16 33l4.5-3.8",
                        fill: "none",
                        stroke: "var(--brand)",
                        "stroke-width": "1.8",
                        "stroke-linecap": "round",
                        "stroke-linejoin": "round",
                    }
                }
                span { class: "brand-word", "Open" strong { "Kite" } }
            }
            if let Some(cluster) = cluster {
                ClusterButton { cluster, on_switch: on_switch_cluster }
            }
            nav { class: "nav", aria_label: "Resource navigation",
                for section in sections.iter().cloned() {
                    div { class: "nav-section",
                        if !section.label.is_empty() {
                            div {
                                class: "nav-title",
                                style: section.accent.as_ref().map(|accent| format!("color: {accent}")),
                                "{section.label}"
                            }
                        }
                        for item in section.items.iter().cloned() {
                            NavItem {
                                item,
                                current_route: current_route.clone(),
                                accent: section.accent.clone(),
                                on_navigate,
                            }
                        }
                    }
                }
                if !native_chrome_can_render() {
                    div { class: "nav-section",
                        span { class: "eyebrow", "Host" }
                        div {
                            class: "nav-item",
                            "data-disabled": "1",
                            "data-native-chrome": "unsupported",
                            "This host has no window chrome: no menu bar, no title-bar theme, no in-window overlays."
                        }
                    }
                }
            }
            SidebarFooter { entries: status }
        }
    }
}

/// The sidebar's cluster button: the context the console is pointed at, with
/// its connection state. Clicking it opens the host's context list — a host
/// that cannot list contexts (a server-side gateway serving one cluster)
/// renders the same button marked `data-unsupported` and read-only, so the
/// affordance is declared rather than silently missing.
#[component]
fn ClusterButton(
    cluster: ClusterInfo,
    #[props(default)] on_switch: Option<EventHandler<()>>,
) -> Element {
    let label = cluster.label.clone();
    if cluster_switch_can_render() {
        let onclick = move |_| {
            if let Some(handler) = on_switch.as_ref() {
                handler.call(());
            }
        };
        rsx! {
            button {
                class: "cluster-btn",
                r#type: "button",
                "data-cluster": "{label}",
                onclick,
                ClusterText { cluster: cluster.clone() }
                Icon { icon: ShellIcon::Chevron }
            }
        }
    } else {
        rsx! {
            div {
                class: "cluster-btn",
                "data-cluster": "{label}",
                "data-unsupported": "cluster-switch",
                ClusterText { cluster: cluster.clone() }
            }
        }
    }
}

/// The cluster button's text: the dot for the connection state, the context
/// name, and the host's secondary line.
#[component]
fn ClusterText(cluster: ClusterInfo) -> Element {
    let dot = if cluster.connected {
        "background: var(--success)"
    } else {
        "background: var(--danger)"
    };
    rsx! {
        span { class: "status-line",
            span { class: "dot", style: "{dot}" }
        }
        span { class: "cluster-text",
            "{cluster.label}"
            if let Some(detail) = cluster.detail.clone() {
                small { "{detail}" }
            }
        }
    }
}

/// The namespace row of the top bar.
///
/// Breadcrumbs resolve from the sidebar model, the action row is what the host
/// wired, and the avatar renders when the host names who is looking.
#[component]
pub fn TopBar(
    crumbs: Vec<Crumb>,
    #[props(default)] actions: Vec<TopBarAction>,
    #[props(default)] identity: Option<String>,
    // The drawer toggle, supplied only by a host that renders window chrome.
    #[props(default)] menu_toggle: Option<EventHandler<()>>,
    // Whether the drawer is open, so the toggle can report it.
    #[props(default)] drawer_open: bool,
) -> Element {
    rsx! {
        header { class: "topbar",
            if let Some(toggle) = menu_toggle {
                button {
                    class: "icon-btn menu-toggle",
                    r#type: "button",
                    aria_label: "Open navigation",
                    "aria-expanded": if drawer_open { "true" } else { "false" },
                    onclick: move |_| toggle.call(()),
                    Icon { icon: ShellIcon::Menu }
                }
            }
            div { class: "breadcrumbs",
                for (index, crumb) in crumbs.iter().enumerate() {
                    if index > 0 {
                        Icon { icon: ShellIcon::Chevron }
                    }
                    span {
                        // The design hides the steps between the cluster and the
                        // current page on narrow viewports.
                        class: if crumb.current {
                            "current"
                        } else if index > 0 {
                            "crumb-hide"
                        } else {
                            "crumb"
                        },
                        "{crumb.label}"
                    }
                }
            }
            div { class: "topbar-actions",
                for action in actions.iter().cloned() {
                    button {
                        class: "icon-btn",
                        r#type: "button",
                        aria_label: "{action.label}",
                        title: "{action.label}",
                        onclick: move |_| action.on_click.call(()),
                        Icon { icon: action.icon }
                    }
                }
                if let Some(identity) = identity.clone() {
                    button {
                        class: "avatar",
                        r#type: "button",
                        aria_label: "User menu",
                        "{initials(&identity)}"
                    }
                }
            }
        }
    }
}

/// The sidebar footer: the host's build on the left, one `.status-line` per
/// live status slot on the right.
#[component]
pub fn SidebarFooter(entries: Vec<StatusBarEntry>) -> Element {
    rsx! {
        div { class: "sidebar-footer",
            for (label, dot) in status_rows(&entries) {
                if dot == "display: none" {
                    span { class: "version", "{label}" }
                } else {
                    span { class: "status-line",
                        span { class: "dot", style: "{dot}" }
                        "{label}"
                    }
                }
            }
        }
    }
}

/// The pull-to-refresh indicator the design floats over the top bar. It stays
/// out of the viewport until a host that supports the gesture adds `.show`.
#[component]
fn PullIndicator() -> Element {
    rsx! {
        div { class: "pull-indicator", "data-pull": "idle",
            span { class: "spinner" }
            "Release to refresh"
        }
    }
}

/// One sidebar entry: an anchor that marks itself active, carries the
/// contributing plugin's accent, and asks the host to route the click.
#[component]
fn NavItem(
    item: ShellNavItem,
    current_route: String,
    #[props(default)] accent: Option<String>,
    #[props(default)] on_navigate: Option<EventHandler<String>>,
) -> Element {
    let active = item.route == current_route;
    let plugin = item.plugin.is_some();
    let route = item.route.clone();
    let onclick = move |event: Event<MouseData>| {
        // A host with a router handles the click itself; the href stays as the
        // honest fallback for anything that ignores the handler.
        if let Some(handler) = on_navigate.as_ref() {
            event.prevent_default();
            handler.call(route.clone());
        }
    };
    rsx! {
        a {
            class: if plugin {
                if active { "nav-item plugin active" } else { "nav-item plugin" }
            } else if active {
                "nav-item active"
            } else {
                "nav-item"
            },
            href: "{item.route}",
            style: accent.as_ref().map(|accent| format!("color: {accent}")),
            onclick,
            span { "{item.label}" }
            if let Some(badge) = item.badge.clone() {
                span { class: "nav-badge", "{badge}" }
            }
        }
    }
}

/// The design's inline glyphs. `class="icon"` carries the stroke styling from
/// the stylesheet; the paths are the design's own.
#[component]
fn Icon(icon: ShellIcon) -> Element {
    rsx! {
        svg { class: "icon", "viewBox": "0 0 24 24", {glyph(icon)} }
    }
}

/// The path set of one glyph, as the design draws it.
fn glyph(icon: ShellIcon) -> Element {
    match icon {
        ShellIcon::Menu => rsx! {
            path { d: "M4 7h16M4 12h16M4 17h16" }
        },
        ShellIcon::Search => rsx! {
            circle { cx: "11", cy: "11", r: "6.5" }
            path { d: "m16 16 4 4" }
        },
        ShellIcon::Refresh => rsx! {
            path { d: "M19 8a7.5 7.5 0 1 0 2 6" }
            path { d: "M19 3v5h-5" }
        },
        ShellIcon::Settings => rsx! {
            circle { cx: "12", cy: "12", r: "3" }
            path { d: "M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3A1.7 1.7 0 0 0 10 3V2.8h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1z" }
        },
        ShellIcon::Chevron => rsx! {
            path { d: "m9 6 6 6-6 6" }
        },
    }
}
