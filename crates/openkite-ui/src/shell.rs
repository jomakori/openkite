//! Console shell: sidebar, nav tree, topbar and the main region the pages render into.

use dioxus::prelude::*;

use crate::nav::{build_nav, entry_for_path, NavEntry, NavGroup, CATALOGUE, VIEWS};

/// Token stylesheet for surfaces 1, 2 and 18.
const CONSOLE_CSS: Asset = asset!("/assets/console.css");
/// Sprite the nav and topbar icons reference.
const ICONS: Asset = asset!("/assets/icons.svg");

/// Per-kind row counts behind the nav badges; filled from counts frames, never from full rows.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Counts {
    counts: std::collections::BTreeMap<&'static str, usize>,
}

impl Counts {
    /// Empty counts, so a shell can render before the first frame lands.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one kind count.
    pub fn with(mut self, kind: &'static str, rows: usize) -> Self {
        self.counts.insert(kind, rows);
        self
    }

    /// Rows known for a kind; zero when no frame has landed yet.
    pub fn get(&self, kind: &str) -> usize {
        self.counts.get(kind).copied().unwrap_or(0)
    }
}

/// Watch state shown in the sidebar footer.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Connection {
    Live,
    Degraded,
    Offline,
}

impl Connection {
    /// Footer wording for the state.
    pub fn label(self) -> &'static str {
        match self {
            Connection::Live => "Watching cluster",
            Connection::Degraded => "Connection degraded",
            Connection::Offline => "Disconnected",
        }
    }

    /// Class suffix that picks the status dot colour.
    pub fn tone(self) -> &'static str {
        match self {
            Connection::Live => "ok",
            Connection::Degraded => "warn",
            Connection::Offline => "err",
        }
    }
}

/// Icon reference into the console sprite.
#[component]
fn Icon(name: &'static str) -> Element {
    let href = format!("{ICONS}#i-{name}");
    rsx! {
        svg { class: "icon", use { href: "{href}" } }
    }
}

/// One nav row; badges read the counts map instead of the row set.
#[component]
fn NavRow(entry: NavEntry, count: usize, active: bool) -> Element {
    let class = if active {
        "nav-item active"
    } else {
        "nav-item"
    };
    rsx! {
        a {
            class: "{class}",
            href: "{entry.path}",
            aria_current: if active { "page" } else { "false" },
            Icon { name: entry.icon }
            span { "{entry.label}" }
            if entry.badge_kind.is_some() {
                span { class: "nav-badge", "{count}" }
            }
        }
    }
}

/// Inputs for the shell frame.
#[derive(Props, Clone, PartialEq, Debug)]
pub struct AppShellProps {
    pub counts: Counts,
    pub active_path: String,
    pub cluster: String,
    pub connection: Connection,
    pub children: Element,
}

/// The shell frame: fixed sidebar and topbar around the routed page body.
#[allow(non_snake_case)]
pub fn AppShell(props: AppShellProps) -> Element {
    let AppShellProps { counts, active_path, cluster, connection, children } = props;
    let groups: Vec<NavGroup> = build_nav(CATALOGUE, VIEWS);
    let trail: Vec<&'static str> = match entry_for_path(&groups, &active_path) {
        Some((group, entry)) => vec![group.title, entry.label],
        None => vec!["Overview"],
    };

    rsx! {
        document::Stylesheet { href: CONSOLE_CSS }
        div { class: "app",
            aside { class: "sidebar",
                div { class: "brand",
                    span { class: "brand-mark", Icon { name: "cluster" } }
                    span { class: "brand-word", "Open" strong { "Kite" } }
                }
                button { class: "cluster-btn", r#type: "button", aria_label: "Switch cluster",
                    Icon { name: "cluster" }
                    span { class: "cluster-text", "{cluster}" }
                    Icon { name: "chevron" }
                }
                nav { class: "nav", aria_label: "Resource navigation",
                    for group in groups.iter() {
                        div { class: "nav-section", key: "{group.title}",
                            button { class: "nav-title", r#type: "button", aria_expanded: "true",
                                span { "{group.title}" }
                                Icon { name: "chevron" }
                            }
                            div { class: "nav-items",
                                for entry in group.entries.iter() {
                                    NavRow {
                                        key: "{entry.path}",
                                        entry: entry.clone(),
                                        count: entry.badge_kind.map(|kind| counts.get(kind)).unwrap_or(0),
                                        active: entry.path == active_path,
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "sidebar-footer",
                    div { class: "status-line",
                        span { class: "dot {connection.tone()}" }
                        span { "{connection.label()}" }
                    }
                }
            }
            main { class: "main",
                header { class: "topbar",
                    button { class: "icon-btn menu-toggle", r#type: "button", aria_label: "Toggle navigation",
                        Icon { name: "menu" }
                    }
                    div { class: "breadcrumbs",
                        button { class: "crumb-cluster", r#type: "button", aria_label: "Cluster overview",
                            span { class: "dot {connection.tone()}" }
                            span { "{cluster}" }
                            Icon { name: "chevron" }
                        }
                        span { class: "crumb-trail",
                            for crumb in trail.iter() {
                                Icon { name: "chevron" }
                                span { class: "crumb", "{crumb}" }
                            }
                        }
                    }
                    div { class: "topbar-actions",
                        button { class: "icon-btn", r#type: "button", aria_label: "Search",
                            Icon { name: "search" }
                        }
                    }
                }
                div { class: "page-body", {children} }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_read_missing_kinds_as_zero() {
        let counts = Counts::new().with("pods", 12);
        assert_eq!(counts.get("pods"), 12);
        assert_eq!(counts.get("secrets"), 0);
    }

    #[test]
    fn connection_states_carry_a_tone() {
        assert_eq!(Connection::Live.tone(), "ok");
        assert_eq!(Connection::Offline.label(), "Disconnected");
    }
}
