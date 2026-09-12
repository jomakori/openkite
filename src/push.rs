//! Additive Rust → JS push channel (OKT-91).
//!
//! The `/openkite` bridge in [`crate::plugin_api`] is strictly
//! request/response: `openkite.api.*` answers a call and `watch` resolves a
//! single one-shot snapshot. Phase 3 needs live cluster state (reflector
//! deltas) to reach the React layer without the page polling, so this module
//! adds a *push* direction without touching the existing wire contract.
//!
//! Shape:
//!
//! - JS calls `openkite.subscribe({kind, ns}, handler)` → a new
//!   `subscribe` op on the bridge. The host allocates a subscription id
//!   ([`PushRegistry::subscribe`]) and returns `{sub: <id>}`.
//! - The host calls [`publish`] whenever it has fresh rows for a kind. Each
//!   subscription matching `(kind, ns)` gets one [`PushMessage`].
//! - A pump running on the **Dioxus side** drains the channel and evaluates
//!   [`PushMessage::to_js`], which invokes `window.openkite._pushState(...)`
//!   and so the JS handler.
//!
//! Two hard constraints, both learned the hard way:
//!
//! 1. **The eval must happen on the Dioxus side.** `document::eval` resolves
//!    the current document through the thread-local Dioxus runtime; called
//!    from a tokio worker it silently returns the no-op document, so the push
//!    evaporates with no error (see `dioxus-07-rsx-gotchas` §13). The reflector
//!    side therefore only *sends*; the pump started by `AppShell` evals.
//! 2. **Only `GlobalSignal` needs the runtime.** A `Signal<_, SyncStorage>` is
//!    writable from any thread (the reflector in [`crate::state`] already does
//!    exactly this). The off-runtime panic in OKT-94 was specific to
//!    runtime-backed globals — do not "fix" the sync-signal writes.
//!
//! Coalescing: [`publish`] sends one message per matching subscription per
//! call. Callers that receive a burst of reflector events should coalesce
//! before publishing rather than emitting one push per event.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

/// One update destined for a single JS subscription.
///
/// Only `PartialEq` (not `Eq`): the payload holds `serde_json::Value`, whose
/// float representation does not admit a total equality.
#[derive(Debug, Clone, PartialEq)]
pub struct PushMessage {
    /// Subscription id handed out by [`PushRegistry::subscribe`].
    pub sub: u64,
    /// Resource kind (e.g. `pods`).
    pub kind: String,
    /// Namespace the subscription was opened for; `None` = all namespaces.
    pub ns: Option<String>,
    /// Serialised rows for this update.
    pub rows: Vec<Value>,
    /// Monotonic counter per subscription, so JS can drop out-of-order pushes.
    pub revision: u64,
}

impl PushMessage {
    /// The JS expression the Dioxus-side pump evaluates.
    ///
    /// Guarded so a page without the bridge installed (or mid-teardown) is a
    /// no-op rather than a thrown error inside the eval.
    pub fn to_js(&self) -> String {
        let payload = serde_json::json!({
            "sub": self.sub,
            "kind": self.kind,
            "ns": self.ns,
            "rows": self.rows,
            "revision": self.revision,
        });
        format!(
            "window.openkite && window.openkite._pushState && \
             window.openkite._pushState({payload});"
        )
    }
}

/// A live subscription: which kind/namespace it wants, and how many updates
/// have been published to it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Subscription {
    kind: String,
    ns: Option<String>,
    revision: u64,
}

/// Host-side registry of live subscriptions, keyed by subscription id.
#[derive(Debug, Default)]
pub struct PushRegistry {
    next_sub: u64,
    subs: BTreeMap<u64, Subscription>,
}

impl PushRegistry {
    /// Register interest in `kind` (optionally one namespace). Returns the
    /// subscription id to hand back to JS.
    pub fn subscribe(&mut self, kind: impl Into<String>, ns: Option<String>) -> u64 {
        self.next_sub += 1;
        let sub = self.next_sub;
        self.subs.insert(
            sub,
            Subscription {
                kind: kind.into(),
                ns,
                revision: 0,
            },
        );
        sub
    }

    /// Drop a subscription. Returns whether it existed.
    pub fn unsubscribe(&mut self, sub: u64) -> bool {
        self.subs.remove(&sub).is_some()
    }

    /// Number of live subscriptions.
    pub fn len(&self) -> usize {
        self.subs.len()
    }

