//! Standalone terminal view: the toolbar (pod/container picker, reconnect /
//! disconnect), the phase state machine, and a vendored xterm.js host.
//!
//! The pure-logic helpers at the top of this file are testable without a
//! Dioxus runtime; the `#[component]` lives at the bottom and depends on the
//! `dioxus::prelude` glob (split per the openkite-dev skill §"Split pure
//! logic from the Dioxus view"). Reuses the P1 surface in `crate::terminal`
//! (`resolve_shell`, `OutputBuffer`) and the typed `ApiRequest::Exec` wire
//! envelope from `crate::plugin_api`.

use dioxus::prelude::*;

use crate::components::terminal::{bootstrap_js, mount_js, reset_js, writeln_js, xterm_host_path};
use crate::terminal::{resolve_shell, OutputBuffer};

/// The connect lifecycle the toolbar renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalPhase {
    /// No pod / no reconnect attempt yet.
    Disconnected,
    /// Reconnect clicked; fetch in flight.
    Connecting,
    /// Bridge returned `Ok`; output streaming (Phase 1 follow-up).
    Connected,
    /// Bridge returned `Err("exec is not supported yet")` — the deferred
    /// Phase-1 contract. The view renders the typed input + shows a
    /// "bridge pending Phase 1" hint in the toolbar.
    BridgePending,
    /// Any other bridge error (network, RBAC, etc.) — message for the toolbar.
    Error(String),
}

/// The toolbar's phase label.
pub fn phase_label(phase: &TerminalPhase) -> &'static str {
    match phase {
        TerminalPhase::Disconnected => "Disconnected",
        TerminalPhase::Connecting => "Connecting…",
        TerminalPhase::Connected => "Connected",
        TerminalPhase::BridgePending => "Bridge pending Phase 1",
        TerminalPhase::Error(_) => "Error",
    }
}

/// The pod's name from the shared selection signal.
///
/// Mirrors the logs viewer's "open from inspector" hand-off: the inspector
/// writes `runtime::SELECTED_POD`; the standalone views read it.
pub fn parse_pod_name(pod: &Option<k8s_openapi::api::core::v1::Pod>) -> Option<String> {
    pod.as_ref().and_then(|p| p.metadata.name.clone())
}

/// The first non-empty container name, or `None` if the list is empty.
///
/// The terminal's container `<select>` defaults to this; the logs viewer's
/// `pick_default_container` is the same logic. A follow-up consolidates if
/// a third caller appears.
pub fn default_container(containers: &[String]) -> Option<String> {
    containers.iter().find(|c| !c.is_empty()).cloned()
}

/// Drain one coalesced chunk from an `OutputBuffer`.
///
/// The Phase-1 exec channel will own an `OutputBuffer` per connection and
/// emit chunks here; today the helper is a thin re-export of
/// `OutputBuffer::next_chunk` so the test can pin the contract.
pub fn drain_output_buffer(buffer: &mut OutputBuffer) -> Vec<u8> {
    buffer.next_chunk().unwrap_or_default()
}

/// Whether a bridge error string is the deferred Phase-1 exec contract.
///
/// Matches the current `"exec is not supported yet"` exactly, plus any
/// forward-compat `"exec (pending …)"` prefix the Phase-1 implementation
/// might use. Deliberately does NOT over-match (other `exec:`-prefixed
/// errors are real failures, not the deferred seam).
pub fn is_bridge_pending_error(message: &str) -> bool {
    message == "exec is not supported yet" || message.starts_with("exec (pending")
}

