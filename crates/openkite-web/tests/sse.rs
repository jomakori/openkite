//! `/openkite/events` SSE delivery test (OKT-122).
//!
//! Boots the host over a real TCP listener, opens a raw socket against the
//! SSE route, publishes a push message through [`openkite_host::push`], and
//! asserts the bytes the browser's `EventSource` would see land on the wire.

mod support;

use std::sync::Arc;
use std::time::Duration;

use openkite_host::bridge::Bridge;
use serde_json::json;
use support::{app, wait_until_serving};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn events_route_streams_a_published_push_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bridge = Arc::new(Bridge::new());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind host");
    let addr = listener.local_addr().expect("host address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app(bridge, dir.path())).await;
    });
    wait_until_serving(addr).await;

    // A subscription is what `publish` matches against; the SSE handler
    // itself has no filter of its own, but a publish with nothing listening
    // builds no message at all.
    let sub = {
        let mut reg = openkite_host::push::registry().lock().unwrap();
        reg.subscribe("pods", None)
    };

    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connect to host");
    let request =
        format!("GET /openkite/events HTTP/1.1\r\nhost: {addr}\r\nconnection: keep-alive\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("write request");

    // Read the response head first, so the handshake has landed before the
    // publish below — otherwise the assertion races the connection setup.
    let mut collected = String::new();
    let head_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !collected.contains("\r\n\r\n") {
        if tokio::time::Instant::now() >= head_deadline {
            panic!("sse handshake never completed: {collected}");
        }
        let mut buf = [0u8; 4096];
        let n = tokio::time::timeout(Duration::from_millis(500), stream.read(&mut buf))
            .await
            .expect("read before timeout")
            .expect("read headers");
        assert!(n > 0, "connection closed before the sse handshake");
        collected.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
    assert!(collected.contains("200 OK"), "sse handshake: {collected}");
    assert!(
        collected.to_lowercase().contains("text/event-stream"),
        "sse content-type: {collected}"
    );

    openkite_host::push::publish("pods", None, vec![json!({"metadata": {"name": "api-0"}})]);

    // Read until the published row shows up on the wire, or time out.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !collected.contains("api-0") {
        if tokio::time::Instant::now() >= deadline {
            panic!("sse stream never delivered the published push: {collected}");
        }
        let mut buf = [0u8; 4096];
        match tokio::time::timeout(Duration::from_millis(500), stream.read(&mut buf)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => collected.push_str(&String::from_utf8_lossy(&buf[..n])),
            Ok(Err(err)) => panic!("read error: {err}"),
            Err(_) => continue,
        }
    }

    assert!(
        collected.contains("event: message"),
        "sse event name: {collected}"
    );
    assert!(
        collected.contains("api-0"),
        "sse payload carried the published row: {collected}"
    );
    assert!(
        collected.contains(&format!("\"sub\":{sub}")),
        "sse payload addressed to the live subscription: {collected}"
    );

    let _ = openkite_host::push::registry()
        .lock()
        .unwrap()
        .unsubscribe(sub);
}
