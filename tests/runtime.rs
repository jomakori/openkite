//! Integration tests for the shared runtime state module (`src/runtime.rs`).
//!
//! Covers the pure, headless surface: `CrudTarget` state machine helpers,
//! namespace toggle semantics, signal publish/read round-trips, and the
//! `OnceLock`-backed plugin state defaults. No kube client, no cluster.
//!
//! Dioxus 0.7 global signals are backed by the *runtime*, so every test body
//! runs inside a throwaway VirtualDom via `with_runtime`.
//!
//! The runtime statics are process-global, so each `#[test]` fn owns
//! exactly one global (or one disjoint set) and sequences its assertions
//! internally — safe under cargo's parallel test execution.

use openkite::runtime::CrudTarget;
use openkite::runtime::{
    clear_crud_target, context_name, current_route, js_plugins, open_delete_for, open_editor_for,
    open_new_for, open_scale_for, set_client, set_context, set_contexts, set_current_route,
    set_namespaces, set_prometheus, set_selected_namespaces, toggle_namespace, CRUD_TARGET,
    SELECTED_NAMESPACES,
};
// `.read()` / `.write()` on the global signals come from the dioxus
// prelude (ReadableExt/WritableExt + Deref on Global); glob-import it the
// way the lib does so the method resolution matches exactly.
use dioxus::prelude::*;

fn crud_target() -> Option<CrudTarget> {
    CRUD_TARGET.read().clone()
}

/// Run a closure with a Dioxus runtime installed on this thread.
fn with_runtime<O>(f: impl FnOnce() -> O) -> O {
    fn stub() -> Element {
        rsx! { div {} }
    }
    let vdom = dioxus::prelude::VirtualDom::new(stub);
    vdom.in_runtime(f)
}

/// Owns `SELECTED_NAMESPACES`: default initializer, add, remove, re-toggle.
#[test]
fn selected_namespaces_default_and_toggle_semantics() {
    with_runtime(|| {
        // Reset to the signal's own default, then verify toggle add/remove.
        set_selected_namespaces(vec!["default".into()]);
        assert_eq!(&*SELECTED_NAMESPACES.read(), &["default".to_string()]);

        set_selected_namespaces(vec!["a".into(), "b".into()]);
        toggle_namespace("c".into());
        assert_eq!(
            &*SELECTED_NAMESPACES.read(),
            &["a".to_string(), "b".to_string(), "c".to_string()]
        );
        toggle_namespace("a".into());
        assert_eq!(
            &*SELECTED_NAMESPACES.read(),
            &["b".to_string(), "c".to_string()]
        );
        // Re-adding an already-present namespace removes it (idempotent toggle).
        toggle_namespace("c".into());
        assert_eq!(&*SELECTED_NAMESPACES.read(), &["b".to_string()]);
    });
}
#[test]
fn crud_target_open_and_clear_state_machine() {
    with_runtime(|| {
        clear_crud_target();
        assert_eq!(crud_target(), None);

        open_new_for("Deployment".into());
        assert_eq!(
            crud_target(),
            Some(CrudTarget::New {
                kind: "Deployment".into()
            })
        );

        let doc = serde_json::json!({"apiVersion": "apps/v1", "kind": "Deployment"});
        open_editor_for("Deployment".into(), doc.clone());
        assert_eq!(
            crud_target(),
            Some(CrudTarget::Edit {
                kind: "Deployment".into(),
                doc,
            })
        );

        open_delete_for("Secret".into(), Some("prod".into()), "db".into());
        assert_eq!(
            crud_target(),
            Some(CrudTarget::Delete {
                kind: "Secret".into(),
                namespace: Some("prod".into()),
                name: "db".into(),
            })
        );

        open_delete_for("Namespace".into(), None, "staging".into());
        assert_eq!(
            crud_target(),
            Some(CrudTarget::Delete {
                kind: "Namespace".into(),
                namespace: None,
                name: "staging".into(),
            })
        );

        open_scale_for("Deployment".into(), Some("web".into()), "api".into(), 3);
        assert_eq!(
            crud_target(),
            Some(CrudTarget::Scale {
                kind: "Deployment".into(),
                namespace: Some("web".into()),
                name: "api".into(),
                current_replicas: 3,
            })
        );

        clear_crud_target();
        assert_eq!(crud_target(), None);
    });
}
#[test]
fn context_publish_read_round_trip() {
    with_runtime(|| {
        set_context(None);
        assert_eq!(context_name(), None);
        set_context(Some("prod".into()));
        assert_eq!(context_name(), Some("prod".into()));
        set_context(None);
        assert_eq!(context_name(), None);
    });
}
#[test]
fn client_publish_none_is_readable() {
    with_runtime(|| {
        set_client(None);
        assert!(openkite::runtime::client().is_none());
    });
}
#[test]
fn contexts_list_round_trip() {
    with_runtime(|| {
        set_contexts(vec!["prod".into(), "dev".into()]);
        assert_eq!(
            openkite::runtime::CONTEXTS.read().clone(),
            vec!["prod".to_string(), "dev".to_string()]
        );
        set_contexts(Vec::new());
        assert!(openkite::runtime::CONTEXTS.read().is_empty());
    });
}
#[test]
fn namespaces_and_prometheus_round_trip() {
    with_runtime(|| {
        set_namespaces(vec!["default".into(), "kube-system".into()]);
        assert_eq!(
            openkite::runtime::NAMESPACES.read().clone(),
            vec!["default".to_string(), "kube-system".to_string()]
        );
        set_namespaces(Vec::new());
        assert!(openkite::runtime::NAMESPACES.read().is_empty());

        set_prometheus(None);
        assert_eq!(*openkite::runtime::PROMETHEUS.read(), None);
        set_prometheus(Some("prometheus-k8s".into()));
        assert_eq!(
            *openkite::runtime::PROMETHEUS.read(),
            Some("prometheus-k8s".into())
        );
    });
}
#[test]
fn current_route_defaults_empty_and_round_trips() {
    with_runtime(|| {
        set_current_route("".into());
        assert_eq!(current_route(), "");
        set_current_route("/workloads".into());
        assert_eq!(current_route(), "/workloads");
        set_current_route("".into());
        assert_eq!(current_route(), "");
    });
}
#[test]
fn js_plugins_defaults_empty() {
    with_runtime(|| {
        assert!(js_plugins().is_empty());
    });
}
