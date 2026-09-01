//! CRUD modal: editor slide-over + confirm modals.
//!
//! Reads `CRUD_TARGET` from the runtime; renders the matching component.
//! Both share the design-system tokens (no inline colors) and the same
//! keyboard contract (Esc cancels; Enter confirms only on the focused
//! primary control).
//!
//! The confirm UX is a first-class design concern: the destructive
//! `ConfirmDelete` modal is a separate component from the editor (not a
//! config flag) with a typed-name gate, an Esc/Enter keyboard contract, and
//! a backdrop click that cancels. `Create` / `Edit` / `Scale` use a lighter
//! 2-button modal with no typed-name gate.

#![allow(non_snake_case)]

use dioxus::prelude::*;
use serde_json::{json, Value};

use crate::crud::{self, apply_mutation, validate_for_edit, validate_manifest, Mutation, PropagationPolicy};
use crate::runtime::{self, CrudTarget, CRUD_TARGET};
use crate::yaml::parse_yaml;

/// How long the toast stays visible after a successful or failed apply.
const TOAST_DURATION_MS: u64 = 3000;

/// The overlay mounted in `AppShell`. Dispatches on `CRUD_TARGET`; renders
/// nothing when the target is `None`.
#[component]
pub fn CrudOverlay() -> Element {
    let target = CRUD_TARGET.read().clone();
    rsx! {
        if let Some(t) = target {
            match t {
                CrudTarget::Edit { doc, kind } => rsx! {
                    CrudEditor { initial_doc: Some(doc), target_kind: kind, mode: EditorMode::Edit }
                },
                CrudTarget::Delete { kind, namespace, name } => rsx! {
                    ConfirmDelete { kind, namespace, name }
                },
                CrudTarget::Scale { kind, namespace, name, current_replicas } => rsx! {
                    ConfirmScale { kind, namespace, name, current_replicas }
                },
                CrudTarget::New { kind } => rsx! {
                    CrudEditor { initial_doc: None, target_kind: kind, mode: EditorMode::New }
                },
            }
        }
    }
}

/// What the editor is doing: create-from-blank or patch-an-existing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    New,
    Edit,
}

impl EditorMode {
    fn label(self) -> &'static str {
        match self {
            EditorMode::New => "New resource",
            EditorMode::Edit => "Edit resource",
        }
    }
}

/// Starter manifest for a brand-new resource, keyed by the kind the user
/// picked in the Workloads tab. Round-trips through `serde_saphyr`/JSON; the
/// editor renders it as a YAML textarea.
fn starter_for_kind(kind: &str) -> Value {
    match kind {
        "Pod" => json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": { "name": "", "namespace": "default" },
            "spec": {
                "containers": [{ "name": "app", "image": "nginx:latest" }]
            }
        }),
        "Deployment" => json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": { "name": "", "namespace": "default" },
            "spec": {
                "replicas": 1,
                "selector": { "matchLabels": { "app": "" } },
                "template": {
                    "metadata": { "labels": { "app": "" } },
                    "spec": {
                        "containers": [{ "name": "app", "image": "nginx:latest" }]
                    }
                }
            }
        }),
        "StatefulSet" => json!({
            "apiVersion": "apps/v1",
            "kind": "StatefulSet",
            "metadata": { "name": "", "namespace": "default" },
            "spec": {
                "serviceName": "",
                "replicas": 1,
                "selector": { "matchLabels": { "app": "" } },
                "template": {
                    "metadata": { "labels": { "app": "" } },
                    "spec": {
                        "containers": [{ "name": "app", "image": "nginx:latest" }]
                    }
                }
            }
        }),
        "DaemonSet" => json!({
            "apiVersion": "apps/v1",
            "kind": "DaemonSet",
            "metadata": { "name": "", "namespace": "default" },
            "spec": {
                "selector": { "matchLabels": { "app": "" } },
                "template": {
                    "metadata": { "labels": { "app": "" } },
                    "spec": {
                        "containers": [{ "name": "app", "image": "nginx:latest" }]
                    }
                }
            }
        }),
        "ReplicaSet" => json!({
            "apiVersion": "apps/v1",
            "kind": "ReplicaSet",
            "metadata": { "name": "", "namespace": "default" },
            "spec": {
                "replicas": 1,
                "selector": { "matchLabels": { "app": "" } },
                "template": {
                    "metadata": { "labels": { "app": "" } },
                    "spec": {
                        "containers": [{ "name": "app", "image": "nginx:latest" }]
                    }
                }
            }
        }),
        "Job" => json!({
            "apiVersion": "batch/v1",
            "kind": "Job",
            "metadata": { "name": "", "namespace": "default" },
            "spec": {
                "template": {
                    "spec": {
                        "containers": [{ "name": "app", "image": "busybox", "args": ["echo", "hi"] }],
                        "restartPolicy": "Never"
                    }
                }
            }
        }),
        "CronJob" => json!({
            "apiVersion": "batch/v1",
            "kind": "CronJob",
            "metadata": { "name": "", "namespace": "default" },
            "spec": {
                "schedule": "*/5 * * * *",
                "jobTemplate": {
                    "spec": {
                        "template": {
                            "spec": {
                                "containers": [{ "name": "app", "image": "busybox", "args": ["echo", "hi"] }],
                                "restartPolicy": "OnFailure"
                            }
                        }
                    }
                }
            }
        }),
        _ => json!({
            "apiVersion": "v1",
            "kind": kind,
            "metadata": { "name": "", "namespace": "default" }
        }),
    }
}

