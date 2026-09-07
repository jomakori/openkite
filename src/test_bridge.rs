//! Test-only DOM bridge (OKT-64).
//!
//! OpenKite is a Dioxus *desktop* app (wry/WebKitGTK): there is no web
//! platform, so browser automation (Playwright/Selenium/tauri-driver)
//! cannot attach — dioxus-desktop 0.7.10 hardcodes WebKit automation off
//! and owns the wry `WebContext` privately. The established E2E harness
//! (e2e/) instead drives X11 input with xdotool and asserts pixels.
//!
//! This module is the in-app half of a DOM-selector bridge: when the app
//! is booted with `OPENKITE_TEST_PORT=<port>` it serves a tiny localhost
//! HTTP endpoint that runs `document::eval` inside the live webview and
//! returns the serialized result. Tests (bats + curl) can then query and
//! drive the real DOM with CSS selectors — no coordinates, no pixel
//! diffing for interactions.
//!
//! Request/response model
//! ----------------------
//! POST /  with a JSON body:  { "selector": ".palette", "op": "count" }
//!
//! Ops (all evaluated against the FIRST match unless noted):
//!   count   -> number of matches for the selector
//!   text    -> textContent of the first match (trimmed)
//!   visible -> bool: match exists AND offsetParent != null
//!   class   -> className of the first match
//!   click   -> dispatch a real MouseEvent click on the first match
//!   focus   -> .focus() on the first match
//!   type    -> requires { "text": "..." }: set value + dispatch
//!              input/change events (for controlled inputs)
//!   key     -> requires { "key": "Escape"|"Enter"|... } (+ optional
//!              "ctrl"/"meta" booleans): dispatch a KeyboardEvent on
//!              document (keydown, bubbles). Prefer this over XTEST for
//!              webview keybinds (palette Ctrl+P / Escape).
//!
//! Response: { "ok": true, "result": <op-specific> }
//! or        { "ok": false, "error": "<message>" }
//!
//! Threading model (why it looks like this)
//! ----------------------------------------
//! `document()` (and thus `document::eval`) resolves the dioxus runtime
//! from a thread-local, and the underlying `evaluate_script` must run on
//! the UI thread. So every op is executed by a worker component rendered
//! in the app shell, whose `use_future` future is polled on the UI
//! thread (the same context the Dioxus org's own desktop eval tests use
//! — packages/desktop/headless_tests/eval.rs). Each HTTP request is
//! forwarded over an mpsc; the worker runs one fresh
//! `document::eval(js)` per op and `join`s its completion value
//! (Dioxus's proven value-return path). The HTTP listener (plain tokio
//! task) is just a mailbox.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

/// One op forwarded from the HTTP listener to the UI-thread worker.
#[derive(Debug, Serialize, Deserialize)]
pub struct Op {
    pub selector: String,
    pub op: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub meta: bool,
}

/// Worker inbox (UI thread). The listener sends parsed ops here.
static INBOX: std::sync::OnceLock<
    mpsc::UnboundedSender<(Op, tokio::sync::oneshot::Sender<String>)>,
> = std::sync::OnceLock::new();

/// Receiver half, claimed by the worker's `use_future` on first poll.
/// Stored so the `FnMut` future initializer (re-run every render) does
/// not try to move the receiver out of its own closure. `OnceLock` can't
/// be used here: `take()` needs `&mut self`, impossible on a static.
static INBOX_RX: std::sync::Mutex<
    Option<mpsc::UnboundedReceiver<(Op, tokio::sync::oneshot::Sender<String>)>>,
> = std::sync::Mutex::new(None);

/// Port the bridge listens on, when enabled (0 = disabled).
fn requested_port() -> Option<u16> {
    let raw = std::env::var("OPENKITE_TEST_PORT").ok()?;
    let port: u16 = raw.trim().parse().ok()?;
    (port != 0).then_some(port)
}

/// Whether the bridge is enabled for this process.
pub fn enabled() -> bool {
    requested_port().is_some()
}

