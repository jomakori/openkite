//! The page the host renders.
//!
//! `App` is the single root the SSR pass and the wasm client both build, so
//! hydration matches by construction: the snapshot is the same on both sides
//! and the `data-node-hydration` ids the server writes line up with the ids
//! the client walks.
//!
//! The markup is composed from `openkite_ui` — the shared shell models and
//! the shared status pill, the same vocabulary the desktop chrome renders —
//! so both hosts paint the same console for the same route contract.
//!
//! Every gateway call is async — `use_resource` keeps it inside Dioxus, so
//! the SSR pass renders the initial snapshot synchronously (no resource has
//! resolved yet) and the client refreshes after hydration through
//! `POST /api/gateway`.

use dioxus::prelude::*;

use openkite_api::capability::{Capabilities, GatewayKind};
use openkite_ui::components::shell::{AppShell, NamespaceChip};
use openkite_ui::components::status_badge::{StatusKind, StatusPill};
use openkite_ui::plugin_api::RegistrationStore;
use openkite_ui::shell::{sidebar_model, status_bar_model, ShellState};

use crate::ssr::Snapshot;

/// The props the SSR pass and the wasm client both build their VirtualDom
/// from. The snapshot carries everything the page paints before the first
/// gateway call resolves.
#[derive(Props, Clone, PartialEq)]
pub struct AppProps {
    pub snapshot: Snapshot,
}

/// What one gateway op is asking the server for. The client sends this; the
/// host answers over HTTP at `POST /api/gateway`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum GatewayRequest {
    /// Re-fetch the snapshot the page paints.
    Snapshot,
}

/// The server-side gateway answer. Mirrors `Snapshot` so the client just
/// swaps state on it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GatewayResponse {
    pub snapshot: Snapshot,
}

/// The single page root: the shared shell (sidebar, top bar, status footer)
/// around the surfaces the snapshot can fill.
#[component]
pub fn App(props: AppProps) -> Element {
    let mut state = use_signal(|| props.snapshot.clone());
    let mut round = use_signal(|| 0u32);
    let mut last_error = use_signal(|| Option::<String>::None);

    let on_refresh = move |_| {
        spawn(async move {
            match crate::client::fetch_snapshot().await {
                Ok(Some(next)) => {
                    state.set(next);
                    round.set(round() + 1);
                    last_error.set(None);
                }
                Ok(None) => {
                    last_error.set(Some("no gateway available".to_string()));
                }
                Err(err) => {
                    last_error.set(Some(err));
                }
            }
        });
    };

    let snapshot = state();
    let capabilities = snapshot.capabilities;

    let shell = ShellState {
        cluster: snapshot.context.clone(),
        namespace: "default".into(),
        connected: snapshot.connected,
        prometheus: None,
    };
    let registrations = RegistrationStore::default();
    let sections = sidebar_model(&registrations);
    let status_entries = status_bar_model(&shell, &registrations, "");

    let gateway_label = match capabilities.gateway {
        GatewayKind::InProcess => "in-process",
        GatewayKind::ServerSide => "server-side",
    };
    let connection = if snapshot.connected {
        StatusKind::Running
    } else {
        StatusKind::Failed
    };

    let namespaces = vec![NamespaceChip {
        label: shell.cluster_label(),
        active: true,
        context: true,
    }];

    rsx! {
        AppShell {
            sections,
            // The page has no client router: entries are plain links, and
            // nothing is marked current until a route resolves.
            current_route: String::new(),
            namespaces,
            status: status_entries,
            "data-surface": "app",
            section { class: "panel", "data-surface": "overview",
                h2 { "Cluster" }
                dl { class: "kv-list",
                    div { class: "kv-row", dt { "Gateway" } dd { "{gateway_label}" } }
                    div { class: "kv-row",
                        dt { "Connection" }
                        dd { StatusPill { status: connection } }
                    }
                    div { class: "kv-row",
                        dt { "Context" }
                        dd { "{shell.cluster_label()}" }
                    }
                }
                button {
                    class: "btn btn-primary",
                    r#type: "button",
                    onclick: on_refresh,
                    "data-action": "refresh",
                    "Refresh"
                }
                if let Some(err) = last_error() {
                    p { class: "field-error", "data-error": "1", "gateway error: {err}" }
                }
            }
            section { class: "panel", "data-surface": "capabilities",
                h2 { "Capabilities" }
                dl { class: "kv-list",
                    for (label, enabled) in capability_rows(&capabilities) {
                        div { class: "kv-row",
                            dt { "{label}" }
                            dd { if enabled { "on" } else { "off" } }
                        }
                    }
                }
                p { class: "tagline", "data-round": "{round()}", "round {round()}" }
            }
            section { class: "panel", "data-surface": "secrets",
                h2 { "Secrets" }
                if snapshot.secrets.is_empty() {
                    p { class: "tagline", "data-empty": "secrets", "No secrets in the gateway's scope" }
                } else {
                    dl { class: "kv-list",
                        for secret in snapshot.secrets.iter() {
                            div { class: "kv-row",
                                dt { class: "resource-name", "{secret.name}" }
                                dd { class: "namespace", "{secret.namespace}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The capability flags the host reports, as `(label, enabled)` rows.
fn capability_rows(capabilities: &Capabilities) -> [(&'static str, bool); 3] {
    [
        ("Plugins", capabilities.plugins),
        ("Terminal", capabilities.terminal),
        ("Exec", capabilities.exec),
    ]
}
