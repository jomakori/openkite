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
//!
//! The plugin wildcard is the same contract (OKT-156): [`PluginRouteView`]
//! renders this chrome around a plugin-owned path, the crate renders the mount
//! node a JS-owned route fills ([`JsRouteMount`]), and the one thing that stays
//! on the host — the `document::eval` that mounts the bundle — arrives as a
//! slot and renders only where the host advertises plugin routes.

use dioxus::prelude::*;

use crate::components::namespace_bar::NamespaceBar;
use crate::runtime::{
    cluster_switch_can_render, mutations_can_render, plugin_route_can_render, terminal_can_render,
};
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
    /// The plugin route surface: a host that serves plugin bundles, so a
    /// plugin-owned path has something to render.
    PluginRoute,
}

impl RouteCapability {
    /// Every capability a route can declare, in declaration order.
    pub const ALL: [RouteCapability; 4] = [
        RouteCapability::Terminal,
        RouteCapability::Mutations,
        RouteCapability::ClusterSwitch,
        RouteCapability::PluginRoute,
    ];

    /// What the host reports. The predicates live in [`crate::runtime`] so the
    /// route chrome, the shell and the terminal route never disagree.
    pub fn is_satisfied(self) -> bool {
        match self {
            RouteCapability::Terminal => terminal_can_render(),
            RouteCapability::Mutations => mutations_can_render(),
            RouteCapability::ClusterSwitch => cluster_switch_can_render(),
            RouteCapability::PluginRoute => plugin_route_can_render(),
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
            RouteCapability::PluginRoute => "plugin-route",
        }
    }

