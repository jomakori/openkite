//! The page the host renders.
//!
//! `App` is the single root the SSR pass and the wasm client both build, so
//! hydration matches by construction: the snapshot is the same on both sides
//! and the `data-node-hydration` ids the server writes line up with the ids
//! the client walks.
//!
//! Every gateway call is async — `use_resource` keeps it inside Dioxus, so
//! the SSR pass renders the initial snapshot synchronously (no resource has
//! resolved yet) and the client refreshes after hydration through
//! `POST /api/gateway`.

use dioxus::prelude::*;

use openkite_api::capability::{Capabilities, GatewayKind};

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

/// The single page root.
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
    let gateway_label = match snapshot.capabilities.gateway {
        GatewayKind::InProcess => "in-process",
        GatewayKind::ServerSide => "server-side",
    };
    let connection = if snapshot.connected {
        "connected"
    } else {
        "disconnected"
    };
    let context = snapshot.context.clone().unwrap_or_else(|| "—".into());

    rsx! {
        div { class: "app-shell", "data-surface": "app",
            header { class: "topbar",
                h1 { "OpenKite" }
                span { class: "tagline", "Browser host — SSR + hydration" }
            }
            section { class: "surface", "data-surface": "overview",
                h2 { "Cluster overview" }
                dl { class: "kv",
                    div { dt { "Gateway" } dd { "{gateway_label}" } }
                    div { dt { "Connection" } dd { class: if snapshot.connected { "status-ok" } else { "status-warn" }, "{connection}" } }
                    div { dt { "Context" } dd { "{context}" } }
                    div { dt { "Plugins" } dd { if snapshot.capabilities.plugins { "yes" } else { "no" } } }
                    div { dt { "Terminal" } dd { if snapshot.capabilities.terminal { "yes" } else { "no" } } }
                    div { dt { "Exec" } dd { if snapshot.capabilities.exec { "yes" } else { "no" } } }
                }
                button {
                    class: "btn btn-primary",
                    r#type: "button",
                    onclick: on_refresh,
                    "data-action": "refresh",
                    "Refresh"
                }
                if let Some(err) = last_error() {
                    p { class: "error", "data-error": "1", "gateway error: {err}" }
                }
            }
            section { class: "surface", "data-surface": "capabilities",
                h2 { "Capabilities" }
                p { "data-round": "{round()}",
                    "round {round()} — {capabilities_summary(&snapshot.capabilities)}"
                }
            }
        }
    }
}

fn capabilities_summary(c: &Capabilities) -> String {
    format!(
        "gateway={:?}, plugins={}, terminal={}, exec={}",
        c.gateway, c.plugins, c.terminal, c.exec
    )
}
