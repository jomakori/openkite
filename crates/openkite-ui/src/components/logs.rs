//! Standalone log viewer: container picker, follow/pause state, and a
//! streaming buffer drained by the host adapter.
//!
//! The pure-logic helpers live in `openkite_api::pod` so both the viewer and
//! the inspector's LogsTab share one source of truth. The `#[component]`
//! lives here and depends on the `dioxus::prelude` glob.

use dioxus::prelude::*;

use openkite_api::pod::{level_class, pick_default_container, should_show_paused_hint, PodObject};

use crate::design::tokens::R_SM;
use crate::runtime::{LOGS_BUFFER, LOGS_CONTAINER, LOGS_FOLLOW, SELECTED_POD};
use crate::touch::{SHEET_DISMISS_MAX_PX, SHEET_DISMISS_MIN_PX};

#[component]
pub fn LogsView() -> Element {
    let pod: Option<PodObject> = SELECTED_POD.read().clone();

    let Some(pod) = pod else {
        return rsx! {
            div { class: "log-panel",
                div { class: "log-body",
                    span { style: "color: var(--subtle);",
                        "Select a pod to view its logs (use the workload list or the inspector)."
                    }
                }
            }
        };
    };

    let containers: Vec<String> = pod.container_names();
    let init_container = pick_default_container(&containers).unwrap_or_default();

    let mut container = use_signal(move || init_container.clone());
    let mut follow = use_signal(|| LOGS_FOLLOW.cloned());
    let mut at_bottom = use_signal(|| true);

    use_effect(move || {
        let chosen = container();
        if chosen.is_empty() {
            *LOGS_CONTAINER.write() = String::new();
        } else if LOGS_CONTAINER.cloned() != chosen {
            *LOGS_CONTAINER.write() = chosen;
        }
    });

    use_effect(move || {
        let want = follow();
        if LOGS_FOLLOW.cloned() != want {
            *LOGS_FOLLOW.write() = want;
        }
    });

    use_effect(move || {
        if container().is_empty() {
            return;
        }
        LOGS_BUFFER.write().clear();
    });

    use_effect(move || {
        let install = r#"
            (function() {
                if (window.__openkite_log_scroll_installed) return;
                window.__openkite_log_scroll_installed = true;
                var el = document.querySelector('.log-body');
                if (!el) { window.__openkite_log_scroll_installed = false; return; }
                el.addEventListener('scroll', function() {
                    var atBottom = (el.scrollHeight - el.scrollTop - el.clientHeight) < 4;
                    window.__openkite_log_at_bottom = atBottom ? '1' : '0';
                });
                window.__openkite_log_at_bottom = '1';
            })();
        "#;
        document::eval(install);
    });

    #[cfg(not(target_arch = "wasm32"))]
    use_effect(move || {
        spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                let raw = document::eval("window.__openkite_log_at_bottom || '1'")
                    .recv::<String>()
                    .await;
                let bottom = raw.map(|v| v == "1").unwrap_or(true);
                if at_bottom() != bottom {
                    at_bottom.set(bottom);
                }
            }
        });
    });

    use_effect(move || {
        let following = LOGS_FOLLOW.cloned();
        let bottom = at_bottom();
        if following && !bottom {
            *LOGS_FOLLOW.write() = false;
        } else if !following && bottom && follow() {
            *LOGS_FOLLOW.write() = true;
        }
    });

    use_effect(move || {
        let f = follow();
        if f != LOGS_FOLLOW.cloned() {
            *LOGS_FOLLOW.write() = f;
        }
    });

    use_effect(move || {
        if LOGS_FOLLOW.cloned() && at_bottom() {
            let _ = document::eval(
                r#"var el = document.querySelector('.log-body');
                   if (el) { el.scrollTop = el.scrollHeight; }"#,
            );
        }
    });

    let lines_snapshot: Vec<(String, &'static str)> = {
        let buf = LOGS_BUFFER.read();
        buf.lines()
            .iter()
            .map(|l| (l.clone(), level_class(l)))
            .collect()
    };
    let show_paused = should_show_paused_hint(LOGS_FOLLOW.cloned(), at_bottom());
    let pod_for_inspector = pod.clone();

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 8px; height: 100%;",
            div { style: "display: flex; gap: 8px; align-items: center;",
                select {
                    style: "font: inherit; font-size: 12px; padding: 4px 8px; border-radius: {R_SM}; border: 1px solid var(--border); background: var(--surface-solid); color: var(--fg);",
                    value: "{container}",
                    oninput: move |e| container.set(e.value()),
                    for c in containers.iter() {
                        option { value: "{c}", "{c}" }
                    }
                }
                label { style: "font-size: 12px; color: var(--subtle);",
                    input {
                        r#type: "checkbox",
                        checked: follow(),
                        oninput: move |e| follow.set(e.value() == "true"),
                    }
                    " Follow"
                }
                button {
                    class: "btn btn-secondary",
                    style: "min-height: 28px; padding: 0 8px; font-size: 12px;",
                    onclick: move |_| {
                        LOGS_BUFFER.write().clear();
                    },
                    "Clear"
                }
                button {
                    class: "btn btn-secondary",
                    style: "min-height: 28px; padding: 0 8px; font-size: 12px;",
                    onclick: move |_| {
                        *SELECTED_POD.write() = Some(pod_for_inspector.clone());
                    },
                    "Open in inspector"
                }
            }
            if show_paused {
                div { class: "log-paused", "paused — scroll to bottom to resume" }
            }
            div { class: "log-panel", style: "flex: 1; min-height: 0;",
                div { class: "log-body",
                    if lines_snapshot.is_empty() {
                        span { style: "color: var(--subtle);", "Select a container to view logs." }
                    } else {
                        for (text, cls) in lines_snapshot.iter().cloned() {
                            div { class: "log-line",
                                if cls.is_empty() {
                                    span { "{text}" }
                                } else {
                                    span { class: "log-level {cls}", "{text}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The ≤767px log surface: the design's bottom sheet, the same `.log-panel`
/// the dock carries, parked below the bottom bar.
///
/// The mockup puts a `.log-handle` — a grip plus "Logs" — on the sheet's top
/// edge, and reveals the sheet from the bar's Logs tab. The handle is the
/// sheet's only drag affordance: a downward drag past `resolve_sheet_drag`'s
/// threshold dismisses it, a shorter one springs it back. The stylesheet owns
/// the geometry (`.log-sheet`, exact mockup values at ≤767px), so above 768px
/// this markup renders nothing at all.
#[component]
pub fn LogSheet(
    #[props(default)] open: bool,
    // The handle and the bar's Logs tab both toggle the sheet.
    on_toggle: EventHandler<()>,
    // The scrim, Escape and a dismissing drag close it.
    on_close: EventHandler<()>,
) -> Element {
    let pod: Option<PodObject> = SELECTED_POD.read().clone();
    let pod_name = pod
        .as_ref()
        .map(|pod| pod.name.clone())
        .unwrap_or_else(|| "No pod selected".to_string());
    let lines_snapshot: Vec<(String, &'static str)> = {
        let buf = LOGS_BUFFER.read();
        buf.lines()
            .iter()
            .map(|line| (line.clone(), level_class(line)))
            .collect()
    };
    let following = LOGS_FOLLOW.cloned();
    let show_paused = should_show_paused_hint(following, false);

    // The handle's drag: track the pointer from the handle down the sheet,
    // follow it while it moves, and on release either dismiss or spring back.
    // `touch-action: none` on the handle keeps the page from scrolling under
    // the drag.
    use_effect(move || {
        // The dismiss threshold is the crate's, not the script's: the same
        // quarter-of-the-sheet rule `resolve_sheet_drag` grades, bounded by
        // the constants the tests pin.
        let install = format!(
            r#"
            (function () {{
                if (window.__openkite_sheet_drag) return;
                var handle = document.querySelector('.log-sheet .log-handle');
                if (!handle) {{ return; }}
                window.__openkite_sheet_drag = true;
                var sheet = handle.closest('.log-sheet');
                var startY = 0, dy = 0, dragging = false;
                var minThreshold = {min};
                var maxThreshold = {max};
                handle.addEventListener('pointerdown', function (event) {{
                    dragging = true;
                    startY = event.clientY;
                    dy = 0;
                    handle.setPointerCapture(event.pointerId);
                    sheet.style.transition = 'none';
                }});
                handle.addEventListener('pointermove', function (event) {{
                    if (!dragging) return;
                    dy = Math.max(0, event.clientY - startY);
                    sheet.style.transform = 'translateY(' + dy + 'px)';
                }});
                function release() {{
                    if (!dragging) return;
                    dragging = false;
                    sheet.style.transition = '';
                    sheet.style.transform = '';
                    var threshold = Math.min(
                        maxThreshold,
                        Math.max(minThreshold, sheet.clientHeight * 0.25)
                    );
                    if (dy >= threshold) dioxus.send('close');
                }}
                handle.addEventListener('pointerup', release);
                handle.addEventListener('pointercancel', release);
            }})();
        "#,
            min = SHEET_DISMISS_MIN_PX,
            max = SHEET_DISMISS_MAX_PX
        );
        let mut eval = document::eval(&install);
        spawn(async move {
            while let Ok(action) = eval.recv::<String>().await {
                if action == "close" {
                    on_close.call(());
                }
            }
        });
    });

    rsx! {
        section {
            class: if open { "log-panel log-sheet open" } else { "log-panel log-sheet" },
            aria_label: "Pod logs",
            button {
                class: "log-handle",
                r#type: "button",
                "aria-expanded": if open { "true" } else { "false" },
                onclick: move |_| on_toggle.call(()),
                span { "Logs" }
            }
            div { class: "log-header",
                div { class: "log-title",
                    span { "Logs" }
                    span { class: "log-pod", "{pod_name}" }
                }
                div { class: "log-actions",
                    button {
                        class: "icon-btn",
                        r#type: "button",
                        aria_label: if following { "Pause logs" } else { "Resume logs" },
                        "aria-pressed": if following { "false" } else { "true" },
                        onclick: move |_| *LOGS_FOLLOW.write() = !following,
                        if following { "❙❙" } else { "▶" }
                    }
                    button {
                        class: "icon-btn",
                        r#type: "button",
                        aria_label: "Clear logs",
                        onclick: move |_| LOGS_BUFFER.write().clear(),
                        "×"
                    }
                }
            }
            if show_paused {
                div { class: "log-paused", "paused — scroll to bottom to resume" }
            }
            div { class: "log-body",
                if lines_snapshot.is_empty() {
                    span { style: "color: var(--subtle);", "Select a container to view logs." }
                } else {
                    for (text, cls) in lines_snapshot.iter().cloned() {
                        div { class: "log-line",
                            if cls.is_empty() {
                                span { "{text}" }
                            } else {
                                span { class: "log-level {cls}", "{text}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
