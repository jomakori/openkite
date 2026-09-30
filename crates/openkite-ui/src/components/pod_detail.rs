//! Pod detail slide-over: the 5-tab inspector (Overview / Logs / Events /
//! YAML / Containers) driven by the owned `SELECTED_POD` contract, so both
//! hosts render it without importing a kube type.

use dioxus::prelude::*;
use openkite_api::pod::pick_default_container;

use crate::runtime::{clear_selected_pod, LOGS_BUFFER, LOGS_CONTAINER, SELECTED_POD};

/// The inspector's tab bar, local to the open slide-over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetailTab {
    Overview,
    Logs,
    Events,
    Yaml,
    Containers,
}

/// One tab button in the inspector tab bar.
fn tab_button(label: &'static str, tab: DetailTab, mut active: Signal<DetailTab>) -> Element {
    let is_active = active() == tab;
    rsx! {
        button {
            class: if is_active { "tab-btn active" } else { "tab-btn" },
            onclick: move |_| active.set(tab),
            "{label}"
        }
    }
}

/// Pod detail slide-over. Mounted inside the app shell; renders when
/// `SELECTED_POD` is set and closes itself via `clear_selected_pod`.
#[component]
pub fn PodDetail() -> Element {
    let open = SELECTED_POD.read().is_some();
    if !open {
        return rsx! {};
    }

    let mut active_tab = use_signal(|| DetailTab::Overview);

    let name = SELECTED_POD
        .read()
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let namespace = SELECTED_POD
        .read()
        .as_ref()
        .and_then(|p| p.namespace.clone())
        .unwrap_or_else(|| "default".into());

    rsx! {
        div { class: "inspector open",
            div { class: "inspector-header",
                div { class: "inspector-title",
                    h2 { "{name}" }
                    span { class: "resource-kind", "Pod" }
                }
                div { class: "inspector-actions",
                    button {
                        class: "btn btn-secondary",
                        onclick: move |_| clear_selected_pod(),
                        "Close"
                    }
                }
            }
            div { class: "inspector-meta",
                span { "namespace: {namespace}" }
            }
            div { class: "inspector-tabs",
                {tab_button("Overview", DetailTab::Overview, active_tab)}
                {tab_button("Logs", DetailTab::Logs, active_tab)}
                {tab_button("Events", DetailTab::Events, active_tab)}
                {tab_button("YAML", DetailTab::Yaml, active_tab)}
                {tab_button("Containers", DetailTab::Containers, active_tab)}
            }
            div { class: "inspector-body",
                match active_tab() {
                    DetailTab::Overview => rsx! { OverviewTab {} },
                    DetailTab::Logs => rsx! { LogsTab {} },
                    DetailTab::Events => rsx! { EventsTab {} },
                    DetailTab::Yaml => rsx! { YamlTab {} },
                    DetailTab::Containers => rsx! { ContainersTab {} },
                }
            }
        }
    }
}

/// Overview tab: pod summary fields + labels + annotations.
#[component]
fn OverviewTab() -> Element {
    let Some(pod) = SELECTED_POD.read().clone() else {
        return rsx! {};
    };
    let summary = &pod.summary;

    rsx! {
        div { class: "kv-list",
            div { class: "kv-row", dt { "Phase" }, dd { "{summary.phase}" } }
            div { class: "kv-row", dt { "Node" }, dd { "{summary.node}" } }
            div { class: "kv-row", dt { "Pod IP" }, dd { "{summary.pod_ip}" } }
            div { class: "kv-row", dt { "QoS" }, dd { "{summary.qos}" } }
            if let Some(reason) = &summary.reason {
                div { class: "kv-row", dt { "Reason" }, dd { "{reason}" } }
            }
            if let Some(msg) = &summary.message {
                div { class: "kv-row", dt { "Message" }, dd { "{msg}" } }
            }
            if !pod.labels.is_empty() {
                div { class: "kv-row", dt { "Labels" },
                    dd {
                        for (k, v) in pod.labels.iter() {
                            div { "{k}={v}" }
                        }
                    }
                }
            }
            if !pod.annotations.is_empty() {
                div { class: "kv-row", dt { "Annotations" },
                    dd {
                        for (k, v) in pod.annotations.iter() {
                            div { "{k}={v}" }
                        }
                    }
                }
            }
        }
    }
}

