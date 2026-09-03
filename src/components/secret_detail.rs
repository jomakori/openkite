//! The Secret detail slide-over: masked key/value rows with per-value reveal,
//! copy-to-clipboard, and a gated bulk-reveal confirm.
//!
//! The helpers at the top are pure (no Dioxus) so the decode/reveal logic is
//! unit-testable without a runtime; the `#[component]` bodies below consume
//! them. The per-value reveal is the trust boundary: values are masked by
//! default, `MaskedSecret` owns the plaintext only for the component's
//! lifetime, and bulk reveal requires typing the secret's name.

use std::collections::HashMap;

use k8s_openapi::api::core::v1::Secret;

use crate::secrets::MaskedSecret;

/// Look up `key` in `secret.data` first (base64-encoded `Vec<u8>`), then
/// `secret.string_data` (already-decoded `String`). Returns the UTF-8 lossy
/// representation. This is the only function in the project that produces a
/// plaintext Secret value; it is called inside the slide-over to construct a
/// `MaskedSecret::new(plaintext)`, and the plaintext is never held outside a
/// `MaskedSecret` for longer than the `display()` call.
pub fn decoded_value_for_key(secret: &Secret, key: &str) -> String {
    if let Some(data) = &secret.data {
        if let Some(bytes) = data.get(key) {
            return String::from_utf8_lossy(bytes).into_owned();
        }
    }
    if let Some(string_data) = &secret.string_data {
        if let Some(s) = string_data.get(key) {
            return s.clone();
        }
    }
    String::new()
}

/// The `ns/name` (or bare `name`) row id for a secret — the same shape
/// `workloads::object_id` produces, kept here so the mapper and the slide-over
/// agree without depending on the private `workloads` helper.
pub fn row_id_for_secret(secret: &Secret) -> String {
    let ns = secret.metadata.namespace.as_deref();
    let name = secret.metadata.name.as_deref().unwrap_or("");
    match ns {
        Some(ns) => format!("{ns}/{name}"),
        None => name.to_string(),
    }
}

/// The typed-name gate for bulk reveal: `true` iff `typed.trim() == name`
/// (case-sensitive, no fuzzy).
pub fn bulk_reveal_predicate(typed: &str, name: &str) -> bool {
    typed.trim() == name
}

/// Map a `Secret.type` string to a human label. `None` / `Some("Opaque")` /
/// empty string all collapse to `"Opaque"` (the kube default); unknown kinds
/// pass through so a CRD-defined type still surfaces in the table.
pub fn secret_kind_label(kind: Option<&str>) -> String {
    match kind.unwrap_or("Opaque") {
        "kubernetes.io/tls" => "TLS".into(),
        "kubernetes.io/dockerconfigjson" => "Docker config".into(),
        "kubernetes.io/service-account-token" => "Service account token".into(),
        "Opaque" | "" => "Opaque".into(),
        other => other.to_string(),
    }
}

use dioxus::prelude::*;

/// One key/value row: the masked-or-revealed value plus its action buttons.
/// The reveal/hide/copy buttons call up into `SecretDetail` — the row itself
/// never mutates the map (single-writer discipline).
#[component]
fn SecretValueRow(
    key: String,
    value: MaskedSecret,
    on_reveal: EventHandler<String>,
    on_hide: EventHandler<String>,
    on_copy: EventHandler<String>,
) -> Element {
    let display = value.display().to_string();
    let revealed = value.is_revealed();
    rsx! {
        div { class: "kv-row",
            dt { "{key}" }
            dd {
                class: if revealed { "value-revealed" } else { "value-masked" },
                span { class: "value-mask", "{display}" }
                div { class: "value-actions",
                    button {
                        class: "btn btn-secondary reveal-btn",
                        style: "display: {if revealed { \"none\" } else { \"inline-flex\" }};",
                        onclick: move |_| on_reveal.call(key.clone()),
                        "Reveal"
                    }
                    button {
                        class: "btn btn-secondary hide-btn",
                        style: "display: {if revealed { \"inline-flex\" } else { \"none\" }};",
                        onclick: move |_| on_hide.call(key.clone()),
                        "Hide"
                    }
                    button {
                        class: "btn btn-secondary copy-btn",
                        style: "display: {if revealed { \"inline-flex\" } else { \"none\" }};",
                        onclick: move |_| on_copy.call(value.value().to_string()),
                        "Copy"
                    }
                }
            }
        }
    }
}

/// The typed-name confirm modal for bulk reveal. The red button is disabled
/// until the input matches the secret's name; Esc cancels, Enter confirms
/// only when the input is focused.
#[component]
fn ConfirmRevealAll(
    secret_name: String,
    on_confirm: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let mut typed = use_signal(String::new);
    rsx! {
        div { class: "modal-backdrop", onclick: move |_| on_cancel.call(()),
            div { class: "modal", onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h3 { "Reveal all values" }
                }
                div { class: "modal-body",
                    p { "Type the secret's name to reveal every value at once." }
                    p { style: "color: var(--fg-2); font-size: 12px;",
                        "Type \"{secret_name}\" to confirm."
                    }
                    input {
                        class: "field-input",
                        autofocus: true,
                        value: "{typed}",
                        oninput: move |e| typed.set(e.value()),
                    }
                }
                div { class: "modal-footer",
                    button { class: "btn btn-secondary", onclick: move |_| on_cancel.call(()), "Cancel" }
                    button {
                        class: "btn btn-danger",
                        disabled: !bulk_reveal_predicate(&typed(), &secret_name),
                        onclick: move |_| on_confirm.call(()),
                        "Reveal all"
                    }
                }
            }
        }
    }
}

