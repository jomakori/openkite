//! The log dock: one or two panes, each with its tools, its status note and its lines.
//!
//! Pane state lives in [`crate::logs`]; this module only renders it and reports what the user
//! asked for. It never reads a stream, so a paused pane cannot accidentally resume here.

use dioxus::prelude::*;

use crate::logs::{DockAction, Level, LogDock, LogPane};

/// Props for [`Dock`].
#[derive(Props, Clone, PartialEq)]
pub struct DockProps {
    /// Pane state plus the picker's candidates.
    pub dock: LogDock,
    /// Called with every toolbar, tab or pane intent.
    pub on_action: EventHandler<DockAction>,
}

/// The bottom log dock.
#[component]
pub fn Dock(props: DockProps) -> Element {
    let dock = props.dock.clone();
    let action = props.on_action;
    let split = dock.split();
    let problems: usize = dock.panes.iter().map(|pane| pane.problems()).sum();

    rsx! {
        section {
            class: if dock.open { "dock open" } else { "dock" },
            "data-open": "{dock.open}",
            "aria-label": "Log dock",
            div { class: "dock-bar",
                button {
                    class: "dock-toggle",
                    r#type: "button",
                    "aria-expanded": "{dock.open}",
                    "data-act": "toggle",
                    onclick: move |_| action.call(DockAction::Toggle),
                    span { class: "ic", dangerous_inner_html: "{icon(\"logs\")}" }
                    span { "Logs" }
                    if problems > 0 {
                        span { class: "chip warn", "data-problems": "{problems}", "{problems}" }
                    }
                }
                div { class: "dock-tabs",
                    for (at, pane) in dock.panes.iter().enumerate() {
                        button {
                            class: if at == dock.focused && split { "dock-tab active" } else { "dock-tab" },
                            r#type: "button",
                            "data-focus": "{at}",
                            onclick: move |_| action.call(DockAction::Focus(at)),
                            span { class: "tab-main",
                                span { class: "ic", dangerous_inner_html: "{icon(\"logs\")}" }
                                span { "{pane.pod.name}" }
                            }
                            if pane.pinned {
                                span { class: "tab-pin", "aria-hidden": "true", "·" }
                            }
                        }
                    }
                }
                div { class: "dock-actions",
                    select {
                        class: "dock-picker",
                        "aria-label": "Follow another pod",
                        "data-pick": "pod",
                        onchange: move |evt| action.call(DockAction::PickPod(evt.value())),
                        for choice in dock.choices.iter() {
                            option {
                                value: "{choice.id}",
                                selected: dock.focused_pane().map(|p| p.pod.id == choice.id).unwrap_or(false),
                                "{choice.namespace}/{choice.name}"
                            }
                        }
                    }
                    button {
                        class: "dock-act",
                        r#type: "button",
                        "data-act": if split { "merge" } else { "split" },
                        disabled: dock.panes.is_empty(),
                        onclick: move |_| action.call(if split { DockAction::Merge } else { DockAction::Split }),
                        if split { "Merge" } else { "Split" }
                    }
                    button {
                        class: "dock-act",
                        r#type: "button",
                        "data-act": "close",
                        "aria-label": "Close the log dock",
                        onclick: move |_| action.call(DockAction::Close),
                        span { class: "ic", dangerous_inner_html: "{icon(\"close\")}" }
                    }
                }
            }
            if dock.open {
                div { class: if split { "dock-body split" } else { "dock-body" },
                    for (at, pane) in dock.panes.iter().enumerate() {
                        Pane { key: "{pane.pod.id}-{at}", pane: pane.clone(), focused: split && at == dock.focused, on_action: action }
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct PaneProps {
    pane: LogPane,
    focused: bool,
    on_action: EventHandler<DockAction>,
}

#[allow(non_snake_case)]
fn Pane(props: PaneProps) -> Element {
    let pane = props.pane.clone();
    let action = props.on_action;
    let visible = pane.visible().count();
    let hidden = pane.lines.len() - visible;

    rsx! {
        div {
            class: if props.focused { "dpane log focused" } else { "dpane log" },
            "data-pod": "{pane.pod.id}",
            "data-paused": "{pane.paused}",
            div { class: "dpane-head",
                span { class: "dpane-title",
                    span { class: "ic", dangerous_inner_html: "{icon(\"logs\")}" }
                    span { "{pane.pod.namespace}/{pane.pod.name}" }
                    span { class: "dpane-ctr", "{pane.container}" }
                }
                span { class: "dpane-note", if pane.paused { "paused" } else if pane.follow { "following" } else { "stopped" } }
            }
            div { class: "dpane-main",
                div { class: "log-tools",
                    select {
                        "aria-label": "Container",
                        "data-ctr": "container",
                        onchange: move |evt| action.call(DockAction::SetContainer(evt.value())),
                        for name in pane.pod.containers.iter() {
                            option { value: "{name}", selected: *name == pane.container, "{name}" }
                        }
                    }
                    label { class: "search-field log-search",
                        span { class: "ic", dangerous_inner_html: "{icon(\"search\")}" }
                        input {
                            r#type: "search",
                            placeholder: "Search logs",
                            "aria-label": "Search logs",
                            value: "{pane.filter}",
                            oninput: move |evt| action.call(DockAction::SetFilter(evt.value())),
                        }
                    }
                    Toggle { label: "Timestamps", on: pane.timestamps, token: "ts", on_action: action, action: DockAction::ToggleTimestamps }
                    Toggle { label: "Follow", on: pane.follow, token: "follow", on_action: action, action: DockAction::ToggleFollow }
                    if pane.paused {
                        button {
                            class: "tool-toggle",
                            r#type: "button",
                            "data-k": "paused",
                            "aria-pressed": "true",
                            onclick: move |_| action.call(DockAction::TogglePause),
                            "Resume"
                        }
                    } else {
                        button {
                            class: "tool-toggle icon-only",
                            r#type: "button",
                            "data-k": "paused",
                            "aria-label": "Pause stream",
                            "aria-pressed": "false",
                            title: "Pause stream",
                            onclick: move |_| action.call(DockAction::TogglePause),
                            span { class: "ic", dangerous_inner_html: "{icon(\"pause\")}" }
                        }
                    }
                    span { class: "tools-sep" }
                    button {
                        class: "tool-toggle icon-only",
                        r#type: "button",
                        "data-k": "clear",
                        "aria-label": "Clear view",
                        title: "Clear view",
                        onclick: move |_| action.call(DockAction::Clear),
                        span { class: "ic", dangerous_inner_html: "{icon(\"clear\")}" }
                    }
                }
                div { class: "dock-note show", role: "status",
                    if pane.paused {
                        "Paused — the stream is not read while paused, so new lines are lost rather than buffered."
                    } else if !pane.follow {
                        "Follow is off — showing what was captured before it stopped."
                    } else if hidden > 0 {
                        "{visible} of {pane.lines.len()} lines match the search."
                    } else {
                        "{pane.lines.len()} lines · {pane.bytes_in()} bytes read"
                    }
                }
                div { class: if pane.paused { "log-body paused" } else { "log-body" },
                    if pane.lines.is_empty() {
                        div { class: "log-empty", "No log lines yet." }
                    } else if visible == 0 {
                        div { class: "log-empty", "No lines match the search." }
                    } else {
                        for line in pane.visible() {
                            div { class: "log-line", "data-seq": "{line.seq}", "data-lvl": "{line.level.id()}",
                                if pane.timestamps {
                                    span { class: "log-time", "{line.stamp.clone().unwrap_or_default()}" }
                                }
                                span { class: level_class(line.level), "{line.level.label()}" }
                                span { class: if line.level == Level::Error { "log-msg error" } else { "log-msg" }, "{line.text}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct ToggleProps {
    label: &'static str,
    on: bool,
    token: &'static str,
    on_action: EventHandler<DockAction>,
    action: DockAction,
}

#[allow(non_snake_case)]
fn Toggle(props: ToggleProps) -> Element {
    let action = props.on_action;
    let pick = props.action.clone();
    rsx! {
        button {
            class: "tool-toggle",
            r#type: "button",
            "data-k": "{props.token}",
            "aria-pressed": "{props.on}",
            onclick: move |_| action.call(pick.clone()),
            "{props.label}"
        }
    }
}

fn level_class(level: Level) -> String {
    format!("log-level {}", level.id())
}

fn icon(name: &str) -> String {
    format!(
        "<svg class=\"icon\" aria-hidden=\"true\"><use href=\"{}/assets/icons.svg#i-{name}\"></use></svg>",
        env!("CARGO_MANIFEST_DIR")
    )
}
