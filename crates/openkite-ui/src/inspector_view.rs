//! Inspector slide-over: header, tab strip and the seven panels.
//!
//! Markup follows the console prototype's `inspector` markup and class names (`insp-tabs`,
//! `kv-list`, `chip-list`, `ctr-card`, `gauge-row`, `inst-list`, `ev-list`, `yaml`,
//! `qa-row`). Rules such as which tabs a kind has live in [`crate::inspector`]; this module
//! only renders them and wires events.

use dioxus::prelude::*;

use crate::inspector::{
    action_bar, default_schema, Gauge, InspectorData, InspectorState, Manifest, QuickAction,
    SchemaNode, Tab,
};

#[derive(Props, Clone, PartialEq)]
pub struct InspectorProps {
    /// Object name shown in the header.
    pub name: String,
    /// Singular kind label, e.g. `Pod`.
    pub kind_label: String,
    /// Catalogue kind key, e.g. `pods`. Drives the tab set and the action bar.
    pub kind: String,
    pub namespace: Option<String>,
    pub state: InspectorState,
    pub data: InspectorData,
    pub on_close: EventHandler<()>,
    pub on_tab: EventHandler<Tab>,
    pub on_action: EventHandler<String>,
    /// Follow a link from the Resources panel.
    pub on_goto: EventHandler<String>,
}

/// Icon from the sprite.
fn icon(name: &'static str) -> Element {
    let href = format!("#i-{name}");
    rsx! {
        svg { class: "icon", use { href: "{href}" } }
    }
}

fn manifest_or_placeholder(manifest: Option<&Manifest>) -> Element {
    match manifest {
        Some(manifest) => rsx! {
            div { class: "yaml-bar",
                span { class: "mono", "{manifest.line_count()} lines" }
                span { class: "mono muted", "read only" }
            }
            pre { class: "yaml", "data-yaml": "loaded", "{manifest.text}" }
        },
        None => rsx! {
            div { class: "yaml-bar",
                span { class: "mono muted", "Fetching manifest…" }
            }
            pre { class: "yaml", "data-yaml": "pending", aria_busy: "true", "" }
        },
    }
}

fn gauge_row(gauge: &Gauge) -> Element {
    let pct = gauge.pct.min(100);
    rsx! {
        div { class: "meter",
            div { class: "meter-head",
                span { "{gauge.label}" }
                span { class: "mono", "{gauge.value}" }
            }
            div { class: "meter-track", span { class: "meter-fill", style: "width:{pct}%" } }
        }
    }
}

fn overview_panel(data: &InspectorData) -> Element {
    let detail = &data.detail;
    rsx! {
        div { class: "kv-list", "data-panel": "overview",
            for (key , value) in detail.props.iter() {
                div { class: "kv-row", dt { "{key}" } dd { class: "mono", "{value}" } }
            }
        }
        if !detail.conditions.is_empty() {
            div { class: "insp-eyebrow", "Conditions" }
            ul { class: "cond-list",
                for (name , ok) in detail.conditions.iter() {
                    li { class: if *ok { "cond ok" } else { "cond warn" },
                        span { class: "dot" }
                        span { class: "mono", "{name}" }
                        span { class: "muted", if *ok { "True" } else { "False" } }
                    }
                }
            }
        }
        if !data.gauges.is_empty() {
            div { class: "insp-eyebrow", "Usage" }
            div { class: "gauge-row", for gauge in data.gauges.iter() { {gauge_row(gauge)} } }
        }
        if !detail.labels.is_empty() {
            div { class: "insp-eyebrow", "Labels" }
            div { class: "chip-list",
                for (key , value) in detail.labels.iter() {
                    span { class: "label-chip", b { "{key}" } "={value}" }
                }
            }
        }
        if !detail.annotations.is_empty() {
            div { class: "insp-eyebrow", "Annotations" }
            div { class: "kv-list",
                for (key , value) in detail.annotations.iter() {
                    div { class: "kv-row", dt { "{key}" } dd { class: "mono", "{value}" } }
                }
            }
        }
    }
}