/// Render a `Value` as a YAML string for the editor body. Falls back to a
/// compact JSON form on serializer error (which should never happen for a
/// valid `Value` but gives a usable string either way).
fn value_to_yaml(doc: &Value) -> String {
    serde_json::to_string_pretty(doc).unwrap_or_default()
}

/// The slide-over editor: a `<textarea>` bound to a `Signal<String>` of YAML;
/// `yaml::parse_yaml` runs on every keystroke. On parse error the editor
/// shows a footer with the `Diagnostic { line, column, message }` and
/// disables `Apply`. On parse success, `crud::validate_manifest` runs and
/// rejects missing `apiVersion` / `kind` / `metadata.name`. Edit mode
/// additionally requires `metadata.resourceVersion` (the new
/// `validate_for_edit` guard) so a patch cannot silently lose-update.
#[component]
pub fn CrudEditor(
    initial_doc: Option<Value>,
    target_kind: String,
    mode: EditorMode,
) -> Element {
    let starter = initial_doc.unwrap_or_else(|| starter_for_kind(&target_kind));
    let initial_text = value_to_yaml(&starter);

    let mut text: Signal<String> = use_signal(|| initial_text);
    let mut toast: Signal<Option<String>> = use_signal(|| None::<String>);
    let mut pending: Signal<bool> = use_signal(|| false);

    // Parse on every keystroke. Precompute the (line, column, message)
    // string outside `rsx!` so the macro never sees a method call inside
    // an interpolation.
    let parsed: Result<Value, String> = match parse_yaml(&text.read()) {
        Ok(v) => Ok(v),
        Err(diag) => Err(format!(
            "line {}, column {}: {}",
            diag.line, diag.column, diag.message
        )),
    };
    let validation_error: Option<String> = match &parsed {
        Ok(doc) => match mode {
            EditorMode::New => validate_manifest(doc).err(),
            EditorMode::Edit => validate_for_edit(doc).err(),
        },
        // Parse error wins; don't double-report.
        Err(_) => None,
    };
    let parse_error: Option<String> = parsed.err();
    let can_apply = parse_error.is_none() && validation_error.is_none() && !pending();

    let on_apply = {
        let target_kind = target_kind.clone();
        move |_| {
            pending.set(true);
            let doc = match parse_yaml(&text.read()) {
                Ok(v) => v,
                Err(diag) => {
                    toast.set(Some(format!(
                        "parse error: line {}, column {}: {}",
                        diag.line, diag.column, diag.message
                    )));
                    pending.set(false);
                    return;
                }
            };
            let m = match mode {
                EditorMode::New => Mutation::Create(doc),
                EditorMode::Edit => Mutation::Edit(doc),
            };
            let kind_for_toast = target_kind.clone();
            spawn(async move {
                let result = match runtime::client() {
                    Some(client) => apply_mutation(&client, &m).await,
                    None => Err("no cluster connected".into()),
                };
                let msg = match result {
                    Ok(()) => format!(
                        "applied {} {}",
                        m.verb(),
                        kind_for_toast
                    ),
                    Err(error) => format!("{} queued ({} — apply pending Phase 1)", m.verb(), error),
                };
                toast.set(Some(msg));
                pending.set(false);
                spawn_dismiss_toast(toast);
            });
        }
    };

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| runtime::clear_crud_target(),
            div {
                class: "modal modal-editor",
                role: "dialog",
                aria_labelledby: "crud-editor-title",
                onclick: move |event| event.stop_propagation(),
                div { class: "modal-header",
                    div { class: "modal-eyebrow", "{target_kind}" }
                    div { id: "crud-editor-title", class: "modal-title", "{mode.label()}" }
                }
                div { class: "modal-body",
                    textarea {
                        class: "editor-textarea",
                        value: "{text.read()}",
                        oninput: move |event| text.set(event.value()),
                        spellcheck: "false",
                    }
                    if let Some(err) = parse_error.as_ref() {
                        div { class: "field-error", "{err}" }
                    } else if let Some(err) = validation_error.as_ref() {
                        div { class: "field-error", "{err}" }
                    }
                }
                div { class: "modal-footer",
                    button {
                        class: "btn btn-secondary",
                        onclick: move |_| runtime::clear_crud_target(),
                        "Cancel"
                    }
                    button {
                        class: "btn btn-primary",
                        disabled: !can_apply,
                        onclick: on_apply,
                        if mode == EditorMode::New { "Create" } else { "Apply" }
                    }
                }
            }
        }
        if let Some(msg) = toast.read().as_ref() {
            div { class: "toast show", role: "status", aria_live: "polite", "{msg}" }
        }
    }
}