/// Logs tab: container selector + the shared buffered line list.
#[component]
fn LogsTab() -> Element {
    let Some(pod) = SELECTED_POD.read().clone() else {
        return rsx! {};
    };
    let containers = pod.container_names();
    let init_container = pick_default_container(&containers).unwrap_or_default();

    let mut selected = use_signal(move || {
        let current = LOGS_CONTAINER.cloned();
        if !current.is_empty() {
            current
        } else {
            init_container.clone()
        }
    });

    use_effect(move || {
        let chosen = selected();
        if chosen.is_empty() {
            *LOGS_CONTAINER.write() = String::new();
        } else if LOGS_CONTAINER.cloned() != chosen {
            *LOGS_CONTAINER.write() = chosen;
        }
    });

    let lines: Vec<String> = LOGS_BUFFER.read().lines().to_vec();

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 8px;",
            div { style: "display: flex; gap: 8px; align-items: center;",
                select {
                    style: "font: inherit; font-size: 12px; padding: 4px 8px; border-radius: 6px; border: 1px solid var(--border); background: var(--bg-2); color: var(--fg-0);",
                    value: "{selected}",
                    oninput: move |e| selected.set(e.value()),
                    for c in containers.iter() {
                        option { value: "{c}", "{c}" }
                    }
                }
            }
            div { class: "log-body", style: "height: 320px; overflow-y: auto; font-family: var(--font-mono); font-size: 12px;",
                for line in lines {
                    div { class: "log-line", "{line}" }
                }
            }
        }
    }
}

/// Events tab: placeholder for cluster-fetched pod events.
#[component]
fn EventsTab() -> Element {
    rsx! {
        div { style: "color: var(--fg-2); font-size: 13px; padding: 8px;",
            "Events will be fetched from the cluster and displayed here."
        }
    }
}

/// YAML tab: raw pod manifest as read-only editor text.
#[component]
fn YamlTab() -> Element {
    let yaml = SELECTED_POD
        .read()
        .as_ref()
        .map(|p| p.yaml.clone())
        .unwrap_or_default();
    rsx! {
        crate::components::code_editor::CodeEditor {
            text: yaml,
            read_only: true,
            diagnostics: Vec::new(),
        }
    }
}

/// Containers tab: table of container info.
#[component]
fn ContainersTab() -> Element {
    let Some(pod) = SELECTED_POD.read().clone() else {
        return rsx! {};
    };
    rsx! {
        table { style: "width: 100%; border-collapse: collapse; font-size: 12px;",
            thead {
                tr { style: "border-bottom: 1px solid var(--border);",
                    th { style: "text-align: left; padding: 6px 8px; color: var(--fg-2);", "Name" }
                    th { style: "text-align: left; padding: 6px 8px; color: var(--fg-2);", "Image" }
                    th { style: "text-align: left; padding: 6px 8px; color: var(--fg-2);", "State" }
                    th { style: "text-align: left; padding: 6px 8px; color: var(--fg-2);", "Ready" }
                    th { style: "text-align: left; padding: 6px 8px; color: var(--fg-2);", "Restarts" }
                }
            }
            tbody {
                for info in pod.containers.iter() {
                    tr { style: "border-bottom: 1px solid var(--border);",
                        td { style: "padding: 6px 8px; font-family: var(--font-mono);", "{info.name}" }
                        td { style: "padding: 6px 8px;", "{info.image}" }
                        td { style: "padding: 6px 8px;", "{info.state}" }
                        td { style: "padding: 6px 8px;",
                            span { class: if info.ready { "dot ok" } else { "dot err" } }
                        }
                        td { style: "padding: 6px 8px;", "{info.restarts}" }
                    }
                }
            }
        }
    }
}
