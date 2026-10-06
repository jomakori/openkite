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
use openkite_ui::components::namespace_bar::selection_matches;
use openkite_ui::components::palette::{CommandPalette, PaletteHost, PaletteKeybind};
use openkite_ui::components::resource_pane::ResourceDetail;
use openkite_ui::components::resource_table::{ResourceTable, TableStatus};
use openkite_ui::components::route_views::RouteView;
use openkite_ui::components::shell::{AppShell, ClusterInfo, ShellIcon, TopBarAction};
use openkite_ui::components::status_badge::{StatusKind, StatusPill};
use openkite_ui::components::switcher::{ClusterSwitcher, SwitcherKeybind};
use openkite_ui::plugin_api::RegistrationStore;
use openkite_ui::runtime::{push_mode, set_push_mode, PushMode, NAMESPACE_SELECTION};
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
    // The route chrome's spinner: true only while a refetch is in flight, so
    // the SSR markup and the client's first render agree (both start false).
    let mut pending = use_signal(|| false);

    // The route the host resolved, read once so a refresh cannot move the page.
    let route = props.snapshot.route.clone();

    // The selection is the address (OKT-175): the host read it out of the query
    // string it served, and it rides the snapshot so both this SSR pass and the
    // hydrating client open the resource detail pane at the same stop for the
    // same resource. `use_hook` runs once per mount, so a later snapshot
    // refresh cannot clobber a selection the user has since made.
    let restored = props.snapshot.selection.clone();
    use_hook(move || {
        if let Some(identity) = restored.clone() {
            openkite_ui::runtime::select_resource(ResourceDetail::from_ref(identity));
        }
    });

    // One refetch, two callers: the top bar's refresh action and the panel's
    // button. The closure only captures signals, so both may hold a copy.
    let refetch = move || {
        spawn(async move {
            pending.set(true);
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
            pending.set(false);
        });
    };
    let on_refresh = move |_: ()| refetch();
    let on_refresh_click = move |_: Event<MouseData>| refetch();

    // This host has no push transport, so the page polls for fresh state.
    // The 10 s interval is half of the degraded bound: an update lands within
    // one interval plus a round-trip and a render.
    use_hook(|| set_push_mode(PushMode::Polling));
    use_future(move || async move {
        loop {
            poll_interval().await;
            refetch();
        }
    });

    let snapshot = state();
    let capabilities = snapshot.capabilities;

    // No namespace inventory here: the secrets' namespaces and the pods'.
    let namespaces: Vec<String> = {
        let mut names: Vec<String> = snapshot
            .secrets
            .iter()
            .map(|secret| secret.namespace.clone())
            .chain(
                snapshot
                    .workloads
                    .rows
                    .iter()
                    .filter_map(|row| row.namespace.clone()),
            )
            .collect();
        names.sort();
        names.dedup();
        names
    };
    let selection = NAMESPACE_SELECTION.read().clone();
    let secrets: Vec<_> = snapshot
        .secrets
        .iter()
        .filter(|secret| selection_matches(&selection, Some(secret.namespace.as_str())))
        .collect();

    let shell = ShellState {
        cluster: snapshot.context.clone(),
        namespace: "default".into(),
        connected: snapshot.connected,
        prometheus: None,
    };
    let registrations = RegistrationStore::default();
    let sections = sidebar_model(&registrations);
    let status_entries = status_bar_model(&shell, &registrations, "", push_mode());

    let gateway_label = match capabilities.gateway {
        GatewayKind::InProcess => "in-process",
        GatewayKind::ServerSide => "server-side",
    };
    let connection = if snapshot.connected {
        StatusKind::Running
    } else {
        StatusKind::Failed
    };

    // The page has no client router, so a palette navigation is a document
    // load — the same thing a sidebar link does. Everything else the palette
    // could offer stays unwired: this host has no theme store, no CRUD overlay
    // and no cluster registry, and the crate does not list commands a host
    // cannot run.
    let palette_host = PaletteHost::navigation(EventHandler::new(|path: &'static str| {
        let _ = document::eval(&format!("window.location.assign({path:?});"));
    }));

    // The sidebar's cluster button: the context this host serves, read-only
    // because a server-side gateway has no context list to switch between.
    let cluster = ClusterInfo {
        label: shell.cluster_label(),
        detail: None,
        connected: snapshot.connected,
    };
    // The top bar's one action this host owns: re-fetch the snapshot.
    let actions = vec![TopBarAction {
        icon: ShellIcon::Refresh,
        label: "Refresh".into(),
        on_click: EventHandler::new(on_refresh),
    }];

    rsx! {
        AppShell {
            sections: sections.clone(),
            // The sidebar marks the route the host resolved.
            current_route: route.clone(),
            cluster: Some(cluster),
            status: status_entries,
            actions,
            "data-surface": "app",
            // The crate's overlays, the same ones the desktop mounts: the
            // palette on this host's single hook (a navigation is a document
            // load) and the switcher, which has no contexts to list here.
            PaletteKeybind {}
            SwitcherKeybind {}
            ClusterSwitcher {}
            CommandPalette { host: palette_host }
            // The route chrome the desktop mounts too: each surface below is
            // that route's body.
            RouteView {
                route: route.clone(),
                sections,
                namespaces,
                busy: pending(),
                content: match route.as_str() {
                    "/workloads" => Some(workload_surface(&snapshot)),
                    // Declared, not invented: this host has not wired them.
                    "/cluster" | "/config" => None,
                    _ => Some(rsx! {
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
                            onclick: on_refresh_click,
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
                        p { class: "message", "data-round": "{round()}", "round {round()}" }
                    }
                    section { class: "panel", "data-surface": "secrets",
                        h2 { "Secrets" }
                        if secrets.is_empty() {
                            p { class: "message", "data-empty": "secrets", "No secrets in the gateway's scope" }
                        } else {
                            dl { class: "kv-list",
                                for secret in secrets.iter() {
                                    div { class: "kv-row",
                                        dt { class: "resource-name", "{secret.name}" }
                                        dd { class: "namespace", "{secret.namespace}" }
                                    }
                                }
                            }
                        }
                    }
                    }),
                },
            }
        }
    }
}

/// The `/workloads` body: the console's table, or its declared error state.
fn workload_surface(snapshot: &Snapshot) -> Element {
    let workloads = &snapshot.workloads;
    let status = match &workloads.error {
        Some(error) => TableStatus::Error(format!("The gateway could not list pods: {error}")),
        None => TableStatus::Ready,
    };
    rsx! {
        div { "data-surface": "workloads",
            ResourceTable {
                columns: workloads.columns.clone(),
                rows: workloads.rows.clone(),
                status,
                empty_message: Some("No pods in the gateway's scope".to_string()),
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

/// How long the page waits between polls: the interval half of the degraded
/// bound, so a round-trip and a render fit inside the rest.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10);

/// Wait one poll interval. No timer is shared by both targets: the SSR pass
/// and the tests sit on tokio, the browser on the page's own timer.
#[cfg(not(target_arch = "wasm32"))]
async fn poll_interval() {
    tokio::time::sleep(POLL_INTERVAL).await;
}

/// Wait one poll interval on the page's timer.
#[cfg(target_arch = "wasm32")]
async fn poll_interval() {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                &resolve,
                POLL_INTERVAL.as_millis() as i32,
            );
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}
