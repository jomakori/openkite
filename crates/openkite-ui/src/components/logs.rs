//! Standalone log viewer: container picker, follow/pause state, and a
//! streaming buffer drained by the host adapter.
//!
//! The pure-logic helpers live in `openkite_api::pod` so both the viewer and
//! the inspector's LogsTab share one source of truth. The `#[component]`
//! lives here and depends on the `dioxus::prelude` glob.

use dioxus::prelude::*;

use openkite_api::pod::{level_class, pick_default_container, should_show_paused_hint, PodObject};

use crate::runtime::{LOGS_BUFFER, LOGS_CONTAINER, LOGS_FOLLOW, SELECTED_POD};

#[component]
pub fn LogsView() -> Element {
    let pod: Option<PodObject> = SELECTED_POD.read().clone();

    let Some(pod) = pod else {
        return rsx! {
            div { class: "log-panel",
                div { class: "log-body",
                    span { style: "color: var(--fg-2);",
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
                    style: "font: inherit; font-size: 12px; padding: 4px 8px; border-radius: 6px; border: 1px solid var(--border); background: var(--bg-2); color: var(--fg-0);",
                    value: "{container}",
                    oninput: move |e| container.set(e.value()),
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
                        span { style: "color: var(--fg-2);", "Select a container to view logs." }
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
