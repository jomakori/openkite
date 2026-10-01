//! Route chrome: the design's heading row, filter toolbar and the declared
//! empty/unsupported states the console's primary routes render (OKT-155).
//!
//! Both hosts mount this chrome. The route is the contract the sidebar entries
//! and the breadcrumbs already carry — the URL path — so nothing here routes,
//! and no second routing model exists: the desktop passes the path its
//! `Routable` enum resolved, the browser console the route it serves.
//!
//! What a host genuinely cannot do is declared, never hidden behind an empty
//! route and never silently absent. Every affordance a route offers names the
//! capability it needs ([`RouteCapability`]); the host's verdict comes from the
//! descriptor in [`crate::runtime`] — the same predicates the shell's cluster
//! button and the terminal route gate on — and an affordance the host does not
//! advertise renders the design's own control marked `data-unsupported` with
//! the descriptor's name for what is missing. The route's `.tag-row` states
//! those facts as chips, so the declaration is readable without hovering.

use dioxus::prelude::*;

use crate::runtime::{cluster_switch_can_render, mutations_can_render, terminal_can_render};
use crate::shell::ShellSection;

/// What a route's chrome needs the host to be able to do before it can offer
/// the affordance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteCapability {
    /// The standalone terminal surface.
    Terminal,
    /// Cluster changes through the host's gateway (create/edit/delete).
    Mutations,
    /// Listing and switching cluster contexts.
    ClusterSwitch,
}

impl RouteCapability {
    /// Every capability a route can declare, in declaration order.
    pub const ALL: [RouteCapability; 3] = [
        RouteCapability::Terminal,
        RouteCapability::Mutations,
        RouteCapability::ClusterSwitch,
    ];

    /// What the host reports. The predicates live in [`crate::runtime`] so the
    /// route chrome, the shell and the terminal route never disagree.
    pub fn is_satisfied(self) -> bool {
        match self {
            RouteCapability::Terminal => terminal_can_render(),
            RouteCapability::Mutations => mutations_can_render(),
            RouteCapability::ClusterSwitch => cluster_switch_can_render(),
        }
    }

    /// The descriptor's own name for the capability — the `data-unsupported`
    /// value, so an unsupported control greps back to the predicate that
    /// removed it.
    pub fn id(self) -> &'static str {
        match self {
            RouteCapability::Terminal => "terminal",
            RouteCapability::Mutations => "cluster-mutation",
            RouteCapability::ClusterSwitch => "cluster-switch",
        }
    }

    /// The capability in the chips' monospace voice (`.tag`).
    pub fn label(self) -> &'static str {
        match self {
            RouteCapability::Terminal => "terminal",
            RouteCapability::Mutations => "cluster changes",
            RouteCapability::ClusterSwitch => "context switching",
        }
    }

    /// What the host is missing, in the user's words — the control's `title`
    /// and the sentence the declaration carries.
    pub fn missing(self) -> &'static str {
        match self {
            RouteCapability::Terminal => "This host does not advertise the terminal surface.",
            RouteCapability::Mutations => "This host's gateway does not accept cluster changes.",
            RouteCapability::ClusterSwitch => {
                "This host serves one cluster: it has no context list to switch between."
            }
        }
    }
}

/// The `.tag` chip's text for a capability: the machine fact, the way the
/// design's own tags read (`img: nginx:1.25`). Unsupported is stated, not
/// implied by an absent chip.
pub fn capability_tag(capability: RouteCapability) -> String {
    let state = if capability.is_satisfied() {
        "on"
    } else {
        "off"
    };
    format!("{}: {state}", capability.label())
}

/// Which of the design's two buttons an action paints as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteActionKind {
    Primary,
    Secondary,
}

/// One action in the route's `.page-actions` row.
///
/// The crate owns the button; the host owns what it does. `id` is what the
/// host's handler switches on, `requires` is what the host must advertise for
/// the button to be live at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteAction {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: RouteActionKind,
    pub requires: RouteCapability,
}

/// The chrome one route renders: the design's page head, the filter row's
/// placeholder, the body copy a host with nothing to show renders, and the
/// actions the route offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePage {
    /// The normalized route contract (the URL path both hosts agree on).
    pub route: String,
    /// The sidebar section the route belongs to, when the model knows it. The
    /// navigation's own word, so the head and the sidebar agree.
    pub eyebrow: Option<String>,
    /// The route's title — the navigation's label for it.
    pub title: String,
    /// The design's `.page-sub` line.
    pub sub: String,
    pub actions: Vec<RouteAction>,
    /// The body copy a route with no surface on this host renders.
    pub empty: String,
    pub filter_placeholder: String,
}

