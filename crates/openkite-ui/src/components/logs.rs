//! Standalone log viewer: the reference's `.log-panel` — the bottom-sheet
//! handle, the header with the pod name, the pause/clear actions and the
//! paused banner — over the shared streaming buffer, plus the container picker
//! and the inspector hand-off the console needs on top.
//!
//! The pure-logic helpers live in `openkite_api::pod` so this viewer and the
//! inspector's LogsTab split a line the same way. The `#[component]` lives here
//! and depends on the `dioxus::prelude` glob.

use dioxus::prelude::*;

use openkite_api::pod::{parse_log_line, pick_default_container, LogLine, PodObject};

use crate::runtime::{
    close_log_sheet, dismiss_log_sheet_on_selection, toggle_log_sheet, toggle_logs_paused,
    LOGS_BUFFER, LOGS_CONTAINER, LOGS_HELD, LOGS_PAUSED, LOGS_SHEET_OPEN, SELECTED_POD,
};

/// Escape dismisses the sheet from anywhere; the listener installs once per webview.
pub const LOG_SHEET_KEYBIND_JS: &str = r#"
if (!window.__openkite_log_sheet_keys) {
  window.__openkite_log_sheet_keys = true;
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      dioxus.send('close');
    }
  });
}
"#;

/// The page behind the open sheet does not scroll.
pub const LOG_SHEET_SCROLL_LOCK_JS: &str = "document.body.style.overflow = 'hidden';";

/// Dismissing the sheet hands the page its scroll back.
pub const LOG_SHEET_SCROLL_RELEASE_JS: &str = "document.body.style.overflow = '';";