fn containers_panel(data: &InspectorData) -> Element {
    rsx! {
        div { "data-panel": "containers",
            if data.containers.is_empty() {
                p { class: "empty-hint", "This object has no containers." }
            }
            for ctr in data.containers.iter() {
                div { class: "ctr-card",
                    div { class: "ctr-head",
                        span { class: "ctr-name mono", "{ctr.name}" }
                        span { class: "muted mono", "{ctr.restarts} restarts" }
                    }
                    div { class: "kv-list",
                        div { class: "kv-row", dt { "Image" } dd { class: "mono", "{ctr.image}" } }
                        div { class: "kv-row", dt { "Ports" } dd { class: "mono", "{ctr.ports}" } }
                        div { class: "kv-row", dt { "Requests" } dd { class: "mono", "{ctr.requests}" } }
                        div { class: "kv-row", dt { "Limits" } dd { class: "mono", "{ctr.limits}" } }
                        div { class: "kv-row", dt { "Liveness" } dd { class: "mono", "{ctr.liveness}" } }
                        div { class: "kv-row", dt { "Mounts" } dd { class: "mono", "{ctr.mounts}" } }
                    }
                }
            }
        }
    }
}

fn metrics_panel(data: &InspectorData) -> Element {
    rsx! {
        div { "data-panel": "metrics",
            div { class: "gauge-row",
                for gauge in data.gauges.iter() {
                    {gauge_row(gauge)}
                }
            }
        }
    }
}

