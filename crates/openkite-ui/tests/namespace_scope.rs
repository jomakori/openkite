//! The host's read path for the console's namespace selection.
//!
//! The host's snapshot and push paths run on plain tokio tasks, outside the
//! Dioxus runtime, so [`namespace_selection_scope`] must be readable there.

use dioxus::prelude::*;
use openkite_ui::runtime::{
    clear_namespace_selection, namespace_selection_scope, set_namespace_selection, toggle_namespace,
};

fn ns(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

/// Run `f` inside a throwaway Dioxus runtime so the global signals are usable.
fn with_runtime<O>(f: impl FnOnce() -> O) -> O {
    fn stub() -> Element {
        rsx! { div {} }
    }
    let vdom = VirtualDom::new(stub);
    vdom.in_runtime(f)
}

#[test]
fn the_host_reads_the_selection_without_a_dioxus_runtime() {
    assert!(
        namespace_selection_scope().is_empty(),
        "no selection ⇒ all namespaces"
    );

    with_runtime(|| {
        set_namespace_selection(ns(&["argocd"]));
        toggle_namespace("default".to_string());
    });

    assert_eq!(
        namespace_selection_scope(),
        ns(&["argocd", "default"]),
        "the selection mirrors out of the runtime for the host to read"
    );

    with_runtime(clear_namespace_selection);

    assert!(
        namespace_selection_scope().is_empty(),
        "reset returns to all namespaces"
    );
}
