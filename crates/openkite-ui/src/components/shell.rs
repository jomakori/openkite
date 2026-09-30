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
//! Navigation is the host's business too: an entry carries its href, and the
//! host passes [`AppShellProps::on_navigate`] when it has a client router (the
//! desktop). Without one the links are plain anchors and the browser navigates
//! — which is what the SSR console wants.

use dioxus::prelude::*;

use crate::components::crud_modal::CrudOverlay;
use crate::components::pod_detail::PodDetail;
use crate::components::secret_detail::SecretDetail;
use crate::runtime::native_chrome_can_render;
use crate::shell::{status_rows, ShellNavItem, ShellSection, StatusBarEntry};

/// One chip in the top bar's namespace row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceChip {
    /// Chip text: a namespace name, or the host's cluster label.
    pub label: String,
    /// Selected (chip pressed / current scope).
    pub active: bool,
    /// The chip an SSR host renders for its own cluster context, which it
    /// cannot toggle (there is no namespace list to select from).
    pub context: bool,
}

/// The app frame: the overlays, the sidebar, the top bar, the route outlet and
/// the status footer.
///
/// `sections` are the sidebar blocks in order (empty renders no entries),
/// `current_route` is what the sidebar marks active (`""` = nothing),
/// `namespaces` the top-bar chips, and `status` the footer entries. `children`
/// is the host's route outlet; `chrome` is the host-only native chrome,
/// rendered when the host advertises it; `attributes` carries whatever else the
/// host marks the frame with (the browser host tags it as its app surface).
#[component]
pub fn AppShell(
    sections: Vec<ShellSection>,
    current_route: String,
    namespaces: Vec<NamespaceChip>,
    status: Vec<StatusBarEntry>,
    // Where an entry click goes when the host owns a client router.
    #[props(default)] on_navigate: Option<EventHandler<String>>,
    // What a chip click does when the host owns a namespace selection.
    #[props(default)] on_toggle_namespace: Option<EventHandler<String>>,
    // Host-only native chrome: the menu-bar binding, the title-bar behaviour,
    // the in-window overlays and their key listeners. Rendered only when the
    // host advertises it.
    #[props(default)] chrome: Option<Element>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    // The host's route outlet.
    children: Element,
) -> Element {
    rsx! {
        div { class: "app-shell", ..attributes,
            if native_chrome_can_render() {
                {chrome}
            }
            PodDetail {}
            SecretDetail {}
            CrudOverlay {}
            Sidebar { sections, current_route, on_navigate }
            div { class: "main-col",
                TopBar { chips: namespaces, on_toggle: on_toggle_namespace }
                main { class: "content", {children} }
                StatusFooter { entries: status }
            }
        }
    }
}

/// The console sidebar: brand, navigation, and the note that stands in for the
/// native chrome a host cannot provide.
#[component]
pub fn Sidebar(
    sections: Vec<ShellSection>,
    current_route: String,
    #[props(default)] on_navigate: Option<EventHandler<String>>,
) -> Element {
    rsx! {
        aside { class: "sidebar",
            h1 { class: "brand", "OpenKite" }
            span { class: "tagline", "Kubernetes from above." }
            nav { class: "nav",
                for section in sections.iter().cloned() {
                    if section.divider {
                        div { class: "nav-divider" }
                    }
                    if section.label.is_empty() {
                        for item in section.items.iter().cloned() {
                            NavItem {
                                item,
                                current_route: current_route.clone(),
                                accent: section.accent.clone(),
                                on_navigate,
                            }
                        }
                    } else {
                        div { class: "nav-section",
                            div {
                                class: "nav-section-label",
                                style: section.accent.as_ref().map(|accent| format!("color: {accent}")),
                                "{section.label}"
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
                }
            }
            if !native_chrome_can_render() {
                NativeChromeUnsupported {}
            }
        }
    }
}

/// The namespace row of the top bar.
///
/// Chips are buttons when the host owns a namespace selection, plain spans
/// otherwise (the SSR console has no selection to toggle).
#[component]
pub fn TopBar(
    chips: Vec<NamespaceChip>,
    #[props(default)] on_toggle: Option<EventHandler<String>>,
) -> Element {
    rsx! {
        header { class: "topbar",
            div { class: "ns-chips",
                {chips.into_iter().map(|chip| chip_element(chip, on_toggle))}
            }
        }
    }
}

/// The bottom bar: one entry per status slot, each with its own dot.
#[component]
pub fn StatusFooter(entries: Vec<StatusBarEntry>) -> Element {
    rsx! {
        footer { class: "status",
            for (label, dot) in status_rows(&entries) {
                span { class: "status-entry",
                    span { class: "status-dot", style: "{dot}" }
                    "{label}"
                }
            }
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
            "{item.label}"
        }
    }
}

/// One top-bar chip, as a button when the host can select and as a span when it
/// cannot.
fn chip_element(chip: NamespaceChip, on_toggle: Option<EventHandler<String>>) -> Element {
    match on_toggle {
        Some(toggle) => {
            let label = chip.label.clone();
            rsx! {
                button {
                    class: if chip.active { "ns-chip active" } else { "ns-chip" },
                    onclick: move |_| toggle.call(label.clone()),
                    "{chip.label}"
                }
            }
        }
        None => rsx! {
            span {
                class: if chip.active { "ns-chip active" } else { "ns-chip" },
                "data-context": chip.context.then_some("1"),
                "{chip.label}"
            }
        },
    }
}

/// Stand-in for the native window chrome a host does not provide: the browser
/// console has no menu bar, no title-bar theme and no in-window overlays, and
/// the shell states which chrome is missing rather than rendering an empty
/// frame where the desktop's own chrome would be.
#[component]
fn NativeChromeUnsupported() -> Element {
    rsx! {
        div {
            class: "native-chrome-unsupported",
            "data-native-chrome": "unsupported",
            "This host provides no native window chrome: no menu bar, no title-bar theme, no in-window overlays. The console renders without them."
        }
    }
}
