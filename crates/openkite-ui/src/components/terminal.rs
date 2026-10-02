//! Vendored xterm.js terminal surface: the bundle bootstrap/mount/reset JS the
//! host evals, and the toolbar + mount point both hosts render.
//!
//! The bundle lives in `assets/vendored/xterm/` (see its `SOURCE.txt` for the
//! rebuild command) and exposes three functions on `window.openkite`:
//! `_term_mount(selector)`, `_term_reset(selector)`, `_term_writeln(selector,
//! text)`. The builders are pure string functions so the selector plumbing is
//! unit-testable without a Dioxus runtime.
//!
//! [`TerminalView`] renders only when the host advertises a terminal through
//! the capability descriptor ([`crate::runtime::terminal_can_render`]); a host
//! that reports none gets [`TerminalUnsupported`] rather than a toolbar with no
//! host behind it. The connect handshake and the input drain are
//! `#[cfg(not(target_arch = "wasm32"))]`: they need a host timer and a shell,
//! and the descriptor is what decides whether a host runs them at all.

use dioxus::prelude::*;
use openkite_api::pod::{pick_default_container, PodObject};

use crate::runtime::{terminal_can_render, SELECTED_POD};

/// Cache-buster id for the vendored xterm bundle. Bump the `vN` suffix
/// whenever the bundle in `assets/vendored/xterm/` is rebuilt (the
/// `SOURCE.txt` in that directory records the rebuild command). The
/// bootstrap effect reads this id to guard the
/// `window.__openkite_xterm_loaded` flag and the `<html>` class.
pub fn xterm_host_path() -> &'static str {
    "xterm-bundle-v1"
}

/// The JS that injects the vendored CSS + JS bundle once per webview load.
///
/// CSS is guarded by an `<html>` class (so a re-render never appends a
/// second stylesheet); the bundle eval is guarded by a window flag. Both
/// guards are keyed off the cache-buster id, so bumping
/// [`xterm_host_path`] forces the new bundle to load.
pub fn bootstrap_js(cache_id: &str) -> String {
    let css = include_str!("../../assets/vendored/xterm/xterm.css");
    let js = include_str!("../../assets/vendored/xterm/xterm.js");
    let css_json = serde_json::to_string(css).unwrap_or_else(|_| "\"\"".into());
    let css_marker = format!("openkite-xterm-css-{cache_id}");
    format!(
        r#"if (!document.documentElement.classList.contains('{css_marker}')) {{
    document.documentElement.classList.add('{css_marker}');
    var s = document.createElement('style');
    s.textContent = {css_json};
    document.head.appendChild(s);
}}
if (!window.__openkite_xterm_loaded) {{
    window.__openkite_xterm_loaded = '{cache_id}';
    {js}
}}"#,
        css_marker = css_marker,
        css_json = css_json,
        cache_id = cache_id,
        js = js,
    )
}

/// The JS that mounts the xterm instance into the host div and forwards
/// keystrokes into the `window.__openkite_term_input` poll global.
///
/// The mount is async with respect to the bundle eval, so the snippet
/// retries up to 64 frames (16ms each) until `_term_mount` exists and the
/// host div is in the DOM. The `__openkite_term_id` stash keeps re-runs
/// from stacking a second Terminal.
pub fn mount_js(selector: &str) -> String {
    format!(
        r#"(function() {{
    var sel = {selector:?};
    function tryMount(retries) {{
        if (window.openkite && typeof window.openkite._term_mount === 'function') {{
            var root = document.querySelector(sel);
            if (root) {{
                if (!root.__openkite_term_id) {{
                    root.__openkite_term_id = window.openkite._term_mount(sel);
                    var t = root.__openkite_term_id;
                    if (t && t.onData) {{
                        t.onData(function(data) {{
                            window.__openkite_term_input = (window.__openkite_term_input || "") + data;
                        }});
                    }}
                }}
            }}
            return;
        }}
        if (retries <= 0) return;
        setTimeout(function() {{ tryMount(retries - 1); }}, 16);
    }}
    tryMount(64);
}})();"#,
        selector = selector,
    )
}

/// The JS that disposes the xterm instance and clears the host div.
pub fn reset_js(selector: &str) -> String {
    format!(
        r#"(function() {{
    var root = document.querySelector({selector:?});
    if (!root) return;
    var t = root.__openkite_term_id;
    if (t && typeof t.dispose === 'function') {{ t.dispose(); }}
    root.innerHTML = "";
    root.__openkite_term_id = null;
}})();"#,
        selector = selector,
    )
}