    /// Whether no subscriptions are live.
    pub fn is_empty(&self) -> bool {
        self.subs.is_empty()
    }

    /// Subscription ids (and their namespace scope) currently watching `kind`.
    ///
    /// Namespace rules:
    /// - a subscription with `ns: None` wants every namespace and matches any
    ///   publish;
    /// - a namespaced publish (`Some`) matches a wildcard subscription and the
    ///   exact namespace, but not another namespace;
    /// - a global publish (`None`) matches every subscription watching the kind,
    ///   including namespaced ones. Those namespaced rows are narrowed before
    ///   delivery ([`deliver_rows`]) so a namespace-scoped view still receives
    ///   live updates even though the reflector publishes all namespaces.
    fn matching(&self, kind: &str, ns: Option<&str>) -> Vec<(u64, Option<String>)> {
        self.subs
            .iter()
            .filter(|(_, s)| {
                s.kind == kind
                    && match (&s.ns, ns) {
                        (None, _) => true,
                        (Some(_), None) => true,
                        (Some(want), Some(got)) => want == got,
                    }
            })
            .map(|(id, s)| (*id, s.ns.clone()))
            .collect()
    }

    /// Bump and return the next revision for `sub`.
    fn advance(&mut self, sub: u64) -> u64 {
        match self.subs.get_mut(&sub) {
            Some(s) => {
                s.revision += 1;
                s.revision
            }
            None => 0,
        }
    }
}

/// Channel into the Dioxus-side pump. Installed once by `AppShell`.
static PUSH_TX: OnceLock<UnboundedSender<PushMessage>> = OnceLock::new();

/// The registry static, created on first use.
static REGISTRY: OnceLock<Mutex<PushRegistry>> = OnceLock::new();

/// The shared registry.
pub fn registry() -> &'static Mutex<PushRegistry> {
    REGISTRY.get_or_init(|| Mutex::new(PushRegistry::default()))
}

/// Create the pump channel and hand the receiver to the caller.
///
/// Called from `AppShell` (inside the Dioxus runtime) on first mount; the
/// caller drives the returned receiver and evaluates each message. The sender
/// is stored here for the rest of the process. Idempotent: a second call
/// returns `None` so the pump is never started twice.
pub fn install() -> Option<UnboundedReceiver<PushMessage>> {
    let (tx, rx) = mpsc::unbounded_channel();
    match PUSH_TX.set(tx) {
        Ok(()) => Some(rx),
        Err(_) => None,
    }
}

/// Whether the pump channel exists yet.
pub fn is_installed() -> bool {
    PUSH_TX.get().is_some()
}

/// Narrow a publish's rows to one subscription's namespace.
///
/// A global publish carries every namespace, so a namespaced subscription must
/// filter. A namespaced publish is already scoped by the publisher. Cluster-
/// scoped objects (no `metadata.namespace`) survive either way, so a
/// namespace-scoped view of nodes never blanks out.
fn deliver_rows(rows: Vec<Value>, publish_ns: Option<&str>, sub_ns: Option<&str>) -> Vec<Value> {
    match (publish_ns, sub_ns) {
        (None, Some(ns)) if !ns.is_empty() => crate::state::live::filter_ns(rows, Some(ns)),
        _ => rows,
    }
}