impl RoutePage {
    /// The capabilities this route declares, de-duplicated in action order —
    /// the `.tag-row` the route carries.
    pub fn declared_capabilities(&self) -> Vec<RouteCapability> {
        let mut declared = Vec::new();
        for action in &self.actions {
            if !declared.contains(&action.requires) {
                declared.push(action.requires);
            }
        }
        declared
    }
}

/// The actions the workloads route offers, ported from the design: attach a
/// terminal, or create a pod.
const WORKLOADS_ACTIONS: &[RouteAction] = &[
    RouteAction {
        id: "terminal",
        label: "Terminal",
        kind: RouteActionKind::Secondary,
        requires: RouteCapability::Terminal,
    },
    RouteAction {
        id: "new-pod",
        label: "Create Pod",
        kind: RouteActionKind::Primary,
        requires: RouteCapability::Mutations,
    },
];

/// The cluster route's one action: switch the context the console is pointed
/// at (the sidebar cluster button's twin).
const CLUSTER_ACTIONS: &[RouteAction] = &[RouteAction {
    id: "switch-context",
    label: "Switch context",
    kind: RouteActionKind::Secondary,
    requires: RouteCapability::ClusterSwitch,
}];

/// The config route's one action: a new resource of the route's own kind.
const CONFIG_ACTIONS: &[RouteAction] = &[RouteAction {
    id: "new-config-map",
    label: "New ConfigMap",
    kind: RouteActionKind::Primary,
    requires: RouteCapability::Mutations,
}];

/// A route's own words. The design's copy where the mockup has it; the
/// console's own where the mockup leaves the route as a disabled nav entry.
struct RouteCopy {
    title: &'static str,
    sub: &'static str,
    empty: &'static str,
    filter_placeholder: &'static str,
    actions: &'static [RouteAction],
}

fn route_copy(route: &str) -> RouteCopy {
    match route {
        "/" => RouteCopy {
            title: "Cluster",
            sub: "Where this console stands: the cluster it is pointed at, and what this \
                  host can do with it.",
            empty: "This host has no cluster overview on this route.",
            filter_placeholder: "Filter namespaces…",
            // The landing route is the cluster's own: what it offers is the
            // context list, the same affordance the sidebar's cluster button
            // carries.
            actions: CLUSTER_ACTIONS,
        },
        "/cluster" => RouteCopy {
            title: "Cluster",
            sub: "Nodes, versions and the context the console is connected to.",
            empty: "This host has no node inventory on this route.",
            filter_placeholder: "Filter nodes…",
            actions: CLUSTER_ACTIONS,
        },
        "/workloads" => RouteCopy {
            title: "Workloads",
            sub: "Live pod inventory across the cluster. Select a row for pod details, \
                  or filter by namespace below.",
            empty: "This host has no pod inventory on this route.",
            filter_placeholder: "Filter pods…",
            actions: WORKLOADS_ACTIONS,
        },
        "/config" => RouteCopy {
            title: "Config",
            sub: "ConfigMaps, secrets and the storage the cluster serves.",
            empty: "This host has no configuration inventory on this route.",
            filter_placeholder: "Filter config…",
            actions: CONFIG_ACTIONS,
        },
        // A route the chrome does not own (a plugin path reaching the wildcard)
        // still gets a head, an empty body and the declarations, never a blank.
        _ => RouteCopy {
            title: "Route",
            sub: "This route renders from the shared console crate.",
            empty: "This host has no surface for this route.",
            filter_placeholder: "Filter…",
            actions: &[],
        },
    }
}

/// The route's chrome, resolved from the route contract alone — plus the
/// sidebar model for the words the navigation already uses.
pub fn route_page(route: &str, sections: &[ShellSection]) -> RoutePage {
    let route = route_path(route);
    let copy = route_copy(&route);
    let owner = sections.iter().find_map(|section| {
        section
            .items
            .iter()
            .find(|item| item.route == route)
            .map(|item| (section, item))
    });
    let eyebrow = match owner {
        Some((section, _)) if !section.label.is_empty() => Some(section.label.clone()),
        // The home route is not a nav entry: it is the section the console
        // lands in, so the first core section names it.
        None if route == "/" => sections
            .first()
            .map(|section| section.label.clone())
            .filter(|label| !label.is_empty()),
        _ => None,
    };

    RoutePage {
        title: owner
            .map(|(_, item)| item.label.clone())
            .unwrap_or_else(|| copy.title.to_string()),
        route,
        eyebrow,
        sub: copy.sub.to_string(),
        actions: copy.actions.to_vec(),
        empty: copy.empty.to_string(),
        filter_placeholder: copy.filter_placeholder.to_string(),
    }
}