#[component]
pub fn LogsView() -> Element {
    let pod: Option<PodObject> = SELECTED_POD.read().clone();
    let containers: Vec<String> = pod
        .as_ref()
        .map(|pod| pod.container_names())
        .unwrap_or_default();
    let pod_name = pod.as_ref().map(|pod| pod.name.clone()).unwrap_or_default();
    let init_container = pick_default_container(&containers).unwrap_or_default();

    let mut container = use_signal(move || init_container.clone());

    // The chosen container is the host stream controller's input: publish it,
    // and drop both windows so the previous container's lines never mix in.
    use_effect(move || {
        let chosen = container();
        if chosen.is_empty() {
            if !LOGS_CONTAINER.cloned().is_empty() {
                *LOGS_CONTAINER.write() = String::new();
            }
            return;
        }
        if LOGS_CONTAINER.cloned() != chosen {
            *LOGS_CONTAINER.write() = chosen;
            LOGS_BUFFER.write().clear();
            LOGS_HELD.write().clear();
        }
    });

    let paused = LOGS_PAUSED.cloned();
    let sheet_open = LOGS_SHEET_OPEN.cloned();

    // Escape is the sheet's dismiss key on every width; the handle covers pointer.
    use_effect(move || {
        let mut eval = document::eval(LOG_SHEET_KEYBIND_JS);
        spawn(async move {
            while let Ok(action) = eval.recv::<String>().await {
                if action == "close" {
                    close_log_sheet();
                }
            }
        });
    });

    // The reference composes both states on one class list: paused and sheet-open.
    let panel_class = match (paused, sheet_open) {
        (true, true) => "log-panel paused open",
        (true, false) => "log-panel paused",
        (false, true) => "log-panel open",
        (false, false) => "log-panel",
    };

    // Following keeps the newest line in view; a paused panel holds its window,
    // so there is nothing to scroll to.
    use_effect(move || {
        if LOGS_PAUSED.cloned() {
            return;
        }
        let _ = LOGS_BUFFER.read().lines().len();
        let _ = document::eval(
            r#"var el = document.querySelector('.log-body');
               if (el) { el.scrollTop = el.scrollHeight; }"#,
        );
    });

    // The page behind the open sheet must not scroll; dismissal releases it.
    use_effect(move || {
        let js = if LOGS_SHEET_OPEN.cloned() {
            LOG_SHEET_SCROLL_LOCK_JS
        } else {
            LOG_SHEET_SCROLL_RELEASE_JS
        };
        let _ = document::eval(js);
    });

    // A new selection dismisses the sheet, so it never covers the next pod.
    let shown_pod_name = pod.as_ref().map(|pod| pod.name.clone());
    let mut shown_pod = use_signal({
        let initial = shown_pod_name.clone();
        move || initial.clone()
    });
    use_effect(move || {
        let current = SELECTED_POD.read().as_ref().map(|pod| pod.name.clone());
        let previous = (*shown_pod.peek()).clone();
        if dismiss_log_sheet_on_selection(previous.as_deref(), current.as_deref()) {
            shown_pod.set(current);
        }
    });

    let lines: Vec<LogLine> = if paused {
        LOGS_HELD
            .read()
            .lines()
            .iter()
            .map(|line| parse_log_line(line))
            .collect()
    } else {
        LOGS_BUFFER
            .read()
            .lines()
            .iter()
            .map(|line| parse_log_line(line))
            .collect()
    };
    let pod_for_inspector = pod.clone();

    rsx! {
        section {
            class: "{panel_class}",
            "data-surface": "logs",
            aria_label: "Pod logs",
            button {
                class: "log-handle",
                r#type: "button",
                aria_expanded: if sheet_open { "true" } else { "false" },
                onclick: move |_| toggle_log_sheet(),
                "Logs"
            }
            div { class: "log-header",
                div { class: "log-title",
                    svg { class: "icon", "viewBox": "0 0 24 24",
                        rect { x: "3", y: "4", width: "18", height: "16", rx: "2" }
                        path { d: "m7 9 3 3-3 3M12 15h5" }
                    }
                    span { "Logs" }
                    if !pod_name.is_empty() {
                        span { class: "log-pod", title: "{pod_name}", "{pod_name}" }
                    }
                }
                div { class: "log-actions",
                    if !containers.is_empty() {
                        select {
                            class: "log-container",
                            aria_label: "Container",
                            value: "{container}",
                            oninput: move |event| container.set(event.value()),
                            for name in containers.iter() {
                                option { value: "{name}", "{name}" }
                            }
                        }
                    }
                    if !pod_name.is_empty() {
                        button {
                            class: "icon-btn",
                            r#type: "button",
                            aria_label: if paused { "Resume logs" } else { "Pause logs" },
                            title: if paused { "Resume logs" } else { "Pause logs" },
                            "aria-pressed": if paused { "true" } else { "false" },
                            onclick: move |_| toggle_logs_paused(),
                            svg { class: "icon", "viewBox": "0 0 24 24",
                                path { d: "M8 6v12M16 6v12" }
                            }
                        }
                        button {
                            class: "icon-btn",
                            r#type: "button",
                            aria_label: "Clear logs",
                            title: "Clear logs",
                            onclick: move |_| {
                                LOGS_BUFFER.write().clear();
                                LOGS_HELD.write().clear();
                            },
                            svg { class: "icon", "viewBox": "0 0 24 24",
                                path { d: "M5 7h14M9 7V4h6v3M8 10v10h8V10M10 13v4M14 13v4" }
                            }
                        }
                        button {
                            class: "btn btn-secondary",
                            r#type: "button",
                            onclick: move |_| {
                                *SELECTED_POD.write() = pod_for_inspector.clone();
                            },
                            "Open in inspector"
                        }
                    }
                }
            }
            div { class: "log-paused", role: "status",
                svg { class: "icon", "viewBox": "0 0 24 24",
                    path { d: "M8 6v12M16 6v12" }
                }
                span { "Log stream paused" }
            }
            div { class: "log-body",
                if lines.is_empty() {
                    span { style: "color: var(--subtle);",
                        if pod_name.is_empty() {
                            "Select a pod to view its logs (use the workload list or the inspector)."
                        } else {
                            "Select a container to view logs."
                        }
                    }
                } else {
                    for line in lines.iter() {
                        LogLineRow { line: line.clone() }
                    }
                }
            }
        }
    }
}

/// One `.log-line`: the reference's clock / level / method / message columns.
///
/// Shared with the inspector's LogsTab so a line is split and coloured in one
/// place, not once per surface.
#[component]
pub fn LogLineRow(line: LogLine) -> Element {
    let level_class = line.level_class();
    rsx! {
        div { class: "log-line",
            if !line.time.is_empty() {
                span { class: "log-time", "{line.time}" }
            }
            if !line.level.is_empty() {
                span {
                    class: if level_class == "warn" { "log-level warn" } else if level_class == "error" { "log-level error" } else { "log-level" },
                    "{line.level}"
                }
            }
            if !line.method.is_empty() {
                span { class: "log-method", "{line.method}" }
            }
            span {
                class: if line.message_is_error() { "log-msg error" } else { "log-msg" },
                "{line.message}"
            }
        }
    }
}