    /// The capability in the chips' monospace voice (`.tag`).
    pub fn label(self) -> &'static str {
        match self {
            RouteCapability::Terminal => "terminal",
            RouteCapability::Mutations => "cluster changes",
            RouteCapability::ClusterSwitch => "context switching",
            RouteCapability::PluginRoute => "plugin routes",
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
            RouteCapability::PluginRoute => {
                "This host serves no plugin bundles: a plugin route has nothing to render here."
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
    /// Capabilities the route declares on its own, with no action of its own
    /// carrying them — the plugin wildcard needs a host that serves plugin
    /// bundles before any plugin route can render at all.
    pub declares: Vec<RouteCapability>,
    /// The body copy a route with no surface on this host renders.
    pub empty: String,
    pub filter_placeholder: String,
}

impl RoutePage {
    /// The capabilities this route declares — the ones it states itself, then
    /// the ones its actions require, de-duplicated in declaration order: the
    /// `.tag-row` the route carries.
    pub fn declared_capabilities(&self) -> Vec<RouteCapability> {
        let mut declared = self.declares.clone();
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
    declares: &'static [RouteCapability],
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
            declares: &[],
        },
        "/cluster" => RouteCopy {
            title: "Cluster",
            sub: "Nodes, versions and the context the console is connected to.",
            empty: "This host has no node inventory on this route.",
            filter_placeholder: "Filter nodes…",
            actions: CLUSTER_ACTIONS,
            declares: &[],
        },
        "/workloads" => RouteCopy {
            title: "Workloads",
            sub: "Live pod inventory across the cluster. Select a row for pod details, \
                  or filter by namespace below.",
            empty: "This host has no pod inventory on this route.",
            filter_placeholder: "Filter pods…",
            actions: WORKLOADS_ACTIONS,
            declares: &[],
        },
        "/config" => RouteCopy {
            title: "Config",
            sub: "ConfigMaps, secrets and the storage the cluster serves.",
            empty: "This host has no configuration inventory on this route.",
            filter_placeholder: "Filter config…",
            actions: CONFIG_ACTIONS,
            declares: &[],
        },
        // A route the chrome does not own (a plugin path reaching the wildcard)
        // still gets a head, an empty body and the declarations, never a blank.
        // The plugin route declares the one thing it needs of its host — a
        // plugin bundle surface — because a host without one has nothing to
        // mount there (OKT-156).
        _ => RouteCopy {
            title: "Route",
            sub: "This route renders from the shared console crate.",
            empty: "This host has no surface for this route.",
            filter_placeholder: "Filter…",
            actions: &[],
            declares: &[RouteCapability::PluginRoute],
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
        declares: copy.declares.to_vec(),
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

/// The route a request path resolves to: the path the chrome owns, else `/`.
pub fn resolve_route(route: &str, sections: &[ShellSection]) -> String {
    let route = route_path(route);
    let primary = matches!(route.as_str(), "/" | "/cluster" | "/workloads" | "/config");
    let navigable = sections
        .iter()
        .any(|section| section.items.iter().any(|item| item.route == route));
    if primary || navigable {
        route
    } else {
        "/".to_string()
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
                NamespaceBar { options: namespaces.clone() }
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

/// The mount node a JS-owned plugin route renders into (OKT-156).
///
/// The node and the `[data-js-route-mount]` contract it carries are the crate's:
/// the host's evaluator looks the node up and hands it to the bundle's
/// `_renderRoute`, so the plugin's markup lands inside the console's route
/// outlet rather than in an overlay of its own. `display: contents` keeps the
/// plugin's boxes where the design put the view. The crate owns the node; the
/// host owns the eval that fills it.
#[component]
pub fn JsRouteMount(path: String) -> Element {
    rsx! {
        div { class: "js-route-slot", "data-js-route-mount": "{path}" }
    }
}

/// The plugin route's chrome (OKT-156).
///
/// The wildcard route is owned by a plugin, and only one half of serving it is
/// host machinery: the `document::eval` that mounts a JS bundle into the
/// webview. That half arrives here as [`PluginRouteView`]'s `evaluator` slot and
/// renders only where the host advertises plugin routes
/// ([`crate::runtime::plugin_route_can_render`]). Everything else is this
/// crate's: the [`RouteView`] chrome, the mount node the evaluator fills
/// ([`JsRouteMount`]), and — on a host with no plugin surface at all — the
/// explicit unsupported body below, instead of a slot nothing would ever fill.
///
/// `route` is the path the host resolved and `sections` the sidebar model the
/// chrome's words come from. `js_route` is the path a JS-owned renderer is
/// registered at, per the host's own plugin table, and `evaluator` the host's
/// evaluator for it: the desktop passes both for a JS-owned route, the browser
/// console neither. A host that advertises plugin routes but has no renderer for
/// the path keeps the chrome's declared empty body — the same shape the
/// desktop's SDK routes use when they mount the plugin's own view.
#[component]
pub fn PluginRouteView(
    route: String,
    sections: Vec<ShellSection>,
    #[props(default)] namespaces: Vec<String>,
    #[props(default)] js_route: Option<String>,
    #[props(default)] evaluator: Option<Element>,
    #[props(default)] on_action: Option<EventHandler<String>>,
) -> Element {
    let page = route_page(&route, &sections);
    if !plugin_route_can_render() {
        let missing = RouteCapability::PluginRoute.missing();
        return rsx! {
            RouteView {
                route: page.route,
                sections,
                namespaces,
                on_action,
                content: rsx! {
                    div {
                        class: "panel",
                        "data-state": "unsupported",
                        "data-unsupported": "plugin-route",
                        div { class: "table-state", title: "{missing}", "{missing}" }
                    }
                },
            }
        };
    }

    // The host has the surface: the crate renders the mount node, the host's
    // evaluator fills it. A path with no renderer on this host falls through to
    // the chrome's declared empty body.
    let content = match (js_route, evaluator) {
        (Some(path), Some(evaluator)) => Some(rsx! {
            JsRouteMount { path }
            {evaluator}
        }),
        _ => None,
    };
    rsx! {
        RouteView { route: page.route, sections, namespaces, content, on_action }
    }
}