/// Normalize a route string to the path the navigation carries: `/` for the
/// home route, no trailing slash, no surrounding whitespace.
fn route_path(route: &str) -> String {
    let trimmed = route.trim().trim_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        format!("/{trimmed}")
    }
}

/// The route's chrome, mounted by both hosts inside the shell's `.view`.
///
/// `route` is what the host resolved (the URL path); `sections` is the same
/// sidebar model the shell renders; `namespaces` are the filter chips the host
/// can fill (empty renders the declared chip instead of an absent row);
/// `busy` is the host fetching the route's surface; `content` is the host's own
/// surface, and `on_action` what the host does with an action it advertises.
#[component]
pub fn RouteView(
    route: String,
    sections: Vec<ShellSection>,
    #[props(default)] namespaces: Vec<String>,
    #[props(default)] busy: bool,
    #[props(default)] content: Option<Element>,
    #[props(default)] on_action: Option<EventHandler<String>>,
) -> Element {
    let page = route_page(&route, &sections);
    let declared = page.declared_capabilities();
    let filter_unsupported = namespaces.is_empty();
    rsx! {
        div { "data-route": "{page.route}",
            div { class: "page-head", "data-page": "{page.title}",
                div {
                    if let Some(eyebrow) = page.eyebrow.clone() {
                        div { class: "eyebrow", "{eyebrow}" }
                    }
                    h1 { "{page.title}" }
                    p { class: "page-sub", "{page.sub}" }
                }
                div { class: "page-actions",
                    for action in page.actions.iter().copied() {
                        ActionButton { action, on_action }
                    }
                }
            }
            div { class: "toolbar", "data-toolbar": "{page.route}",
                div {
                    class: "chip-row",
                    role: "group",
                    aria_label: "Namespace filter",
                    "data-empty": if filter_unsupported { Some("namespaces") } else { None },
                    if filter_unsupported {
                        button {
                            class: "chip",
                            r#type: "button",
                            disabled: true,
                            "data-unsupported": "namespace-filter",
                            title: "This host has no namespace inventory to filter by.",
                            "No namespaces"
                        }
                    } else {
                        button { class: "chip active", r#type: "button", "data-ns": "all", "All" }
                        for namespace in namespaces.iter().cloned() {
                            button { class: "chip", r#type: "button", "data-ns": "{namespace}", "{namespace}" }
                        }
                    }
                }
                if busy {
                    span { class: "spinner", "data-state": "loading", title: "Loading {page.title}…" }
                }
                label { class: "search-field",
                    input {
                        r#type: "search",
                        placeholder: "{page.filter_placeholder}",
                        aria_label: "{page.filter_placeholder}",
                        disabled: filter_unsupported,
                        "data-unsupported": if filter_unsupported { Some("resource-filter") } else { None },
                    }
                }
            }
            if let Some(content) = content {
                {content}
            } else {
                div { class: "panel", "data-state": "empty",
                    div { class: "table-state", "{page.empty}" }
                    div { class: "panel-footer",
                        span { "data-count": "0", "Showing 0 of 0" }
                        div { class: "pager", aria_label: "Pagination",
                            button { class: "active", r#type: "button", disabled: true, "1" }
                        }
                    }
                }
            }
            div { class: "tag-row", "data-declaration": "capabilities",
                for capability in declared {
                    span {
                        class: "tag",
                        "data-capability": "{capability.id()}",
                        "data-unsupported": if capability.is_satisfied() { None } else { Some(capability.id()) },
                        title: "{capability.missing()}",
                        "{capability_tag(capability)}"
                    }
                }
            }
        }
    }
}

/// One `.page-actions` button: live when the host advertises what it needs and
/// wired it, declared otherwise. Never a dead control.
#[component]
fn ActionButton(
    action: RouteAction,
    #[props(default)] on_action: Option<EventHandler<String>>,
) -> Element {
    let satisfied = action.requires.is_satisfied();
    let wired = on_action.is_some();
    if satisfied && wired {
        let id = action.id;
        let onclick = move |_| {
            if let Some(handler) = on_action.as_ref() {
                handler.call(id.to_string());
            }
        };
        rsx! {
            button {
                class: if action.kind == RouteActionKind::Primary { "btn btn-primary" } else { "btn btn-secondary" },
                r#type: "button",
                "data-action": "{action.id}",
                onclick,
                "{action.label}"
            }
        }
    } else {
        rsx! {
            button {
                class: if action.kind == RouteActionKind::Primary { "btn btn-primary" } else { "btn btn-secondary" },
                r#type: "button",
                disabled: true,
                "data-action": "{action.id}",
                "data-unsupported": if satisfied { "unwired" } else { action.requires.id() },
                title: if satisfied { "This host did not wire this action." } else { action.requires.missing() },
                "{action.label}"
            }
        }
    }
}