/// Publish rows for `kind`/`ns` to every matching subscription.
///
/// Returns how many messages were queued. Zero means nobody was listening —
/// which is normal and not an error. Safe to call from any thread: it only
/// touches the registry lock and the channel.
pub fn publish(kind: &str, ns: Option<&str>, rows: Vec<Value>) -> usize {
    let messages = {
        let mut reg = match registry().lock() {
            Ok(reg) => reg,
            // A poisoned lock means another thread panicked mid-update. Skip
            // this publish rather than propagate the panic into the caller's
            // task: dropping a UI update is survivable, aborting a bridge
            // request is not.
            Err(poisoned) => poisoned.into_inner(),
        };
        let matched = reg.matching(kind, ns);
        matched
            .into_iter()
            .map(|(sub, sub_ns)| {
                let delivered = deliver_rows(rows.clone(), ns, sub_ns.as_deref());
                PushMessage {
                    sub,
                    kind: kind.to_string(),
                    ns: sub_ns.or_else(|| ns.map(str::to_string)),
                    rows: delivered,
                    revision: reg.advance(sub),
                }
            })
            .collect::<Vec<_>>()
    };

    let Some(tx) = PUSH_TX.get() else {
        // Pump not started (headless test, or a push before first mount).
        return 0;
    };
    let mut sent = 0;
    for msg in messages {
        if tx.send(msg).is_ok() {
            sent += 1;
        }
    }
    sent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribe_allocates_unique_ids_and_unsubscribe_removes() {
        let mut reg = PushRegistry::default();
        let a = reg.subscribe("pods", None);
        let b = reg.subscribe("pods", Some("default".into()));
        assert_ne!(a, b);
        assert_eq!(reg.len(), 2);
        assert!(reg.unsubscribe(a));
        assert!(!reg.unsubscribe(a), "second unsubscribe is a no-op");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn matching_honours_namespace_rules() {
        let mut reg = PushRegistry::default();
        let all = reg.subscribe("pods", None);
        let one = reg.subscribe("pods", Some("default".into()));
        let other = reg.subscribe("pods", Some("kube-system".into()));

        // A cluster-wide publish reaches every pods subscription.
        let mut got: Vec<u64> = reg
            .matching("pods", None)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        got.sort();
        assert_eq!(got, vec![all, one, other]);

        // A namespaced publish reaches the wildcard AND the exact match only.
        let mut got: Vec<u64> = reg
            .matching("pods", Some("default"))
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        got.sort();
        assert_eq!(got, vec![all, one]);
        assert!(!got.contains(&other));

        // Different kind matches nothing.
        assert!(reg.matching("secrets", Some("default")).is_empty());
    }

    #[test]
    fn deliver_rows_narrows_a_global_publish_to_a_namespaced_subscription() {
        let rows = vec![
            serde_json::json!({"metadata": {"namespace": "default", "name": "a"}}),
            serde_json::json!({"metadata": {"namespace": "kube-system", "name": "b"}}),
            serde_json::json!({"metadata": {"name": "node-1"}}),
        ];
        let namespaced = deliver_rows(rows.clone(), None, Some("default"));
        assert_eq!(namespaced.len(), 2, "default row + cluster-scoped node");
        assert!(namespaced
            .iter()
            .any(|row| row.pointer("/metadata/name").and_then(|v| v.as_str()) == Some("a")));
        assert!(namespaced
            .iter()
            .any(|row| row.pointer("/metadata/name").and_then(|v| v.as_str()) == Some("node-1")));

        assert_eq!(
            deliver_rows(rows.clone(), None, None).len(),
            3,
            "wildcard keeps all"
        );
        assert_eq!(
            deliver_rows(rows, Some("default"), Some("default")).len(),
            3,
            "an already-scoped publish passes through"
        );
    }

    #[test]
    fn revisions_increase_per_subscription() {
        let mut reg = PushRegistry::default();
        let sub = reg.subscribe("pods", None);
        assert_eq!(reg.advance(sub), 1);
        assert_eq!(reg.advance(sub), 2);
        // Unknown sub yields 0 rather than panicking.
        assert_eq!(reg.advance(999), 0);
    }

    #[test]
    fn to_js_is_guarded_and_carries_the_payload() {
        let msg = PushMessage {
            sub: 7,
            kind: "pods".into(),
            ns: Some("default".into()),
            rows: vec![serde_json::json!({ "name": "api-0" })],
            revision: 3,
        };
        let js = msg.to_js();
        assert!(js.contains("_pushState"), "routes through the JS shim");
        assert!(
            js.contains("window.openkite &&"),
            "guarded against a missing bridge"
        );
        assert!(js.contains("\"sub\":7"));
        assert!(js.contains("\"revision\":3"));
        assert!(js.contains("api-0"));
    }

    #[test]
    fn publish_without_a_pump_is_a_no_op_not_a_panic() {
        // No AppShell mount in a unit test, so there is no channel. This must
        // return 0 rather than panic — the whole point of the guard.
        let mut reg = registry().lock().unwrap();
        let sub = reg.subscribe("pods", None);
        drop(reg);
        let sent = publish("pods", None, vec![serde_json::json!({ "name": "x" })]);
        assert_eq!(sent, 0, "no pump installed ⇒ nothing sent");
        // clean up the shared registry for other tests
        let _ = registry().lock().unwrap().unsubscribe(sub);
    }
}