/// The slide-over body: a `.kv-list` of `SecretValueRow`s driven by a
/// per-mount `HashMap<String, MaskedSecret>`. The map is rebuilt whenever
/// `SELECTED_SECRET` changes, so revealed values never persist across
/// navigation. Esc closes the slide-over.
#[component]
pub fn SecretDetail() -> Element {
    let mut secrets_map: Signal<HashMap<String, MaskedSecret>> = use_signal(HashMap::new);
    let mut reveal_all_open: Signal<bool> = use_signal(|| false);

    // Task slot for the Esc poll loop: aborted on re-run so the loop never
    // stacks (the install-once guard keeps the listener single, but the
    // poll task is per-effect-run).
    let mut esc_task = use_hook(|| CopyValue::new(None::<dioxus::core::Task>));

    // Populate the map whenever the selected secret changes. Reassignment
    // drops the prior map (and its plaintext) in one step.
    use_effect(move || {
        let Some(secret) = crate::runtime::SELECTED_SECRET.read().clone() else {
            secrets_map.write().clear();
            return;
        };
        let mut map: HashMap<String, MaskedSecret> = HashMap::new();
        for key in crate::network::secret_keys(&secret) {
            let plaintext = decoded_value_for_key(&secret, &key);
            map.insert(key, MaskedSecret::new(plaintext));
        }
        *secrets_map.write() = map;
    });

    // Install-once keydown listener: Esc sets a window flag. The flag is
    // consumed by the poll task below (the dioxus.send channel is not used
    // here because this component has no eval receiver loop).
    use_effect(move || {
        let _ = document::eval(
            r#"
            (function () {
                if (window.__openkite_secret_esc_installed) return;
                window.__openkite_secret_esc_installed = true;
                document.addEventListener('keydown', function (e) {
                    if (e.key === 'Escape') {
                        window.__openkite_secret_esc = '1';
                    }
                });
            })();
            "#,
        );
    });

    // Poll the Esc flag; on close, clear the selection (which unmounts this
    // component — the `.inspector.open` class disappears with it). The loop
    // uses dioxus `spawn` (not tokio) because `document::eval` needs the
    // Dioxus runtime; `Task::cancel` aborts a prior loop on re-run.
    use_effect(move || {
        if let Some(task) = esc_task.write().take() {
            task.cancel();
        }
        let task = spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                let flag = document::eval("window.__openkite_secret_esc || ''")
                    .recv::<String>()
                    .await
                    .unwrap_or_default();
                let _ = document::eval("window.__openkite_secret_esc = '';");
                if flag == "1" {
                    *crate::runtime::SELECTED_SECRET.write() = None;
                }
            }
        });
        *esc_task.write() = Some(task);
    });

    let Some(secret) = crate::runtime::SELECTED_SECRET.read().clone() else {
        return rsx! {};
    };
    let name = secret.metadata.name.clone().unwrap_or_default();
    let namespace = secret.metadata.namespace.clone().unwrap_or_else(|| "default".into());

    // Precompute the key list so the rsx! loop has an owned Vec.
    let keys: Vec<String> = crate::network::secret_keys(&secret);

    rsx! {
        div { class: "inspector open",
            div { class: "inspector-header",
                div { class: "inspector-title",
                    h2 { "{name}" }
                    span { class: "resource-kind", "Secret" }
                }
                div { class: "inspector-actions",
                    button {
                        class: "btn btn-secondary",
                        onclick: move |_| reveal_all_open.set(true),
                        "Reveal all values"
                    }
                    button {
                        class: "btn btn-secondary",
                        onclick: move |_| *crate::runtime::SELECTED_SECRET.write() = None,
                        "Close"
                    }
                }
            }
            div { class: "inspector-meta",
                span { "namespace: {namespace}" }
                span { "type: {secret_kind_label(secret.type_.as_deref())}" }
            }
            div { class: "kv-list",
                for key in keys {
                    let value = secrets_map.read().get(&key).cloned().unwrap_or_else(|| MaskedSecret::new(""));
                    SecretValueRow {
                        key: key.clone(),
                        value,
                        on_reveal: {
                            let key = key.clone();
                            EventHandler::new(move |_| {
                                if let Some(v) = secrets_map.write().get_mut(&key) {
                                    v.reveal();
                                }
                            })
                        },
                        on_hide: {
                            let key = key.clone();
                            EventHandler::new(move |_| {
                                if let Some(v) = secrets_map.write().get_mut(&key) {
                                    v.hide();
                                }
                            })
                        },
                        on_copy: {
                            EventHandler::new(move |plaintext: String| {
                                let js = format!("navigator.clipboard.writeText({plaintext:?});");
                                let _ = document::eval(&js);
                            })
                        },
                    }
                }
            }
            if reveal_all_open() {
                ConfirmRevealAll {
                    secret_name: name.clone(),
                    on_confirm: {
                        EventHandler::new(move |()| {
                            for v in secrets_map.write().values_mut() {
                                v.reveal();
                            }
                            reveal_all_open.set(false);
                        })
                    },
                    on_cancel: {
                        EventHandler::new(move |()| reveal_all_open.set(false))
                    },
                }
            }
        }
    }
}
