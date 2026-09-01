//! Standalone log viewer: pod/container picker, follow/pause state, and a
//! streaming `LineBuffer` drained from a kube `log_stream`.
//!
//! The pure-logic helpers at the top of this file are testable without a
//! Dioxus runtime; the `#[component]` lives at the bottom and depends on the
//! `dioxus::prelude` glob (split per the openkite-dev skill
//! §"Split pure logic from the Dioxus view"). Reuses the P1 surface in
//! `crate::logs` (`LogOptions`, `LogStream`, `LineBuffer`, `FollowState`).

use crate::logs::{FollowState, LineBuffer};
use dioxus::prelude::*;

/// The first non-empty container name, or `None` if the list is empty.
///
/// The standalone viewer's `<select>` defaults to this; the inspector's
/// `LogsTab` does the same inline at `src/views/pod_detail.rs:140`. The
/// helper unifies the two so the empty-string skip is testable in one place.
pub fn pick_default_container(containers: &[String]) -> Option<String> {
    containers.iter().find(|c| !c.is_empty()).cloned()
}

/// Whether the `.log-paused` hint should be rendered.
///
/// The hint shows when the follow state is `Paused` OR the user has scrolled
/// up; either condition tells the user the view is "stuck" so they know to
/// scroll back down. The predicate handles both cases independently so the
/// consumer does not need to know the order they were set.
pub fn should_show_paused_hint(state: FollowState, at_bottom: bool) -> bool {
    !state.is_following() || !at_bottom
}

/// Map a log line's first whitespace-delimited token to a log-level class.
///
/// Returns `"warn"` for `WARN|warn`, `"error"` for `ERROR|ERR|error|err`,
/// and `""` otherwise. The class names line up with the CSS rules in
/// `assets/main.css:366-368`. The sniff is deliberately narrow — a
/// structured JSON / logfmt parser is a follow-up ticket.
pub fn level_class(line: &str) -> &'static str {
    let head = line.split_ascii_whitespace().next().unwrap_or("");
    match head {
        "WARN" | "warn" => "warn",
        "ERROR" | "ERR" | "error" | "err" => "error",
        _ => "",
    }
}

/// Clear every retained line. The toolbar's "Clear" button calls this so the
/// test can pin a single entry point without importing `LineBuffer` for a
/// one-liner.
pub fn clear_and_reset(buffer: &mut LineBuffer) {
    buffer.clear();
}

/// Owned-clone snapshot of the buffer's lines.
///
/// The viewer's `use_effect` drains the stream into a `Signal<LineBuffer>`
/// and re-renders; the `rsx!` body iterates `lines.read().lines()` (the
/// same shape the inspector's `LogsTab` uses at `src/views/pod_detail.rs:203`).
/// The helper exposes the owned-clone form so the test can assert ordering
/// without a Dioxus runtime.
pub fn view_buffers_snapshot(buffers: &LineBuffer) -> Vec<String> {
    buffers.lines().to_vec()
}

#[cfg(test)]
mod pure_logic_tests {
    use super::*;

    #[test]
    fn pick_default_container_returns_first_when_non_empty() {
        assert_eq!(
            pick_default_container(&["a".into(), "b".into(), "c".into()]),
            Some("a".into())
        );
    }

    #[test]
    fn pick_default_container_skips_empty_string() {
        assert_eq!(
            pick_default_container(&["".into(), "a".into()]),
            Some("a".into())
        );
    }

    #[test]
    fn pick_default_container_returns_none_for_empty() {
        assert_eq!(pick_default_container(&[]), None);
        assert_eq!(pick_default_container(&["".into()]), None);
    }

    #[test]
    fn should_show_paused_hint_is_true_when_paused() {
        assert!(should_show_paused_hint(FollowState::Paused, true));
    }

    #[test]
    fn should_show_paused_hint_is_true_when_scrolled_up() {
        assert!(should_show_paused_hint(FollowState::Following, false));
    }

