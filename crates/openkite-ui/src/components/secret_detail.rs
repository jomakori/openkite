//! The Secret detail slide-over: masked key/value rows with per-value reveal,
//! copy-to-clipboard, and a gated bulk-reveal confirm.
//!
//! The helpers at the top are pure (no Dioxus) so the decode/reveal logic is
//! unit-testable without a runtime; the `#[component]` bodies below consume
//! them. The per-value reveal is the trust boundary: values are masked by
//! default, `MaskedSecret` owns the plaintext only for the component's
//! lifetime, and bulk reveal requires typing the secret's name.

use std::collections::HashMap;

use crate::secrets::MaskedSecret;

pub use openkite_api::secret::{
    bulk_reveal_predicate, decoded_value_for_key, row_id_for_secret, secret_keys,
    secret_kind_label, SecretObject,
};

use dioxus::prelude::*;

/// One key/value row: the masked-or-revealed value plus its action buttons.
/// The reveal/hide/copy buttons call up into `SecretDetail` — the row itself
/// never mutates the map (single-writer discipline).
#[component]
fn SecretValueRow(
    key_name: String,
    value: MaskedSecret,
    on_reveal: EventHandler<String>,
    on_hide: EventHandler<String>,
    on_copy: EventHandler<String>,
) -> Element {
    let display = value.display().to_string();
    let revealed = value.is_revealed();
    let key_display = key_name.clone();
    let key_for_reveal = key_name.clone();
    let key_for_hide = key_name.clone();
    rsx! {
        div { class: "kv-row",
            dt { "{key_display}" }
            dd {
                class: if revealed { "value-revealed" } else { "value-masked" },
                span { class: "value-mask", "{display}" }
                div { class: "value-actions",
                    button {
                        class: "btn btn-secondary reveal-btn",
                        style: if revealed { "display: none;"} else { "display: inline-flex;" },
                        onclick: move |_| on_reveal.call(key_for_reveal.clone()),
                        "Reveal"
                    }
                    button {
                        class: "btn btn-secondary hide-btn",
                        style: if revealed { "display: inline-flex;" } else { "display: none;" },
                        onclick: move |_| on_hide.call(key_for_hide.clone()),
                        "Hide"
                    }
                    button {
                        class: "btn btn-secondary copy-btn",
                        style: if revealed { "display: inline-flex;" } else { "display: none;" },
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

    #[cfg(not(target_arch = "wasm32"))]
    let mut esc_task = use_hook(|| CopyValue::new(None::<dioxus::core::Task>));

    use_effect(move || {
        let Some(secret) = crate::runtime::SELECTED_SECRET.read().clone() else {
            secrets_map.write().clear();
            return;
        };
        let mut map: HashMap<String, MaskedSecret> = HashMap::new();
        for key in secret_keys(&secret) {
            let plaintext = decoded_value_for_key(&secret, &key);
            map.insert(key, MaskedSecret::new(plaintext));
        }
        *secrets_map.write() = map;
    });

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

    #[cfg(not(target_arch = "wasm32"))]
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
    let name = secret.name.clone();
    let namespace = secret.namespace.clone().unwrap_or_else(|| "default".into());

    let keys: Vec<String> = secret_keys(&secret);

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
                    SecretValueRow {
                        key_name: key.clone(),
                        value: secrets_map.read().get(&key).cloned().unwrap_or_else(|| MaskedSecret::new("")),
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