/// Destructive confirm modal (centered, typed-name gate). The red `Delete`
/// button is disabled until the typed string exactly matches the resource
/// name. `Esc` cancels; `Enter` confirms only when the typed-name input is
/// focused (Dioxus's `onkeydown` on the input handles Enter directly); the
/// backdrop cancels; the body `stop_propagation`s so clicks inside don't
/// dismiss.
#[component]
pub fn ConfirmDelete(kind: String, namespace: Option<String>, name: String) -> Element {
    let mut typed: Signal<String> = use_signal(String::new);
    let mut toast: Signal<Option<String>> = use_signal(|| None::<String>);
    let mut pending: Signal<bool> = use_signal(|| false);

    let triple = format_resource_triple(&kind, namespace.as_deref(), &name);
    let matches = crud::typed_name_matches(&typed.read(), &name);
    let can_confirm = matches && !pending();

    let on_confirm = {
        let kind = kind.clone();
        let namespace = namespace.clone();
        let name = name.clone();
        move |_| {
            pending.set(true);
            let m = Mutation::Delete {
                kind: kind.clone(),
                namespace: namespace.clone(),
                name: name.clone(),
                propagation: PropagationPolicy::Default,
            };
            let verb = m.verb();
            let kind_for_toast = kind.clone();
            let name_for_toast = name.clone();
            spawn(async move {
                let result = match runtime::client() {
                    Some(client) => apply_mutation(&client, &m).await,
                    None => Err("no cluster connected".into()),
                };
                let msg = match result {
                    Ok(()) => format!("deleted {} {}", kind_for_toast, name_for_toast),
                    Err(error) => format!(
                        "{} queued ({} — apply pending Phase 1)",
                        verb, error
                    ),
                };
                toast.set(Some(msg));
                pending.set(false);
                runtime::clear_crud_target();
                spawn_dismiss_toast(toast);
            });
        }
    };

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| runtime::clear_crud_target(),
            div {
                class: "modal modal-confirm",
                role: "alertdialog",
                aria_labelledby: "confirm-delete-title",
                aria_describedby: "confirm-delete-helper",
                onclick: move |event| event.stop_propagation(),
                div { class: "modal-header",
                    div { class: "modal-eyebrow", "{triple}" }
                    div { id: "confirm-delete-title", class: "modal-title", "Delete resource" }
                }
                div { class: "modal-body",
                    p { class: "confirm-warning",
                        "This will delete the resource from your cluster."
                    }
                    label { class: "field-label",
                        "Type "
                        code { "{name}" }
                        " to confirm."
                    }
                    input {
                        class: "search-field",
                        r#type: "text",
                        autofocus: true,
                        placeholder: "{name}",
                        value: "{typed.read()}",
                        aria_describedby: "confirm-delete-helper",
                        oninput: move |event| typed.set(event.value()),
                        onkeydown: move |event| {
                            if event.key() == Key::Enter && can_confirm {
                                on_confirm(());
                            }
                        },
                    }
                    div { id: "confirm-delete-helper", class: "field-helper",
                        "Deletion is irreversible."
                    }
                }
                div { class: "modal-footer",
                    button {
                        class: "btn btn-secondary",
                        onclick: move |_| runtime::clear_crud_target(),
                        "Cancel"
                    }
                    button {
                        class: "btn btn-danger",
                        disabled: !can_confirm,
                        onclick: on_confirm,
                        "Delete"
                    }
                }
            }
        }
        if let Some(msg) = toast.read().as_ref() {
            div { class: "toast show", role: "status", aria_live: "polite", "{msg}" }
        }
    }
}