/// Start the HTTP listener. Called from `run()` before the desktop event
/// loop launches, on the bootstrap runtime's handle (a bare `tokio::spawn`
/// would panic here — the main thread is not inside a runtime context).
pub fn install_bridge(handle: &tokio::runtime::Handle) {
    let Some(port) = requested_port() else { return };
    let addr: SocketAddr = format!("127.0.0.1:{port}")
        .parse()
        .expect("bad OPENKITE_TEST_PORT");
    let rt = handle.clone();
    handle.spawn(async move {
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(%e, %port, "test bridge bind failed");
                return;
            }
        };
        tracing::info!(%port, "test bridge listening (OKT-64)");
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                continue;
            };
            let rt = rt.clone();
            rt.spawn(async move {
                if let Err(e) = handle_conn(&mut sock).await {
                    tracing::debug!(%e, "test bridge conn error");
                }
            });
        }
    });
}

/// Read one HTTP request, parse the op, forward it to the UI-thread
/// worker, and write the JSON reply back.
async fn handle_conn(sock: &mut tokio::net::TcpStream) -> Result<(), String> {
    // The reader owns the stream for the duration of the request; the
    // response is written through `get_mut()` so no move/borrow juggling.
    let mut reader = BufReader::new(sock);
    let mut head = String::new();
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("eof before headers done".into());
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        head.push_str(&line);
    }
    if !head.starts_with("POST ") {
        return write_json(reader.get_mut(), r#"{"ok":false,"error":"POST only"}"#).await;
    }

    let clen: usize = head
        .lines()
        .find_map(|l| {
            let l = l.to_ascii_lowercase();
            l.strip_prefix("content-length:")
                .map(|v| v.trim().parse().unwrap_or(0))
        })
        .unwrap_or(0);

    let mut body = vec![0u8; clen];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|e| e.to_string())?;
    let op: Op = serde_json::from_slice(&body).map_err(|e| e.to_string())?;

    let Some(inbox) = INBOX.get() else {
        return write_json(
            reader.get_mut(),
            r#"{"ok":false,"error":"bridge worker not running"}"#,
        )
        .await;
    };
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    inbox
        .send((op, reply_tx))
        .map_err(|_| "worker dropped".to_string())?;
    let result = reply_rx.await.map_err(|_| "worker dropped".to_string())?;

    write_json(reader.get_mut(), &result).await
}

async fn write_json(sock: &mut tokio::net::TcpStream, payload: &str) -> Result<(), String> {
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        payload.len(),
        payload
    );
    sock.write_all(resp.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

/// Build the JS for one op. The eval wrapper runs the body as
/// `new AsyncFunction("dioxus", body)`, so the body must end in an
/// explicit `return <expr>;` for `Eval::join` to observe a value (see
/// dioxus-desktop query.rs).
fn op_js(op: &Op) -> Result<String, String> {
    let sel = serde_json::to_string(&op.selector).map_err(|e| e.to_string())?;
    let body = match op.op.as_str() {
        "count" => format!("return document.querySelectorAll({sel}).length;"),
        "text" => {
            format!(r#"return (document.querySelector({sel})?.textContent ?? "").trim();"#)
        }
        "visible" => format!(
            r#"return (() => {{ const el = document.querySelector({sel}); return el !== null && el.offsetParent !== null; }})();"#
        ),
        "class" => {
            format!(r#"return (document.querySelector({sel})?.className ?? "");"#)
        }
        "click" => format!(
            r#"return (() => {{ const el = document.querySelector({sel}); if (!el) return false; el.click(); return true; }})();"#
        ),
        "focus" => format!(
            r#"return (() => {{ const el = document.querySelector({sel}); if (!el) return false; el.focus(); return true; }})();"#
        ),
        "type" => {
            let val = serde_json::to_string(op.text.as_deref().unwrap_or(""))
                .map_err(|e| e.to_string())?;
            format!(
                r#"return (() => {{ const el = document.querySelector({sel}); if (!el) return false;
  el.focus();
  // Dioxus-style controlled inputs: set the native value setter then
  // dispatch input so the framework's onChange fires.
  const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')?.set;
  if (setter) setter.call(el, {val}); else el.value = {val};
  el.dispatchEvent(new Event('input', {{ bubbles: true }}));
  el.dispatchEvent(new Event('change', {{ bubbles: true }}));
  return true; }})();"#
            )
        }
        "key" => {
            let k = serde_json::to_string(op.key.as_deref().unwrap_or(""))
                .map_err(|e| e.to_string())?;
            let ctrl = if op.ctrl { "ctrlKey: true," } else { "" };
            let meta = if op.meta { "metaKey: true," } else { "" };
            format!(
                r#"return (() => {{ document.dispatchEvent(new KeyboardEvent('keydown', {{ key: {k}, {ctrl} {meta} bubbles: true, cancelable: true }})); return true; }})();"#
            )
        }
        other => return Err(format!("unknown op: {other}")),
    };
    Ok(body)
}

