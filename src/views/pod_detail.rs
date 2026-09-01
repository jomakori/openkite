//! Pod detail slide-over: 5-tab inspector (Overview, Logs, Events, YAML, Containers).
//!
//! Reads `SELECTED_POD` from the runtime; renders as a right-side slide-over
//! panel with `.inspector` / `.inspector.open` CSS classes.

use dioxus::prelude::*;
use k8s_openapi::api::core::v1::Pod;
use kube::api::LogParams;
use kube::Api;

use crate::pod::{container_infos, pod_summary};

/// Tab identifiers for the inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetailTab {
    Overview,
    Logs,
    Events,
    Yaml,
    Containers,
}

/// Pod detail slide-over. Mounted inside AppShell; opens when SELECTED_POD
/// is set (row click) and closes on the X button.
#[component]
pub fn PodDetail() -> Element {
    let open = crate::runtime::SELECTED_POD.read().is_some();
    let mut active_tab = use_signal(|| DetailTab::Overview);

    // Close handler: clear the selected pod and reset to Overview tab.
    let close = move |_| {
        *crate::runtime::SELECTED_POD.write() = None;
        active_tab.set(DetailTab::Overview);
    };

    let class = if open { "inspector open" } else { "inspector" };

    rsx! {
        div {
            class: "{class}",
            if let Some(p) = crate::runtime::SELECTED_POD.read().clone() {
                div { class: "inspector-header",
                    div { class: "inspector-eyebrow",
                        "{p.metadata.namespace.as_deref().unwrap_or(\"default\")}"
                    }
                    div { style: "display: flex; align-items: center; gap: 8px;",
                        h3 { style: "margin: 0; font-size: 15px;",
                            "{p.metadata.name.as_deref().unwrap_or(\"unknown\")}"
                        }
                        button {
                            class: "btn btn-secondary",
                            style: "margin-left: auto; min-height: 28px; padding: 0 8px; font-size: 12px;",
                            onclick: close,
                            "✕"
                        }
                    }
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
                        DetailTab::Overview => rsx! { OverviewTab { pod: p.clone() } },
                        DetailTab::Logs => rsx! { LogsTab { pod: p.clone() } },
                        DetailTab::Events => rsx! { EventsTab { pod: p.clone() } },
                        DetailTab::Yaml => rsx! { YamlTab { pod: p.clone() } },
                        DetailTab::Containers => rsx! { ContainersTab { pod: p.clone() } },
                    }
                }
            }
        }
    }
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

/// Overview tab: pod summary fields + labels + annotations.
#[component]
fn OverviewTab(pod: Pod) -> Element {
    let summary = pod_summary(&pod);
    let labels = pod.metadata.labels.clone();
    let annotations = pod.metadata.annotations.clone();

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
            if let Some(labels) = labels {
                div { class: "kv-row", dt { "Labels" },
                    dd {
                        for (k, v) in labels.iter() {
                            div { "{k}={v}" }
                        }
                    }
                }
            }
            if let Some(annotations) = annotations {
                div { class: "kv-row", dt { "Annotations" },
                    dd {
                        for (k, v) in annotations.iter() {
                            div { "{k}={v}" }
                        }
                    }
                }
            }
        }
    }
}

/// Logs tab: container selector + follow checkbox + line list.
#[component]
fn LogsTab(pod: Pod) -> Element {
    let containers: Vec<String> = pod
        .spec
        .as_ref()
        .map(|s| s.containers.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default();
    let mut selected_container = use_signal(|| containers.first().cloned().unwrap_or_default());
    let mut follow = use_signal(|| true);
    let mut lines = use_signal(Vec::<String>::new);

    // Spawn the log stream when container or follow changes.
    let pod_name = pod.metadata.name.clone().unwrap_or_default();
    let pod_ns = pod
        .metadata
        .namespace
        .clone()
        .unwrap_or_else(|| "default".into());
    use_effect(move || {
        let container = selected_container();
        let should_follow = follow();
        if container.is_empty() {
            return;
        }
        lines.write().clear();
        if let Some(client) = crate::runtime::client() {
            let api: Api<Pod> = Api::namespaced(client, &pod_ns);
            let name = pod_name.clone();
            let cont = container.clone();
            tokio::spawn(async move {
                let params = LogParams {
                    container: Some(cont),
                    follow: should_follow,
                    tail_lines: Some(100),
                    timestamps: true,
                    ..LogParams::default()
                };
                // Touch the stream so the runtime registers the watch; full
                // line buffering arrives in a follow-up that drains the
                // AsyncBufRead into the `lines` signal.
                let _ = api.log_stream(&name, &params).await;
            });
        }
    });

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 8px;",
            div { style: "display: flex; gap: 8px; align-items: center;",
                select {
                    style: "font: inherit; font-size: 12px; padding: 4px 8px; border-radius: 6px; border: 1px solid var(--border); background: var(--bg-2); color: var(--fg-0);",
                    value: "{selected_container}",
                    oninput: move |e| selected_container.set(e.value()),
                    for c in containers.iter() {
                        option { value: "{c}", "{c}" }
                    }
                }
                label { style: "font-size: 12px; color: var(--fg-2);",
                    input {
                        r#type: "checkbox",
                        checked: follow(),
                        oninput: move |e| follow.set(e.value() == "true"),
                    }
                    " Follow"
                }
            }
            div { class: "log-panel", style: "max-height: 400px;",
                div { class: "log-body", style: "padding: 8px 12px; font-size: 12px;",
                    if lines.read().is_empty() {
                        span { style: "color: var(--fg-2);", "Select a container to view logs." }
                    } else {
                        for line in lines.read().iter() {
                            div { class: "log-line", "{line}" }
                        }
                    }
                }
            }
        }
    }
}

/// Events tab: placeholder for cluster-fetched pod events.
#[component]
fn EventsTab(pod: Pod) -> Element {
    rsx! {
        div { style: "color: var(--fg-2); font-size: 13px; padding: 8px;",
            "Events will be fetched from the cluster and displayed here."
        }
    }
}

/// YAML tab: raw pod manifest as preformatted text.
#[component]
fn YamlTab(pod: Pod) -> Element {
    let yaml = serde_saphyr::to_string(&pod).unwrap_or_else(|_| "Failed to serialize".into());
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
fn ContainersTab(pod: Pod) -> Element {
    let infos = container_infos(&pod);
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
                for info in infos.iter() {
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
