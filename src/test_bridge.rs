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
//! from a thread-local — it is only valid on the UI thread. We therefore
//! install ONE long-lived eval on the UI thread (`install_bridge` is
//! called from the app-shell mount effect) whose JS side runs a
//! request/response loop: Rust sends an op over `eval.send`, JS executes
//! it against the real DOM and replies with `dioxus.send`. The HTTP
//! listener (plain tokio task) is just a mailbox: it forwards parsed ops
//! through an mpsc channel to the UI-thread worker and waits on a
//! oneshot for the reply. This mirrors the proven palette-keybind eval
//! pattern and keeps every `document` touch on the UI thread.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

/// One op forwarded from the HTTP listener to the UI-thread worker.
#[derive(Debug, Serialize, Deserialize)]
struct Op {
    selector: String,
    op: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    ctrl: bool,
    #[serde(default)]
    meta: bool,
}

/// Worker inbox (UI thread). The listener sends parsed ops here.
static INBOX: std::sync::OnceLock<
    mpsc::UnboundedSender<(Op, tokio::sync::oneshot::Sender<String>)>,
> = std::sync::OnceLock::new();

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
/// Parsed ops are forwarded to the UI-thread worker registered by
/// [`spawn_bridge_worker`].
pub fn install_bridge(handle: &tokio::runtime::Handle) {
    let Some(port) = requested_port() else { return };
    let addr: SocketAddr = format!("127.0.0.1:{port}")
        .parse()
        .expect("bad OPENKITE_TEST_PORT");
    // Two clones: the listener future moves one in, per-conn spawns use
    // the other. (Handle::spawn borrows its receiver, so a single clone
    // would E0505 — the future can't capture what the call borrows.)
    let rt_listener = handle.clone();
    let rt_conn = handle.clone();
    rt_listener.spawn(async move {
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
            rt_conn.spawn(async move {
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

/// The JS dispatcher installed once in the webview. It receives one op
/// per `dioxus.recv()`, runs it against the real DOM, and replies via
/// `dioxus.send`. Every result is JSON-serializable.
const DISPATCHER_JS: &str = r#"
async function __openkite_bridge_loop(dioxus) {
  while (true) {
    const raw = await dioxus.recv();
    const op = (typeof raw === 'string') ? JSON.parse(raw) : raw;
    try {
      const result = await __openkite_bridge_run(op);
      dioxus.send(JSON.stringify({ ok: true, result }));
    } catch (err) {
      dioxus.send(JSON.stringify({ ok: false, error: String(err) }));
    }
  }
}

async function __openkite_bridge_run(op) {
  const el = () => document.querySelector(op.selector);
  switch (op.op) {
    case 'count':
      return document.querySelectorAll(op.selector).length;
    case 'text':
      return (el()?.textContent ?? '').trim();
    case 'visible':
      return el() !== null && el().offsetParent !== null;
    case 'class':
      return el()?.className ?? '';
    case 'click': {
      const target = el();
      if (!target) return false;
      target.click();
      return true;
    }
    case 'focus': {
      const target = el();
      if (!target) return false;
      target.focus();
      return true;
    }
    case 'type': {
      const target = el();
      if (!target) return false;
      target.focus();
      const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')?.set;
      const value = op.text ?? '';
      if (setter) setter.call(target, value); else target.value = value;
      target.dispatchEvent(new Event('input', { bubbles: true }));
      target.dispatchEvent(new Event('change', { bubbles: true }));
      return true;
    }
    case 'key': {
      const init = { key: op.key ?? '', bubbles: true, cancelable: true };
      if (op.ctrl) init.ctrlKey = true;
      if (op.meta) init.metaKey = true;
      document.dispatchEvent(new KeyboardEvent('keydown', init));
      return true;
    }
    default:
      throw new Error('unknown op: ' + op.op);
  }
}

// Start the request pump AND keep this eval alive: the wrapper resolves
// the async body's promise when it settles, then closes the query
// (nulling window.__msg_queues[id], which Rust's eval.send needs). So we
// return the never-resolving loop promise — the body never settles, the
// query stays registered for the life of the process.
return __openkite_bridge_loop(dioxus);
"#;

/// Spawn the UI-thread worker. Call ONCE from the app shell's mount
/// effect — this thread owns the dioxus runtime, so the eval installed
/// here is the only one that can touch `document`. No-op when disabled.
pub fn spawn_bridge_worker() {
    if !enabled() {
        return;
    }
    let mut eval = dioxus::document::eval(DISPATCHER_JS);
    // Register the worker inbox BEFORE the first HTTP request can land.
    let (tx, mut rx) = mpsc::unbounded_channel::<(Op, tokio::sync::oneshot::Sender<String>)>();
    let _ = INBOX.set(tx);

    // UI-thread request pump: read one op, eval.send it, await the JS
    // reply, forward it to the HTTP listener's oneshot. A stuck dispatch
    // must not hang the caller forever, so the recv is time-boxed.
    dioxus::prelude::spawn(async move {
        while let Some((op, reply)) = rx.recv().await {
            if eval.send(op).is_err() {
                let _ = reply.send(
                    r#"{"ok":false,"error":"bridge eval channel closed (webview gone?)"}"#.into(),
                );
                continue;
            }
            match tokio::time::timeout(std::time::Duration::from_secs(5), eval.recv::<String>())
                .await
            {
                Ok(Ok(payload)) => {
                    let _ = reply.send(payload);
                }
                Ok(Err(e)) => {
                    let _ = reply.send(format!(
                        r#"{{"ok":false,"error":"bridge recv failed: {e}"}}"#
                    ));
                }
                Err(_) => {
                    let _ = reply.send(
                        r#"{"ok":false,"error":"bridge dispatch timed out (JS loop stuck?)"}"#
                            .into(),
                    );
                }
            }
        }
    });
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
    fn dispatcher_js_starts_the_pump() {
        // The eval body must `return` the never-resolving loop so the
        // wrapper never closes the query (close nulls window.__msg_queues
        // which eval.send depends on). Guard against accidental revert.
        assert!(
            DISPATCHER_JS.contains("return __openkite_bridge_loop(dioxus);"),
            "dispatcher must keep the eval alive via the returned loop promise"
        );
    }

    #[test]
    fn disabled_when_env_unset() {
        unsafe { std::env::remove_var("OPENKITE_TEST_PORT") };
        assert!(!enabled());
    }
}
