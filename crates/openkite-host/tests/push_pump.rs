//! The push pump end to end: an installed pump receives one message per
//! matching subscription.
//!
//! A separate test binary on purpose: [`PUSH_TX`](openkite_host::push) is a
//! process-global `OnceLock`, so the unit test that pins "no pump means no
//! delivery" would be order-dependent against an installed one in the same
//! process.

use openkite_host::push::{install, publish, registry};
use serde_json::json;

#[test]
fn installed_pump_receives_one_message_per_matching_subscription() {
    let mut rx = install().expect("the first install hands back the receiver");
    let sub = registry()
        .lock()
        .expect("registry lock")
        .subscribe("pods", None);

    let rows = vec![json!({"metadata": {"name": "api-0"}})];
    assert_eq!(publish("pods", None, rows.clone()), 1);

    let first = rx.try_recv().expect("one message queued");
    assert_eq!(first.sub, sub);
    assert_eq!(first.kind, "pods");
    assert_eq!(first.ns, None);
    assert_eq!(first.rows, rows);
    assert_eq!(first.revision, 1);

    assert_eq!(publish("pods", None, rows), 1);
    let second = rx.try_recv().expect("second message queued");
    assert_eq!(second.sub, sub);
    assert_eq!(
        second.revision, 2,
        "revisions are monotonic per subscription"
    );
}
