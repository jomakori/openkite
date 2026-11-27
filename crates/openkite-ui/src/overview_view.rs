//! Overview board render layer: bands, cards, the edit bar and the widget tray.
//!
//! Layout rules live in [`crate::overview`]; this module only turns a layout plus
//! fetched rows into markup and reports what the user clicked.

use dioxus::prelude::*;

use crate::overview::{
    Attention, BoardAction, Group, Lane, OverviewData, OverviewLayout, Size, UsagePanel, WidgetId,
};

#[derive(Props, Clone, PartialEq)]
pub struct OverviewProps {
    pub layout: OverviewLayout,
    pub data: OverviewData,
    pub namespaces: Vec<String>,
    pub on_action: EventHandler<BoardAction>,
}

#[component]
pub fn Overview(props: OverviewProps) -> Element {
    let action = props.on_action;
    let layout = props.layout.clone();
    let data = props.data.clone();
    let editing = layout.editing;
    let groups = layout.groups();

    rsx! {
        section { class: "overview", "aria-label": "Cluster overview",
            if !props.namespaces.is_empty() {
                div { class: "ov-ns-note", role: "status",
                    span {
                        "Showing namespaces: "
                        b { "{props.namespaces.join(\", \")}" }
                    }
                    button {
                        r#type: "button",
                        class: "icon-btn x-clear",
                        "aria-label": "Clear namespace filter",
                        onclick: move |_| action.call(BoardAction::ClearNamespaces),
                        svg { class: "icon", use { href: "#i-close" } }
                    }
                }
            }
            div { class: "overview-head",
                h1 { "Cluster overview" }
                p { class: "overview-meta", "{data.cluster} · {data.pods_total} pods" }
                button {
                    r#type: "button",
                    class: "btn btn-secondary",
                    id: "ovCustomize",
                    "aria-pressed": if editing { "true" } else { "false" },
                    onclick: move |_| action.call(BoardAction::ToggleEdit),
                    svg { class: "icon", use { href: "#i-settings" } }
                    span { if editing { "Done" } else { "Customize" } }
                }
            }
            if editing {
                div { class: "ov-edit-bar", role: "status",
                    span {
                        b { "Customizing overview." }
                        " Reorder, resize or hide widgets. Changes are saved on this device."
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-secondary",
                        "data-act": "reset",
                        onclick: move |_| action.call(BoardAction::Reset),
                        "Reset to default"
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-primary",
                        "data-act": "done",
                        onclick: move |_| action.call(BoardAction::ToggleEdit),
                        "Done"
                    }
                }
            }
            div { class: if editing { "ov-widgets editing" } else { "ov-widgets" },
                for group in groups {
                    match group {
                        Group::Full(id) => rsx! {
                            {card(id, &layout, &data, editing, action)}
                        },
                        Group::Band { left, right } => rsx! {
                            div { class: "ov-band", "data-band": "true",
                                for (lane, ids) in [(Lane::Left, left), (Lane::Right, right)] {
                                    div { class: "ov-lane", "data-lane": "{lane.id()}",
                                        for id in ids {
                                            {card(id, &layout, &data, editing, action)}
                                        }
                                        div { class: "ov-lane-end", "aria-hidden": "true" }
                                    }
                                }
                            }
                        },
                    }
                }
            }
            if editing {
                div { class: "ov-tray",
                    h3 { "Available widgets" }
                    if layout.hidden.is_empty() {
                        p { class: "muted-note", "Every widget is on your overview." }
                    } else {
                        for id in layout.hidden.clone() {
                            div { class: "ov-tray-item", key: "{id.id()}",
                                div {
                                    b { "{id.title()}" }
                                    small { "{id.description()}" }
                                }
                                button {
                                    r#type: "button",
                                    class: "btn btn-secondary",
                                    "data-ovw": "add",
                                    "data-id": "{id.id()}",
                                    onclick: move |_| action.call(BoardAction::Show(id)),
                                    svg { class: "icon", use { href: "#i-plus" } }
                                    "Add"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One card: grip, edit bar and widget body.
fn card(
    id: WidgetId,
    layout: &OverviewLayout,
    data: &OverviewData,
    editing: bool,
    action: EventHandler<BoardAction>,
) -> Element {
    let size = layout.size(id);
    rsx! {
        section {
            class: if size == Size::Full { "ov-w full" } else { "ov-w half" },
            "data-w": "{id.id()}",
            "data-pref": "{size.id()}",
            button {
                r#type: "button",
                class: "ov-grip",
                "data-grip": "{id.id()}",
                "aria-label": "Reorder {id.title()}. Arrow keys move it.",
                title: "Drag to reorder",
                svg { view_box: "0 0 10 18", "aria-hidden": "true",
                    g { fill: "currentColor",
                        circle { cx: "2.5", cy: "3", r: "1.3" }
                        circle { cx: "7.5", cy: "3", r: "1.3" }
                        circle { cx: "2.5", cy: "9", r: "1.3" }
                        circle { cx: "7.5", cy: "9", r: "1.3" }
                        circle { cx: "2.5", cy: "15", r: "1.3" }
                        circle { cx: "7.5", cy: "15", r: "1.3" }
                    }
                }
            }
            div { class: "ov-body",
                if editing {
                    div { class: "ov-w-bar",
                        span { class: "ov-w-title", "{id.title()}" }
                        button {
                            r#type: "button",
                            class: "ovb",
                            "data-ovw": "up",
                            "data-id": "{id.id()}",
                            "aria-label": "Move {id.title()} up",
                            onclick: move |_| action.call(BoardAction::Move { id, delta: -1 }),
                            svg { class: "icon", style: "transform:rotate(-90deg)", use { href: "#i-chevron" } }
                        }
                        button {
                            r#type: "button",
                            class: "ovb",
                            "data-ovw": "down",
                            "data-id": "{id.id()}",
                            "aria-label": "Move {id.title()} down",
                            onclick: move |_| action.call(BoardAction::Move { id, delta: 1 }),
                            svg { class: "icon", style: "transform:rotate(90deg)", use { href: "#i-chevron" } }
                        }
                        if size == Size::Half {
                            button {
                                r#type: "button",
                                class: "ovb",
                                "data-ovw": "lane",
                                "data-id": "{id.id()}",
                                "aria-label": "Move {id.title()} to the other column",
                                title: "Other column",
                                onclick: move |_| action.call(BoardAction::SwapLane(id)),
                                svg { class: "icon", use { href: "#i-swap" } }
                            }
                        }
                        button {
                            r#type: "button",
                            class: "ovb txt",
                            "data-ovw": "size",
                            "data-id": "{id.id()}",
                            "aria-label": "Toggle width of {id.title()}",
                            onclick: move |_| action.call(BoardAction::ToggleSize(id)),
                            if size == Size::Full { "Full" } else { "Half" }
                        }
                        button {
                            r#type: "button",
                            class: "ovb",
                            "data-ovw": "hide",
                            "data-id": "{id.id()}",
                            "aria-label": "Hide {id.title()}",
                            onclick: move |_| action.call(BoardAction::Hide(id)),
                            svg { class: "icon", use { href: "#i-close" } }
                        }
                    }
                }
                {body(id, data, layout.threshold, action)}
            }
        }
    }
}

fn body(
    id: WidgetId,
    data: &OverviewData,
    threshold: u32,
    action: EventHandler<BoardAction>,
) -> Element {
    match id {
        WidgetId::Attention => attention(data, action),
        WidgetId::Usage => usage(data, action),
        WidgetId::Problems => problems(data, action),
        WidgetId::Restarts => restarts(data, threshold, action),
        WidgetId::Capacity => capacity(data),
        WidgetId::Nodes => nodes(data, action),
        WidgetId::Health => health(data, action),
        WidgetId::Argo => argo(data, action),
        WidgetId::Events => events(data, action),
    }
}

fn panel(id: WidgetId, head: Option<Element>, body: Element) -> Element {
    rsx! {
        div { class: "panel", "data-od-id": "overview-{id.id()}",
            div { class: "panel-head",
                h3 { "{id.title()}" }
                {head}
            }
            {body}
        }
    }
}

fn all_clear(message: &str) -> Element {
    rsx! {
        div { class: "all-clear",
            svg { class: "icon", use { href: "#i-check" } }
            span { "{message}" }
        }
    }
}

fn attention(data: &OverviewData, action: EventHandler<BoardAction>) -> Element {
    let att: Attention = data.attention(3);
    rsx! {
        {panel(
            WidgetId::Attention,
            None,
            rsx! {
                div { class: "att-grid",
                    {tile(att.failing, "Failing pods", "danger", "failing", action)}
                    {tile(att.pending, "Pending pods", "warn", "pending", action)}
                    {tile(att.restarts, "Restarts ≥ 3", "warn", "restarts", action)}
                    {tile(att.degraded, "Degraded workloads", "danger", "degraded", action)}
                    {tile(att.nodes, "Nodes not ready", "warn", "nodes", action)}
                    {tile(att.argo, "Apps out of sync", "warn", "argo", action)}
                    {tile(att.warnings, "Warning events", "warn", "warnings", action)}
                }
            },
        )}
    }
}

fn tile(
    count: usize,
    label: &'static str,
    tone: &'static str,
    act: &'static str,
    action: EventHandler<BoardAction>,
) -> Element {
    let class = if count == 0 {
        "att-tile ok".to_string()
    } else {
        format!("att-tile {tone}")
    };
    rsx! {
        button {
            r#type: "button",
            class: "{class}",
            "data-act": "{act}",
            onclick: move |_| action.call(BoardAction::OpenKind(act)),
            span { class: "att-n", "{count}" }
            span { class: "att-l", "{label}" }
        }
    }
}

fn usage(data: &OverviewData, action: EventHandler<BoardAction>) -> Element {
    if data.pods_total == 0 {
        return panel(
            WidgetId::Usage,
            None,
            all_clear("No pods in the selected namespaces"),
        );
    }
    rsx! {
        UsageCard { data: data.clone(), action }
    }
}

/// Chrome for the usage panel: the title toggle that expands and collapses the
/// body on its own, an optional trailing tool, and the body itself.
#[component]
pub fn UsagePanelShell(
    open: bool,
    trailing: Element,
    on_toggle: EventHandler<()>,
    children: Element,
) -> Element {
    let state = if open { "true" } else { "false" };
    let label = if open {
        "Collapse usage"
    } else {
        "Expand usage"
    };
    rsx! {
        div { class: "panel summary-panel",
            "data-od-id": "overview-usage",
            "data-open": state,
            div { class: "panel-head",
                button {
                    r#type: "button",
                    class: "sum-toggle",
                    "data-sum": "usage",
                    "aria-expanded": state,
                    "aria-controls": "overview-usage-body",
                    "aria-label": "{label}",
                    onclick: move |_| on_toggle.call(()),
                    svg { class: "icon", "aria-hidden": "true", use { href: "#i-chevron" } }
                    "Usage"
                }
                {trailing}
            }
            div { class: "panel-body", id: "overview-usage-body", {children} }
        }
    }
}

/// The usage gauges. Each one is a real `<button>`, so pointer, Enter and Space
/// take the same path, and the label names the detail it opens.
#[component]
pub fn UsageGauges(
    gauges: Vec<(&'static str, usize, &'static str)>,
    on_open: EventHandler<&'static str>,
) -> Element {
    rsx! {
        div { class: "gauge-row",
            for (label, pct, series) in gauges {
                button {
                    r#type: "button",
                    class: "meter",
                    "data-metric": "{series}",
                    "aria-label": "{label} usage, {pct}%. Opens the {label} detail.",
                    onclick: move |_| on_open.call(series),
                    span { class: "meter-head",
                        span { "{label}" }
                        span { class: "mono", "{pct}%" }
                    }
                    span { class: "meter-track", span { class: "meter-fill", style: "width:{pct}%" } }
                }
            }
        }
    }
}

/// The drill-down a gauge opens: the series level and the nodes carrying it.
#[component]
pub fn MetricDetail(
    series: &'static str,
    pct: usize,
    contributors: Vec<(String, u8)>,
    more: usize,
    on_back: EventHandler<()>,
) -> Element {
    let label = if series == "cpu" { "CPU" } else { "Memory" };
    rsx! {
        div { class: "metric-detail", "data-metric": "{series}",
            div { class: "metric-detail-head",
                button {
                    r#type: "button",
                    class: "metric-back",
                    "aria-label": "Back to usage",
                    onclick: move |_| on_back.call(()),
                    svg { class: "icon", "aria-hidden": "true", use { href: "#i-chevron" } }
                }
                h4 { "{label} usage" }
                span { class: "mono metric-detail-val", "{pct}%" }
            }
            if contributors.is_empty() {
                div { class: "empty-hint", "No node samples yet" }
            } else {
                div { class: "insp-eyebrow", "By node" }
                dl { class: "kv-list",
                    for (name, value) in contributors {
                        div { class: "kv-row",
                            dt { "{name}" }
                            dd { class: "mono", "{value}%" }
                        }
                    }
                }
                if more > 0 {
                    div { class: "empty-hint", "and {more} more" }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct UsageCardProps {
    data: OverviewData,
    action: EventHandler<BoardAction>,
}

/// The usage panel. Panel state lives in [`UsagePanel`], which the workload
/// tree never reads: collapsing this panel cannot fold the list, and folding
/// the list cannot collapse this panel.
#[component]
fn UsageCard(props: UsageCardProps) -> Element {
    let data = props.data;
    let action = props.action;
    let mut panel_state = use_signal(UsagePanel::default);
    let cpu = data.counts.get("cpu_pct").copied().unwrap_or(0);
    let mem = data.counts.get("mem_pct").copied().unwrap_or(0);
    let (open, detail) = {
        let panel = panel_state.read();
        let detail = if panel.shows_detail() {
            panel.metric()
        } else {
            None
        };
        (panel.is_open(), detail)
    };
    let mut contributors = Vec::new();
    let mut more = 0;
    if let Some(series) = detail {
        let mut rows: Vec<(String, u8)> = data
            .nodes
            .iter()
            .map(|node| {
                let value = if series == "cpu" {
                    node.cpu_pct
                } else {
                    node.mem_pct
                };
                (node.name.clone(), value)
            })
            .collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        more = rows.len().saturating_sub(5);
        rows.truncate(5);
        contributors = rows;
    }
    let body = match detail {
        Some(series) => rsx! {
            MetricDetail {
                series,
                pct: if series == "cpu" { cpu } else { mem },
                contributors,
                more,
                on_back: move |_| panel_state.write().close_metric(),
            }
        },
        None => rsx! {
            UsageGauges {
                gauges: vec![("CPU", cpu, "cpu"), ("Memory", mem, "mem")],
                on_open: move |series| panel_state.write().open_metric(series),
            }
        },
    };
    rsx! {
        UsagePanelShell {
            open,
            trailing: rsx! {
                button {
                    r#type: "button",
                    class: "link-back",
                    "data-act": "usage-all",
                    onclick: move |_| action.call(BoardAction::OpenKind("nodes")),
                    "View all"
                    svg { class: "icon", "aria-hidden": "true", use { href: "#i-arrow" } }
                }
            },
            on_toggle: move |_| panel_state.write().toggle(),
            {body}
        }
    }
}

fn problems(data: &OverviewData, action: EventHandler<BoardAction>) -> Element {
    let rows = data.problems.clone();
    let head = (!rows.is_empty()).then(|| {
        rsx! {
            button {
                r#type: "button",
                class: "link-back",
                "data-act": "problems-all",
                onclick: move |_| action.call(BoardAction::OpenKind("problems-all")),
                "View all "
                svg { class: "icon", use { href: "#i-arrow" } }
            }
        }
    });
    if rows.is_empty() {
        return panel(
            WidgetId::Problems,
            None,
            all_clear("No pending or failing pods"),
        );
    }
    rsx! {
        {panel(
            WidgetId::Problems,
            head,
            rsx! {
                div { class: "prob-list",
                    for pod in rows {
                        button {
                            r#type: "button",
                            class: "prob-row",
                            "data-goto": "{pod.id}",
                            onclick: move |_| action.call(BoardAction::Goto(pod.id.clone())),
                            span { class: "pill warn", "{pod.status}" }
                            span { class: "mono", "{pod.name}" }
                            span { class: "prob-ns", "{pod.namespace}" }
                            span { class: "prob-r", title: "Restarts", "↻ {pod.restarts}" }
                        }
                    }
                }
            },
        )}
    }
}

fn restarts(data: &OverviewData, threshold: u32, action: EventHandler<BoardAction>) -> Element {
    let rows = data.restarts.clone();
    let max = rows.first().map(|r| r.restarts).unwrap_or(1).max(1);
    let head = Some(rsx! {
        label { class: "thr",
            span { "Restarts ≥" }
            select {
                "data-ov-thr": "true",
                "aria-label": "Restart threshold",
                onchange: move |evt| {
                    if let Ok(value) = evt.value().parse::<u32>() {
                        action.call(BoardAction::Threshold(value));
                    }
                },
                for n in [1u32, 3, 5, 10] {
                    option { value: "{n}", selected: n == threshold, "{n}" }
                }
            }
        }
    });
    if rows.is_empty() {
        return panel(
            WidgetId::Restarts,
            head,
            all_clear("No pod has restarted that often"),
        );
    }
    rsx! {
        {panel(
            WidgetId::Restarts,
            head,
            rsx! {
                div { class: "rank-list",
                    for pod in rows {
                        button {
                            r#type: "button",
                            class: "rank-row",
                            "data-goto": "{pod.id}",
                            onclick: move |_| action.call(BoardAction::Goto(pod.id.clone())),
                            span { class: "mono", "{pod.name}" }
                            span { class: "hbar",
                                span { class: "track",
                                    span {
                                        class: "fill danger",
                                        style: "width:{((pod.restarts.max(1) as f32 / max as f32) * 100.0).max(6.0):.0}%",
                                    }
                                }
                                span { class: "num", "{pod.restarts}×" }
                            }
                        }
                    }
                }
            },
        )}
    }
}

fn capacity(data: &OverviewData) -> Element {
    rsx! {
        {panel(
            WidgetId::Capacity,
            None,
            rsx! {
                div { class: "panel-body",
                    div { class: "gauge-row",
                        for gauge in data.capacity.clone() {
                            div { class: "meter",
                                span { class: "meter-head",
                                    span { "{gauge.label}" }
                                    span { "{gauge.value}" }
                                }
                                span { class: "meter-track",
                                    span { class: "meter-fill {gauge.tone}", style: "width:{gauge.pct}%" }
                                }
                            }
                        }
                    }
                }
            },
        )}
    }
}

fn nodes(data: &OverviewData, action: EventHandler<BoardAction>) -> Element {
    let head = Some(rsx! { span { class: "muted-note", "CPU · Memory" } });
    rsx! {
        {panel(
            WidgetId::Nodes,
            head,
            rsx! {
                div { class: "node-list",
                    for node in data.nodes.clone() {
                        button {
                            r#type: "button",
                            class: "node-row",
                            "data-goto": "{node.id}",
                            onclick: move |_| action.call(BoardAction::Goto(node.id.clone())),
                            span { class: "mono", "{node.name}" }
                            span { class: "hbar",
                                span { class: "track",
                                    span { class: "fill", style: "width:{node.cpu_pct}%" }
                                }
                            }
                            span { class: "hbar",
                                span { class: "track",
                                    span { class: "fill violet", style: "width:{node.mem_pct}%" }
                                }
                            }
                            span {
                                class: if node.degraded { "pill warn" } else { "pill success" },
                                "{node.note}"
                            }
                        }
                    }
                }
            },
        )}
    }
}

fn health(data: &OverviewData, action: EventHandler<BoardAction>) -> Element {
    let total: usize = data.pods_by_status.iter().map(|(_, n)| *n).sum();
    let top = data.namespaces.first().map(|(_, n)| *n).unwrap_or(1).max(1);
    rsx! {
        {panel(
            WidgetId::Health,
            None,
            rsx! {
                div { class: "panel-body",
                    if total > 0 {
                        div { class: "stackbar", role: "img", "aria-label": "Pods by status",
                            for (status, count) in data.pods_by_status.clone() {
                                span {
                                    style: "width:{(count as f32 / total as f32 * 100.0):.1}%",
                                    title: "{status}: {count}",
                                }
                            }
                        }
                    }
                    div { class: "seg-legend",
                        for (status, count) in data.pods_by_status.clone() {
                            span {
                                i {}
                                "{status} "
                                b { "{count}" }
                            }
                        }
                    }
                    div { class: "inspector-eyebrow", style: "margin-top:18px", "Top namespaces by CPU" }
                    div { class: "rank-list", style: "margin-top:0",
                        for (ns, count) in data.namespaces.clone() {
                            button {
                                r#type: "button",
                                class: "rank-row",
                                "data-ns-open": "{ns}",
                                onclick: move |_| action.call(BoardAction::OpenKind("pods")),
                                span { class: "mono", "{ns}" }
                                span { class: "hbar",
                                    span { class: "track",
                                        span {
                                            class: "fill",
                                            style: "width:{(count as f32 / top as f32 * 100.0).max(4.0):.0}%",
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
        )}
    }
}

fn argo(data: &OverviewData, action: EventHandler<BoardAction>) -> Element {
    let bad: Vec<_> = data
        .argo
        .iter()
        .filter(|a| a.sync != "Synced" || a.health != "Healthy")
        .cloned()
        .collect();
    let head = Some(rsx! {
        span { class: "muted-note", "{data.argo.len() - bad.len()} of {data.argo.len()} healthy" }
    });
    if bad.is_empty() {
        return panel(
            WidgetId::Argo,
            head,
            all_clear("All applications are synced and healthy"),
        );
    }
    rsx! {
        {panel(
            WidgetId::Argo,
            head,
            rsx! {
                div { class: "prob-list",
                    for app in bad {
                        button {
                            r#type: "button",
                            class: "prob-row",
                            "data-goto": "{app.id}",
                            onclick: move |_| action.call(BoardAction::Goto(app.id.clone())),
                            span { class: "pill warn", "{app.sync}" }
                            span { class: "mono", "{app.name}" }
                            span { class: "pill warn", "{app.health}" }
                        }
                    }
                }
            },
        )}
    }
}

fn events(data: &OverviewData, action: EventHandler<BoardAction>) -> Element {
    let head = Some(rsx! {
        div { class: "chip-row",
            button { r#type: "button", class: "chip active", "data-evf": "all", "All" }
            button { r#type: "button", class: "chip", "data-evf": "Warning", "Warnings" }
            button {
                r#type: "button",
                class: "link-back",
                "data-kindgo": "events",
                onclick: move |_| action.call(BoardAction::OpenKind("events")),
                "View all "
                svg { class: "icon", use { href: "#i-arrow" } }
            }
        }
    });
    if data.events.is_empty() {
        return panel(WidgetId::Events, head, all_clear("No events yet"));
    }
    rsx! {
        {panel(
            WidgetId::Events,
            head,
            rsx! {
                table { class: "ev-table",
                    tbody {
                        for ev in data.events.clone() {
                            tr {
                                class: if ev.target.is_some() { "go" } else { "" },
                                tabindex: "0",
                                "data-goto": ev.target.clone().unwrap_or_default(),
                                td { style: "width:1%",
                                    span {
                                        class: if ev.kind == "Warning" { "pill warn" } else { "pill success" },
                                        "{ev.kind}"
                                    }
                                }
                                td { b { "{ev.reason}" } }
                                td { class: "col-obj obj", "{ev.object}" }
                                td { class: "col-msg msg", "{ev.message}" }
                                td { class: "age", "{ev.age}" }
                            }
                        }
                    }
                }
            },
        )}
    }
}