    #[test]
    fn should_show_paused_hint_is_false_when_following_at_bottom() {
        assert!(!should_show_paused_hint(FollowState::Following, true));
    }

    #[test]
    fn level_class_recognises_warn_and_error() {
        assert_eq!(level_class("WARN foo"), "warn");
        assert_eq!(level_class("ERROR bar"), "error");
        assert_eq!(level_class("INFO baz"), "");
        assert_eq!(level_class("warning baz"), "");
    }

    #[test]
    fn level_class_handles_empty_and_whitespace_only() {
        assert_eq!(level_class(""), "");
        assert_eq!(level_class("   "), "");
    }

    #[test]
    fn clear_and_reset_empties_the_buffer() {
        let mut buf = LineBuffer::default();
        buf.push("a");
        buf.push("b");
        clear_and_reset(&mut buf);
        assert!(buf.is_empty());
    }

    #[test]
    fn view_buffers_snapshot_returns_owned_clone() {
        let mut buf = LineBuffer::default();
        buf.push("a");
        buf.push("b");
        buf.push("c");
        let snap = view_buffers_snapshot(&buf);
        assert_eq!(
            snap,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        // Mutating the buffer after the snapshot does not affect the
        // captured clone (the helper is `&LineBuffer`, not `&mut`).
        buf.push("d");
        assert_eq!(snap.len(), 3);
    }
}

#[component]
pub fn LogsView() -> Element {
    use crate::logs::{LogOptions, LogStream};
    use futures::{AsyncBufReadExt, StreamExt};
    use k8s_openapi::api::core::v1::Pod;
    use kube::Api;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tokio::task::JoinHandle;

    // The pod comes from the shared `SELECTED_POD` signal (the same one the
    // inspector reads). A direct pod picker is a follow-up; OKT-35 ships
    // the "open from inspector" hand-off as the entry point.
    let pod: Option<Pod> = crate::runtime::SELECTED_POD.read().clone();

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

    let pod_name = pod.metadata.name.clone().unwrap_or_default();
    let pod_ns = pod
        .metadata
        .namespace
        .clone()
        .unwrap_or_else(|| "default".into());
    let containers: Vec<String> = pod
        .spec
        .as_ref()
        .map(|s| s.containers.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default();

    let mut container = use_signal_sync(|| pick_default_container(&containers).unwrap_or_default());
    let mut follow = use_signal_sync(|| true);
    let mut follow_state = use_signal_sync(|| FollowState::Following);
    let mut at_bottom = use_signal_sync(|| true);
    let mut lines = use_signal_sync(LineBuffer::default);

    // Task slot: holds the in-flight drain `JoinHandle` so re-runs (container
    // change, follow toggle, pod change) abort the prior task before spawning
    // fresh. Skill: OKT-51 reflector-leak pattern.
    let mut task_slot = use_hook(|| CopyValue::new(None::<JoinHandle<()>>));

    // Snapshot the reactive inputs the effect subscribes to. `container_for_effect`,
    // `follow_for_effect`, `lines_for_effect`, `follow_state_for_effect` are aliased
    // once (Skill: `Signal<T>` is `Copy`; signal writes inside a use_effect that
    // captured the signal by value would E0507-move it).
    use_effect(move || {
        let container_name = container();
        let should_follow = follow();
        if container_name.is_empty() {
            return;
        }
        // Wipe the buffer on every fresh open (container / follow / pod change).
        lines.write().clear();

        // Abort any prior drain task before spawning a replacement.
        if let Some(handle) = task_slot.write().take() {
            handle.abort();
        }

        let Some(client) = crate::runtime::client() else {
            return;
        };
        let api: Api<Pod> = Api::namespaced(client, &pod_ns);
        let name = pod_name.clone();
        let cont = container_name.clone();

        // Pending buffer between the line-drain task and the 50ms tick that
        // flushes into the Dioxus signal. Caps signal writes at ~20/sec on a
        // chatty pod (Dioxus absorbs that without dropping frames).
        let pending: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let handle = tokio::spawn(async move {
            let opts = LogOptions {
                container: Some(cont),
                follow: should_follow,
                tail_lines: Some(5000),
                timestamps: true,
            };
            let reader = match LogStream::new(api, name, opts).open().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "log stream open failed");
                    return;
                }
            };

            let mut line_stream = reader.lines();
            let mut ticker = tokio::time::interval(Duration::from_millis(50));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    biased;
                    next = line_stream.next() => {
                        match next {
                            Some(Ok(text)) => {
                                pending.lock().await.push(text);
                            }
                            Some(Err(e)) => {
                                tracing::warn!(error = %e, "log line read failed");
                                break;
                            }
                            None => break,
                        }
                    }
                    _ = ticker.tick() => {
                        let drained: Vec<String> = {
                            let mut guard = pending.lock().await;
                            std::mem::take(&mut *guard)
                        };
                        if drained.is_empty() {
                            continue;
                        }
                        let mut buf = lines.write();
                        for line in drained {
                            buf.push(line);
                        }
                        follow_state.write().resume();
                    }
                }
            }
            // Final flush: drain anything still pending.
            let drained: Vec<String> = {
                let mut guard = pending.lock().await;
                std::mem::take(&mut *guard)
            };
            if !drained.is_empty() {
                let mut buf = lines.write();
                for line in drained {
                    buf.push(line);
                }
            }
        });

        *task_slot.write() = Some(handle);
    });

    // Scroll listener: tiny JS handler writes a window-level flag on every
    // scroll event. A 100ms host-side poll reads the flag into `at_bottom`.
    // Skill: `use_effect` re-runs do NOT stop previously spawned tasks — we
    // install the listener once, in an effect that does no async work, and
    // let a `spawn`ed task own the polling lifetime.
    use_effect(move || {
        // One-time install guarded via a window flag so route changes don't
        // re-attach duplicate listeners. (Pattern from OKT-48 JsRouteSlot.)
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
        // Cheap 100ms poll of the JS-set global. The JS side writes the
        // global on every scroll event, so the viewer's "show paused hint"
        // lags by at most 100ms.
        spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
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

    // Auto-pause when the user scrolls up; auto-resume (and scroll to bottom)
    // when they scroll back. Effect subscribes to `at_bottom`; the body
    // never writes a signal.
    use_effect(move || {
        let bottom = at_bottom();
        if follow_state().is_following() && !bottom {
            follow_state.write().pause();
        } else if !follow_state().is_following() && bottom && follow() {
            follow_state.write().resume();
        }
    });

    // Sync the explicit follow checkbox into `follow_state`. The checkbox
    // is the user's authoritative intent; auto-pause from scrolling only
    // sets `follow_state` (the visual hint), and the explicit follow toggle
    // is what the user wants next.
    use_effect(move || {
        let f = follow();
        if f && !follow_state().is_following() {
            follow_state.write().resume();
        } else if !f && follow_state().is_following() {
            follow_state.write().pause();
        }
    });

    // Auto-scroll to bottom when following + at-bottom. Effect, not render body.
    use_effect(move || {
        if follow_state().is_following() && at_bottom() {
            let _ = document::eval(
                r#"var el = document.querySelector('.log-body');
                   if (el) { el.scrollTop = el.scrollHeight; }"#,
            );
        }
    });

    // Precompute the (line, class) tuples for the rsx! for-loop (skill:
    // precompute Vec<T> outside rsx!).
    let lines_snapshot: Vec<(String, &'static str)> = {
        let buf = lines.read();
        buf.lines()
            .iter()
            .map(|l| (l.clone(), level_class(l)))
            .collect()
    };
    let show_paused = should_show_paused_hint(follow_state(), at_bottom());

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
                        let mut buf = lines.write();
                        clear_and_reset(&mut buf);
                    },
                    "Clear"
                }
                button {
                    class: "btn btn-secondary",
                    style: "min-height: 28px; padding: 0 8px; font-size: 12px;",
                    onclick: move |_| {
                        *crate::runtime::SELECTED_POD.write() = Some(pod.clone());
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