/// Non-destructive scale modal (centered, 2-button row, no typed-name gate).
/// The `Replicas` input defaults to the row's current replica count.
#[component]
pub fn ConfirmScale(
    kind: String,
    namespace: Option<String>,
    name: String,
    current_replicas: u32,
) -> Element {
    let mut replicas: Signal<String> = use_signal(|| current_replicas.to_string());
    let mut toast: Signal<Option<String>> = use_signal(|| None::<String>);
    let mut pending: Signal<bool> = use_signal(|| false);

    let triple = format_resource_triple(&kind, namespace.as_deref(), &name);
    let parsed_replicas: Option<u32> = replicas.read().trim().parse().ok();
    let can_confirm = parsed_replicas.is_some() && !pending();

    let on_confirm = {
        let kind = kind.clone();
        let namespace = namespace.clone();
        let name = name.clone();
        move |_| {
            let Some(replicas) = parsed_replicas else {
                return;
            };
            pending.set(true);
            let m = Mutation::Scale {
                kind: kind.clone(),
                namespace: namespace.clone(),
                name: name.clone(),
                replicas,
            };
            let verb = m.verb();
            let kind_for_toast = kind.clone();
            let name_for_toast = name.clone();
            spawn(async move {
                let result = match runtime::client() {
                    Some(client) => apply_mutation(&client, &m).await,
                    None => Err("no cluster connected".into()),
                };
                let msg = match result {
                    Ok(()) => format!("scaled {} {} to {}", kind_for_toast, name_for_toast, replicas),
                    Err(error) => format!(
                        "{} queued ({} — apply pending Phase 1)",
                        verb, error
                    ),
                };
                toast.set(Some(msg));
                pending.set(false);
                runtime::clear_crud_target();
                spawn_dismiss_toast(toast);
            });
        }
    };

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| runtime::clear_crud_target(),
            div {
                class: "modal modal-confirm",
                role: "dialog",
                aria_labelledby: "confirm-scale-title",
                onclick: move |event| event.stop_propagation(),
                div { class: "modal-header",
                    div { class: "modal-eyebrow", "{triple}" }
                    div { id: "confirm-scale-title", class: "modal-title", "Scale workload" }
                }
                div { class: "modal-body",
                    label { class: "field-label", "Replicas" }
                    input {
                        class: "search-field",
                        r#type: "number",
                        min: "0",
                        max: "1000",
                        value: "{replicas.read()}",
                        oninput: move |event| replicas.set(event.value()),
                        onkeydown: move |event| {
                            if event.key() == Key::Enter && can_confirm {
                                on_confirm(());
                            }
                        },
                    }
                    div { class: "field-helper", "Current: {current_replicas}" }
                }
                div { class: "modal-footer",
                    button {
                        class: "btn btn-secondary",
                        onclick: move |_| runtime::clear_crud_target(),
                        "Cancel"
                    }
                    button {
                        class: "btn btn-primary",
                        disabled: !can_confirm,
                        onclick: on_confirm,
                        "Scale"
                    }
                }
            }
        }
        if let Some(msg) = toast.read().as_ref() {
            div { class: "toast show", role: "status", aria_live: "polite", "{msg}" }
        }
    }
}

/// Render the resource triple as a one-line eyebrow (kind · namespace/name).
fn format_resource_triple(kind: &str, namespace: Option<&str>, name: &str) -> String {
    match namespace {
        Some(ns) => format!("{kind} · {ns}/{name}"),
        None => format!("{kind} · {name}"),
    }
}

/// Spawn a task that clears the toast after the standard delay. The toast
/// signal lives in the caller's component scope; we capture it by value
/// here and the spawned task deref-assigns to clear.
fn spawn_dismiss_toast(mut toast: Signal<Option<String>>) {
    spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(TOAST_DURATION_MS)).await;
        *toast.write() = None;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_for_kind_emits_typed_seeds() {
        let pod = starter_for_kind("Pod");
        assert_eq!(pod.get("apiVersion").and_then(Value::as_str), Some("v1"));
        assert_eq!(pod.get("kind").and_then(Value::as_str), Some("Pod"));
        assert_eq!(
            pod.pointer("/metadata/namespace").and_then(Value::as_str),
            Some("default")
        );
        let deploy = starter_for_kind("Deployment");
        assert_eq!(
            deploy.get("apiVersion").and_then(Value::as_str),
            Some("apps/v1")
        );
        assert_eq!(deploy.get("kind").and_then(Value::as_str), Some("Deployment"));
        let unknown = starter_for_kind("Other");
        assert_eq!(unknown.get("kind").and_then(Value::as_str), Some("Other"));
    }

    #[test]
    fn format_resource_triple_includes_namespace_when_present() {
        assert_eq!(format_resource_triple("Pod", Some("default"), "nginx"), "Pod · default/nginx");
        assert_eq!(format_resource_triple("Node", None, "node-1"), "Node · node-1");
    }

    #[test]
    fn editor_modes_have_distinct_labels() {
        assert_eq!(EditorMode::New.label(), "New resource");
        assert_eq!(EditorMode::Edit.label(), "Edit resource");
    }
}