/// The JS that writes one line into the terminal (used for the
/// bridge-pending hint and error rendering).
pub fn writeln_js(selector: &str, text: &str) -> String {
    let text_json = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into());
    format!(
        "window.openkite._term_writeln({selector:?}, {text_json});",
        selector = selector,
        text_json = text_json,
    )
}

/// The connect lifecycle the toolbar renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalPhase {
    /// No pod / no reconnect attempt yet.
    Disconnected,
    /// Reconnect clicked; exec request in flight.
    Connecting,
    /// Exec request answered `ok`; output streaming is the Phase-1 follow-up.
    Connected,
    /// The host answered the deferred Phase-1 exec contract — the terminal
    /// renders, input is captured, and the toolbar says the bridge is pending.
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

/// Whether a bridge error string is the deferred Phase-1 exec contract.
///
/// Matches `"exec is not supported yet"` exactly, plus any forward-compat
/// `"exec (pending …)"` prefix the Phase-1 implementation might use.
/// Deliberately does NOT over-match (other `exec:`-prefixed errors are real
/// failures, not the deferred seam).
pub fn is_bridge_pending_error(message: &str) -> bool {
    message == "exec is not supported yet" || message.starts_with("exec (pending")
}

/// Resolve the shell an exec request asks for: `$SHELL`, falling back to
/// `cmd` (Windows) or `sh` (Unix) when unset or empty.
pub fn resolve_shell(shell_env: Option<&str>, is_windows: bool) -> String {
    shell_env
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if is_windows {
                "cmd".to_string()
            } else {
                "sh".to_string()
            }
        })
}

/// The selected pod's name, empty names reading as "no pod".
///
/// Mirrors the logs viewer's "open from inspector" hand-off: the inspector
/// writes [`SELECTED_POD`]; both standalone views read it.
fn selected_pod_name(pod: &Option<PodObject>) -> Option<String> {
    pod.as_ref()
        .map(|pod| pod.name.clone())
        .filter(|name| !name.is_empty())
}

/// What a host without a terminal capability renders in place of the surface.
#[component]
pub fn TerminalUnsupported() -> Element {
    rsx! {
        div { class: "view view-terminal-unsupported",
            h2 { "Terminal not available" }
            p { "This host does not advertise the terminal surface." }
            p { "The in-app terminal renders only when the host reports a terminal capability; the browser host reports none. Run the desktop app to exec into a pod." }
        }
    }
}

