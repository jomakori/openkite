//! The headless Dioxus runtime the reflector set needs.
//!
//! `openkite::state::live` keeps its snapshots in Dioxus `Signal`s, and a dioxus
//! 0.7 signal resolves its owner scope from the ambient runtime: creating one
//! goes `Signal::new_maybe_sync` → `generational_box::current_owner` →
//! `Runtime::current().current_owner()`, which needs both a current runtime and a
//! scope on its stack. A host that never renders has neither, so starting the
//! reflectors from one panics before the first watch opens. This module supplies
//! both: a `VirtualDom` with an empty root that is never rendered, and the root
//! scope the signals resolve against.
//!
//! Only *creating* a signal needs the runtime. The reflector tasks write their
//! snapshots from ordinary tokio workers, which works because a `SyncStorage`
//! signal is writable from any thread.

use dioxus::core::{ScopeId, VNode, VirtualDom};
use kube::Client;

/// Start the core reflectors inside the runtime that owns their signals.
///
/// Idempotent per kind, like [`openkite::state::live::start`]: it answers how
/// many reflectors were newly started.
pub fn start_reflectors(client: Client) -> usize {
    DOM.with(|dom| {
        dom.in_runtime(|| dom.in_scope(ScopeId::ROOT, || openkite::state::live::start(client)))
    })
}

thread_local! {
    /// The runtime that owns the reflectors' signals.
    ///
    /// Thread-local because a Dioxus runtime and everything it owns is
    /// single-threaded, and leaked because those signals outlive the call that
    /// creates them: dropping the dom would free them out from under the
    /// reflector tasks still publishing into them.
    static DOM: &'static VirtualDom = {
        let mut dom = VirtualDom::new(VNode::empty);
        // Build the empty tree once, so the root scope every signal resolves its
        // owner against exists before the first reflector starts.
        dom.rebuild_in_place();
        Box::leak(Box::new(dom))
    };
}