fn resources_panel(data: &InspectorData, on_goto: EventHandler<String>) -> Element {
    rsx! {
        div { "data-panel": "resources",
            if data.resources.is_empty() {
                p { class: "empty-hint", "Nothing references this object." }
            }
            ul { class: "inst-list",
                for link in data.resources.iter() {
                    li {
                        button {
                            r#type: "button",
                            class: "inst-link",
                            "data-goto": "{link.id}",
                            onclick: {
                                let id = link.id.clone();
                                move |_| on_goto.call(id.clone())
                            },
                            span { class: "mono", "{link.label}" }
                            if let Some(status) = link.status.clone() {
                                span { class: "muted", "{status}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn schema_nodes(nodes: &[SchemaNode], depth: usize) -> Element {
    rsx! {
        for node in nodes.iter() {
            div { class: "schema-row", "data-depth": "{depth}",
                span { class: "mono schema-name", "{node.name}" }
                span { class: "mono muted", "{node.ty}" }
                span { class: "schema-desc", "{node.description}" }
            }
            {schema_children(&node.children, depth + 1)}
        }
    }
}

/// Recursion helper kept out of `rsx!` so the nesting stays readable.
fn schema_children(nodes: &[SchemaNode], depth: usize) -> Element {
    if nodes.is_empty() {
        return rsx! {};
    }
    schema_nodes(nodes, depth)
}

fn schema_panel(data: &InspectorData) -> Element {
    let owned;
    let nodes = if data.schema.is_empty() {
        owned = default_schema();
        &owned[..]
    } else {
        &data.schema[..]
    };
    rsx! {
        div { "data-panel": "schema", {schema_nodes(nodes, 0)} }
    }
}

fn events_panel(data: &InspectorData) -> Element {
    rsx! {
        div { "data-panel": "events",
            if data.events.is_empty() {
                p { class: "empty-hint", "No events in the last hour." }
            }
            ul { class: "ev-list",
                for event in data.events.iter() {
                    li { class: if event.kind == "Warning" { "ev warn" } else { "ev" },
                        span { class: "dot" }
                        span { class: "ev-reason mono", "{event.reason}" }
                        span { class: "ev-msg", "{event.message}" }
                        span { class: "muted mono", "{event.age}" }
                    }
                }
            }
        }
    }
}

fn panel(tab: Tab, data: &InspectorData, on_goto: EventHandler<String>) -> Element {
    let body = match tab {
        Tab::Overview => overview_panel(data),
        Tab::Containers => containers_panel(data),
        Tab::Metrics => metrics_panel(data),
        Tab::Resources => resources_panel(data, on_goto),
        Tab::Schema => schema_panel(data),
        Tab::Yaml => {
            rsx! { div { "data-panel": "yaml", {manifest_or_placeholder(data.manifest.as_ref())} } }
        }
        Tab::Events => events_panel(data),
    };
    rsx! {
        div { class: "insp-panel", role: "tabpanel", id: "insp-panel-{tab.id()}", {body} }
    }
}

fn action_menu_item(action: QuickAction, on_action: EventHandler<String>) -> Element {
    let class = if action.danger {
        "menu-item danger"
    } else {
        "menu-item"
    };
    rsx! {
        li { role: "none",
            button {
                r#type: "button",
                role: "menuitem",
                class: "{class}",
                "data-act": "{action.id}",
                onclick: move |_| on_action.call(action.id.to_string()),
                {icon(action.icon)}
                span { "{action.label}" }
            }
        }
    }
}

fn action_button(action: QuickAction, on_action: EventHandler<String>) -> Element {
    let class = if action.primary() {
        "btn btn-primary"
    } else if action.danger {
        "btn btn-danger"
    } else {
        "btn"
    };
    rsx! {
        button {
            r#type: "button",
            class: "{class}",
            "data-act": "{action.id}",
            onclick: move |_| on_action.call(action.id.to_string()),
            {icon(action.icon)}
            span { "{action.label}" }
        }
    }
}

#[allow(non_snake_case)]
pub fn Inspector(props: InspectorProps) -> Element {
    let tabs = crate::inspector::tabs_for(&props.kind);
    let active = props.state.tab;
    let open = props.state.is_open();
    let bar = action_bar(&props.kind);
    let on_action = props.on_action;
    let left_buttons: Vec<Element> = bar
        .left
        .iter()
        .map(|action| action_button(*action, on_action))
        .collect();
    let right_buttons: Vec<Element> = bar
        .right
        .iter()
        .map(|action| action_button(*action, on_action))
        .collect();
    let menu_items: Vec<Element> = bar
        .menu
        .iter()
        .map(|action| action_menu_item(*action, on_action))
        .collect();
    let on_tab = props.on_tab;
    let on_close = props.on_close;
    let on_goto = props.on_goto;
    let class = if open { "inspector open" } else { "inspector" };
    let ns = props.namespace.clone().unwrap_or_else(|| "—".to_string());

    rsx! {
        button {
            r#type: "button",
            class: if open { "inspector-scrim open" } else { "inspector-scrim" },
            "data-part": "scrim",
            aria_label: "Close the inspector",
            onclick: move |_| on_close.call(()),
        }
        aside {
            class: "{class}",
            id: "inspector",
            "data-open": "{open}",
            "aria-hidden": if open { "false" } else { "true" },
            div { class: "insp-handle", aria_hidden: "true" }
            header { class: "inspector-header",
                div { class: "insp-title",
                    span { class: "insp-eyebrow mono", "{ns}" }
                    h2 { id: "inspectorName", class: "mono", "{props.name}" }
                    span { id: "inspectorKind", class: "muted", "{props.kind_label}" }
                }
                div { class: "insp-tools",
                    button {
                        r#type: "button",
                        class: "btn btn-ghost",
                        "data-part": "copy-name",
                        title: "Copy the object name",
                        onclick: move |_| on_action.call("copy-name".to_string()),
                        {icon("copy")}
                        span { "Copy" }
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-ghost btn-danger",
                        "data-part": "delete",
                        title: "Delete this object",
                        onclick: move |_| on_action.call("delete".to_string()),
                        {icon("trash")}
                        span { "Delete" }
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-ghost",
                        "data-part": "close",
                        aria_label: "Close",
                        onclick: move |_| on_close.call(()),
                        {icon("close")}
                    }
                }
            }
            div { class: "qa-row",
                div { class: "qa-left",
                    for button in left_buttons { {button} }
                }
                div { class: "qa-right",
                    for button in right_buttons { {button} }
                    if !bar.menu.is_empty() {
                        div {
                            class: if props.state.menu_open { "menu-wrap open" } else { "menu-wrap" },
                            button {
                                r#type: "button",
                                class: "btn btn-ghost",
                                "data-part": "more",
                                aria_haspopup: "menu",
                                "aria-expanded": if props.state.menu_open { "true" } else { "false" },
                                onclick: move |_| on_action.call("more".to_string()),
                                {icon("kebab")}
                            }
                            ul { class: "menu", role: "menu", "data-menu": "actions",
                                for item in menu_items { {item} }
                            }
                        }
                    }
                }
            }
            div { class: "insp-tabs", role: "tablist", "aria-label": "Inspector sections",
                for tab in tabs.iter().copied() {
                    button {
                        r#type: "button",
                        class: if tab == active { "insp-tab active" } else { "insp-tab" },
                        role: "tab",
                        id: "insp-tab-{tab.id()}",
                        "data-tab": "{tab.id()}",
                        "aria-selected": if tab == active { "true" } else { "false" },
                        "aria-controls": "insp-panel-{tab.id()}",
                        onclick: move |_| on_tab.call(tab),
                        "{tab.label()}"
                    }
                }
            }
            div { class: "inspector-body",
                {panel(active, &props.data, on_goto)}
            }
        }
    }
}