/// The standalone terminal surface: pod/container picker, reconnect state
/// machine, and the vendored xterm.js mount point.
#[component]
pub fn TerminalView() -> Element {
    #[cfg(not(target_arch = "wasm32"))]
    use std::time::Duration;

    if !terminal_can_render() {
        return rsx! { TerminalUnsupported {} };
    }

    let pod: Option<PodObject> = SELECTED_POD.read().clone();
    let pod_name = selected_pod_name(&pod);
    let containers: Vec<String> = pod
        .as_ref()
        .map(PodObject::container_names)
        .unwrap_or_default();

    let mut phase = use_signal_sync(|| TerminalPhase::Disconnected);
    let mut container = use_signal_sync(|| pick_default_container(&containers).unwrap_or_default());
    let last_error = use_signal_sync(String::new);

    #[cfg(not(target_arch = "wasm32"))]
    let mut fetch_slot = use_hook(|| CopyValue::new(None::<dioxus::core::Task>));

    let instance_id = use_hook(|| {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    });
    let cache_id = xterm_host_path();
    let data_attr = format!("{cache_id}-{instance_id}");
    let selector = format!("[data-term-host=\"{data_attr}\"]");

    use_effect(move || {
        let _ = document::eval(&bootstrap_js(cache_id));
    });

    #[cfg(not(target_arch = "wasm32"))]
    use_effect(move || {
        if let Some(task) = fetch_slot.write().take() {
            task.cancel();
        }
        if phase() != TerminalPhase::Connecting {
            return;
        }
        let Some(pod) = SELECTED_POD.read().clone() else {
            phase.set(TerminalPhase::Disconnected);
            return;
        };
        let name = pod.name.clone();
        let ns = pod.namespace.clone().unwrap_or_else(|| "default".into());
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

    #[cfg(not(target_arch = "wasm32"))]
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
                // The exec channel consumes the capture here once a host
                // advertises exec; clearing it keeps the global bounded.
                let _ = document::eval("window.__openkite_term_input = '';");
            }
        });
    });

    use_effect(move || {
        let current = phase();
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
                span { style: "font-size: 12px; color: var(--subtle);", "pod: {pod_label}" }
                select {
                    style: "font: inherit; font-size: 12px; padding: 4px 8px; border-radius: 6px; border: 1px solid var(--border); background: var(--surface-solid); color: var(--fg);",
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
            div { style: "flex: 1; min-height: 0; position: relative; overflow: hidden; background: var(--terminal-bg); border-radius: var(--r-md);",
                if show_empty_state {
                    span { style: "color: var(--subtle); position: absolute; inset: 0; display: flex; align-items: center; justify-content: center;",
                        "Pick a pod to start a terminal session (use the workload list or the inspector)."
                    }
                }
                div { "data-term-host": "{data_attr}", style: "position: absolute; inset: 0;" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xterm_host_path_is_stable() {
        assert_eq!(xterm_host_path(), "xterm-bundle-v1");
    }

    #[test]
    fn mount_js_uses_the_given_selector() {
        let js = mount_js("[data-term-host=\"xterm-bundle-v1-1\"]");
        assert!(js.contains("_term_mount"));
        assert!(js.contains("tryMount(64)"));
        assert!(js.contains("__openkite_term_input"));
        assert!(js.contains(r#"var sel = "[data-term-host=\"xterm-bundle-v1-1\"]";"#));
        assert!(js.contains("document.querySelector(sel)"));
    }

    #[test]
    fn reset_js_disposes_and_clears() {
        let js = reset_js("[data-term-host=\"x\"]");
        assert!(js.contains("dispose"));
        assert!(js.contains("__openkite_term_id = null"));
        assert!(js.contains("[data-term-host=\\\"x\\\"]"));
    }

    #[test]
    fn writeln_js_escapes_the_text_payload() {
        let js = writeln_js("[data-term-host=\"x\"]", "exec bridge pending");
        assert!(js.contains("_term_writeln"));
        assert!(js.contains("\"exec bridge pending\""));
    }

    #[test]
    fn bootstrap_js_injects_bundle_once_guarded() {
        let js = bootstrap_js("xterm-bundle-v1");
        assert!(js.contains("openkite-xterm-css-xterm-bundle-v1"));
        assert!(js.contains("__openkite_xterm_loaded"));
        assert!(js.contains("window.__openkite_xterm_loaded = 'xterm-bundle-v1';"));
    }

    #[test]
    fn phase_labels_cover_every_phase() {
        assert_eq!(phase_label(&TerminalPhase::Disconnected), "Disconnected");
        assert_eq!(phase_label(&TerminalPhase::Connecting), "Connecting…");
        assert_eq!(phase_label(&TerminalPhase::Connected), "Connected");
        assert_eq!(
            phase_label(&TerminalPhase::BridgePending),
            "Bridge pending Phase 1"
        );
        assert_eq!(phase_label(&TerminalPhase::Error("x".into())), "Error");
    }

    #[test]
    fn bridge_pending_matches_only_the_deferred_contract() {
        assert!(is_bridge_pending_error("exec is not supported yet"));
        assert!(is_bridge_pending_error("exec (pending Phase 1)"));
        assert!(!is_bridge_pending_error("no cluster connected"));
        assert!(!is_bridge_pending_error("exec: pod not found"));
        assert!(!is_bridge_pending_error(""));
    }

    #[test]
    fn resolve_shell_prefers_env() {
        assert_eq!(resolve_shell(Some("/bin/zsh"), false), "/bin/zsh");
    }

    #[test]
    fn resolve_shell_falls_back_by_platform() {
        assert_eq!(resolve_shell(None, false), "sh");
        assert_eq!(resolve_shell(None, true), "cmd");
        assert_eq!(resolve_shell(Some(""), false), "sh");
    }

    #[test]
    fn selected_pod_name_reads_the_owned_contract() {
        let pod = PodObject {
            name: "web-1".into(),
            ..PodObject::default()
        };
        assert_eq!(selected_pod_name(&Some(pod)), Some("web-1".to_string()));
        assert_eq!(
            selected_pod_name(&Some(PodObject::default())),
            None,
            "a pod with an empty name is not a selection"
        );
        assert_eq!(selected_pod_name(&None), None);
    }
}