#[component]
pub fn TerminalView() -> Element {
    use k8s_openapi::api::core::v1::Pod;
    use std::time::Duration;

    // The pod comes from the shared `SELECTED_POD` signal (the same one the
    // inspector writes). A direct pod picker is a follow-up; the "open from
    // inspector" hand-off is the entry point.
    let pod: Option<Pod> = crate::runtime::SELECTED_POD.read().clone();

    let pod_name = parse_pod_name(&pod);
    let containers: Vec<String> = pod
        .as_ref()
        .and_then(|p| p.spec.as_ref())
        .map(|s| s.containers.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default();

    let mut phase = use_signal_sync(|| TerminalPhase::Disconnected);
    let mut container = use_signal_sync(|| default_container(&containers).unwrap_or_default());
    let last_error = use_signal_sync(String::new);

    // Task slot: holds the in-flight exec fetch `Task` so re-runs (reconnect
    // click, container change) cancel the prior task before spawning fresh.
    // Skill: OKT-51 reflector-leak pattern.
    let mut fetch_slot = use_hook(|| CopyValue::new(None::<dioxus::core::Task>));

    // Stable per-mount instance id for the host div's data-term-host
    // attribute (the OKT-37 CodeMirror shape).
    let instance_id = use_hook(|| {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    });
    let cache_id = xterm_host_path();
    let data_attr = format!("{cache_id}-{instance_id}");
    let selector = format!("[data-term-host=\"{data_attr}\"]");
    let selector_for_effect = selector.clone();

    // Bootstrap effect: inject the vendored CSS + JS bundle once per
    // webview load. Mount-once: not subscribed to props or signals.
    use_effect(move || {
        let _ = document::eval(&bootstrap_js(cache_id));
    });

    // Exec fetch effect. Subscribes to `phase`; fires only while
    // `Connecting`. POSTs the OKT-46 wire envelope via same-origin fetch
    // into the `/openkite` asset handler; the bridge rejects with the
    // deferred Phase-1 exec error, which flips the view to `BridgePending`.
    //
    // The eval-based fetch runs on the Dioxus runtime (`spawn`, not
    // `tokio::spawn`) because `document::eval` needs the Dioxus runtime
    // context (thread-local). Same split as the logs viewer's drain task
    // (kube work in `tokio::spawn`) vs its poll loop (eval in `spawn`).
    use_effect(move || {
        // Cancel any prior fetch before spawning a replacement.
        if let Some(task) = fetch_slot.write().take() {
            task.cancel();
        }
        if phase() != TerminalPhase::Connecting {
            return;
        }
        let Some(pod) = crate::runtime::SELECTED_POD.read().clone() else {
            phase.set(TerminalPhase::Disconnected);
            return;
        };
        let name = pod.metadata.name.clone().unwrap_or_default();
        let ns = pod
            .metadata
            .namespace
            .clone()
            .unwrap_or_else(|| "default".into());
        let cont = container();
        let cmd = vec![resolve_shell(
            std::env::var("SHELL").ok().as_deref(),
            cfg!(windows),
        )];

        let mut phase_signal = phase;
        let mut err_signal = last_error;
        let task = spawn(async move {
            let payload = serde_json::json!({
                "id": 1,
                "plugin": "openkite-core",
                "request": {
                    "op": "exec",
                    "name": name,
                    "ns": ns,
                    "container": cont,
                    "cmd": cmd,
                }
            });
            let body = match serde_json::to_string(&payload) {
                Ok(b) => b,
                Err(e) => {
                    phase_signal.set(TerminalPhase::Error(format!("serialize: {e}")));
                    return;
                }
            };
            let body_json = serde_json::to_string(&body).unwrap_or_else(|_| "\"\"".into());
            let source = format!(
                r#"(async function() {{
                    try {{
                        const res = await fetch("/openkite", {{
                            method: "POST",
                            headers: {{ "Content-Type": "application/json" }},
                            body: {body_json}
                        }});
                        const json = await res.json();
                        window.__openkite_term_last_error = json.error || "";
                        window.__openkite_term_last_status = json.status || "";
                    }} catch (err) {{
                        window.__openkite_term_last_error = String(err);
                        window.__openkite_term_last_status = "error";
                    }}
                }})();"#,
                body_json = body_json,
            );
            let _ = document::eval(&source);
            // The eval is fire-and-forget; poll the globals the JS sets.
            // (The fetch settles within a frame or two; 6 polls x 25ms.)
            let mut status = String::new();
            let mut error = String::new();
            for _ in 0..6 {
                tokio::time::sleep(Duration::from_millis(25)).await;
                let raw = document::eval(
                    r#"JSON.stringify({
                        status: window.__openkite_term_last_status || "",
                        error: window.__openkite_term_last_error || ""
                    });"#,
                )
                .recv::<String>()
                .await
                .unwrap_or_default();
                let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
                let st = parsed
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !st.is_empty() {
                    status = st;
                    error = parsed
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    break;
                }
            }
            err_signal.set(error.clone());
            if status == "ok" {
                phase_signal.set(TerminalPhase::Connected);
            } else if is_bridge_pending_error(&error) {
                phase_signal.set(TerminalPhase::BridgePending);
            } else if error.is_empty() {
                phase_signal.set(TerminalPhase::Error("no bridge response".into()));
            } else {
                phase_signal.set(TerminalPhase::Error(error));
            }
        });
        *fetch_slot.write() = Some(task);
    });

    // Input poll: reads the xterm `onData` accumulator every 50ms. Today
    // the keystrokes are only logged — the Phase-1 exec channel dispatches
    // them. The seam is marked with the deferred-contract TODO.
    use_effect(move || {
        spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let raw = document::eval("window.__openkite_term_input || ''")
                    .recv::<String>()
                    .await
                    .unwrap_or_default();
                if raw.is_empty() {
                    continue;
                }
                // Clear the accumulator so each keystroke batch is seen once.
                let _ = document::eval("window.__openkite_term_input = '';");
                // TODO (Phase 1 exec): dispatch `raw` over the exec channel.
                tracing::info!(
                    bytes = raw.len(),
                    "terminal input captured (Phase 1 exec pending)"
                );
            }
        });
    });

    // Phase effect: reconcile the xterm host with the toolbar state.
    // Eval work belongs on the Dioxus runtime, so this is a plain effect
    // (mount / reset / writeln one-shots) — no spawned task needed.
    use_effect(move || {
        let current = phase();
        // `selector_for_effect` is captured by the FnMut closure; borrow it
        // (E0507 — the String cannot be moved out of the capture on each run).
        let selector = selector_for_effect.clone();
        match current {
            TerminalPhase::Disconnected => {
                let _ = document::eval(&reset_js(&selector));
            }
            TerminalPhase::Connecting => {
                let _ = document::eval(&mount_js(&selector));
            }
            TerminalPhase::Connected => {
                let _ = document::eval(&mount_js(&selector));
            }
            TerminalPhase::BridgePending => {
                let _ = document::eval(&mount_js(&selector));
                let _ = document::eval(&writeln_js(
                    &selector,
                    "exec bridge pending Phase 1 — input is captured but not yet dispatched",
                ));
            }
            TerminalPhase::Error(_) => {
                let _ = document::eval(&mount_js(&selector));
                let msg = last_error();
                if !msg.is_empty() {
                    let _ = document::eval(&writeln_js(&selector, &format!("error: {msg}")));
                }
            }
        }
    });

    let phase_now = phase();
    let pod_label = pod_name
        .clone()
        .unwrap_or_else(|| "(none — open from inspector)".into());
    let show_empty_state = phase_now == TerminalPhase::Disconnected && pod_name.is_none();

    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 8px; height: 100%; box-sizing: border-box; padding: 12px 16px;",
            div { style: "display: flex; gap: 8px; align-items: center; flex-wrap: wrap;",
                span { style: "font-size: 12px; color: var(--fg-2);", "pod: {pod_label}" }
                select {
                    style: "font: inherit; font-size: 12px; padding: 4px 8px; border-radius: 6px; border: 1px solid var(--border); background: var(--bg-2); color: var(--fg-0);",
                    value: "{container}",
                    oninput: move |e| container.set(e.value()),
                    for c in containers.iter() {
                        option { value: "{c}", "{c}" }
                    }
                }
                button {
                    class: "btn btn-secondary",
                    style: "min-height: 28px; padding: 0 8px; font-size: 12px;",
                    onclick: move |_| phase.set(TerminalPhase::Connecting),
                    "Reconnect"
                }
                button {
                    class: "btn btn-secondary",
                    style: "min-height: 28px; padding: 0 8px; font-size: 12px;",
                    onclick: move |_| phase.set(TerminalPhase::Disconnected),
                    "Disconnect"
                }
                span { class: "term-status", "{phase_label(&phase_now)}" }
            }
            div { style: "flex: 1; min-height: 0; position: relative; overflow: hidden; background: var(--term-bg); border-radius: var(--r-md);",
                if show_empty_state {
                    span { style: "color: var(--fg-2); position: absolute; inset: 0; display: flex; align-items: center; justify-content: center;",
                        "Pick a pod to start a terminal session (use the workload list or the inspector)."
                    }
                }
                div { "data-term-host": "{data_attr}", style: "position: absolute; inset: 0;" }
            }
        }
    }
}