/// The UI-thread worker component. Rendered once by the app shell when
/// the bridge is enabled. Its `use_future` is polled on the UI thread —
/// the context where `document::eval` + `evaluate_script` are valid
/// (same pattern as DioxusLabs/dioxus packages/desktop/headless_tests).
/// Each request runs one fresh eval and joins its value.
#[component]
pub fn BridgeWorker() -> Element {
    // Create the channel pair once (component re-renders re-run the
    // body, but the statics only store on the first call).
    let (tx, rx) = mpsc::unbounded_channel::<(Op, tokio::sync::oneshot::Sender<String>)>();
    let _ = INBOX.set(tx);
    *INBOX_RX.lock().unwrap() = Some(rx);

    use_future(move || async move {
        // Claim the receiver on first poll. The future initializer may
        // run on later renders too; the Mutex makes the claim idempotent.
        let Some(mut rx) = INBOX_RX.lock().unwrap().take() else {
            return;
        };
        while let Some((op, reply)) = rx.recv().await {
            let js = match op_js(&op) {
                Ok(js) => js,
                Err(e) => {
                    let _ = reply.send(format!(
                        r#"{{"ok":false,"error":{}}}"#,
                        serde_json::to_string(&e).unwrap_or_default()
                    ));
                    continue;
                }
            };
            // One fresh eval per op; join waits for the completion value.
            // A stuck page can't hang the caller: bound the wait and reply
            // with an error instead.
            let eval = dioxus::document::eval(&js);
            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                eval.join::<serde_json::Value>(),
            )
            .await
            {
                Ok(Ok(value)) => {
                    let _ = reply.send(format!(r#"{{"ok":true,"result":{value}}}"#));
                }
                Ok(Err(e)) => {
                    let _ = reply.send(format!(
                        r#"{{"ok":false,"error":"bridge eval failed: {e}"}}"#
                    ));
                }
                Err(_) => {
                    let _ = reply.send(r#"{"ok":false,"error":"bridge eval timed out"}"#.into());
                }
            }
        }
    });

    VNode::empty()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_deserializes_with_defaults() {
        let op: Op = serde_json::from_str(r#"{"selector":".palette","op":"count"}"#).unwrap();
        assert_eq!(op.op, "count");
        assert!(!op.ctrl && !op.meta && op.text.is_none() && op.key.is_none());
    }

    #[test]
    fn op_round_trips_key_with_modifiers() {
        let op: Op =
            serde_json::from_str(r#"{"selector":"","op":"key","key":"p","ctrl":true}"#).unwrap();
        assert_eq!(op.key.as_deref(), Some("p"));
        assert!(op.ctrl);
        assert!(!op.meta);
    }

    #[test]
    fn op_js_builds_returning_bodies() {
        let op: Op = serde_json::from_str(r#"{"selector":".palette","op":"count"}"#).unwrap();
        let js = op_js(&op).unwrap();
        assert!(js.starts_with("return "), "eval body must return: {js}");
        assert!(js.contains("querySelectorAll"));
    }

    #[test]
    fn op_js_rejects_unknown_op() {
        let op: Op = serde_json::from_str(r#"{"selector":".x","op":"bogus"}"#).unwrap();
        assert!(op_js(&op).is_err());
    }

    #[test]
    fn disabled_when_env_unset() {
        unsafe { std::env::remove_var("OPENKITE_TEST_PORT") };
        assert!(!enabled());
    }
}
