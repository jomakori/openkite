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

use openkite_api::capability::Capabilities;
use openkite_api::pod::pick_default_container;
use openkite_ui::components::logs::LogsView;
use openkite_ui::components::namespace_bar::selection_matches;
use openkite_ui::components::overview::OverviewDashboard;
use openkite_ui::components::palette::{CommandPalette, PaletteHost, PaletteKeybind};
use openkite_ui::components::resource_pane::ResourceDetail;
use openkite_ui::components::resource_table::{ResourceTable, TableStatus};
use openkite_ui::components::route_views::RouteView;
use openkite_ui::components::shell::{AppShell, ClusterInfo, ShellIcon, TopBarAction};
use openkite_ui::components::switcher::{ClusterSwitcher, SwitcherKeybind};
use openkite_ui::plugin_api::RegistrationStore;
use openkite_ui::runtime::{
    push_mode, set_push_mode, PushMode, LOGS_BUFFER, LOGS_CONTAINER, NAMESPACE_SELECTION,
    SELECTED_POD,
};
use openkite_ui::shell::{sidebar_model, status_bar_model, ShellState};

use crate::account_menu::AccountMenu;
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

    // Who this host reads the cluster as: the avatar's identity and the account
    // menu's subject. The snapshot carries it on both passes (SSR + hydrate).
    let account = props.snapshot.account.clone();

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

    // The log panel reads the console's selection globals, the way the desktop's
    // does, so `/logs` seeds them from the snapshot once per mount: the SSR pass
    // and the hydrating client then paint the same pod and the same lines. Seeded
    // in a hook, not in the render, so a snapshot refresh cannot throw away lines
    // the tail has appended since (OKT-189).
    let sealed_logs = props.snapshot.logs.clone();
    let sealed_route = route.clone();
    use_hook(move || {
        if sealed_route != "/logs" {
            return;
        }
        *SELECTED_POD.write() = sealed_logs.pod.clone();
        let container = sealed_logs
            .pod
            .as_ref()
            .and_then(|pod| pick_default_container(&pod.container_names()));
        *LOGS_CONTAINER.write() = container.unwrap_or_default();
        let mut buffer = LOGS_BUFFER.write();
        buffer.clear();
        for line in sealed_logs.lines.iter() {
            buffer.push(line.clone());
        }
    });

    // The panel tails: the browser host has no push transport, so `/logs` re-reads
    // the pod's log on the same interval the page polls with, and the feed keeps
    // only the lines the stream added — the SSR lines are already painted.
    let tail_route = route.clone();
    let tail_pane = props.snapshot.logs.clone();
    use_future(move || {
        let route = tail_route.clone();
        let pane = tail_pane.clone();
        async move {
            if route != "/logs" {
                return;
            }
            let Some(pod) = pane.pod.clone() else {
                return;
            };
            let mut feed = crate::logs::LogFeed::seeded(pane.lines.len());
            loop {
                let container = LOGS_CONTAINER.cloned();
                // A refusal mid-stream keeps the lines already painted: the panel
                // says nothing about a failed read, the next poll retries.
                if let Ok(Some(read)) =
                    crate::client::fetch_logs(&pod, Some(container.as_str())).await
                {
                    let fresh = feed.merge(read);
                    if !fresh.is_empty() {
                        let mut buffer = LOGS_BUFFER.write();
                        for line in fresh {
                            buffer.push(line);
                        }
                    }
                }
                poll_interval().await;
            }
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

    // The namespace bar includes every namespace represented by the snapshot.
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
    let namespace = namespaces
        .iter()
        .find(|name| {
            snapshot
                .secrets
                .iter()
                .any(|secret| &secret.namespace == *name)
        })
        .cloned()
        .unwrap_or_else(|| "default".into());
    let selection = NAMESPACE_SELECTION.read().clone();
    let secrets: Vec<_> = snapshot
        .secrets
        .iter()
        .filter(|secret| selection_matches(&selection, Some(secret.namespace.as_str())))
        .collect();

    // The state each surface paints, read from the snapshot. The same value
    // lands on `data-state`, so the state a surface is in can be read out of the
    // DOM (G4) instead of guessed from a screenshot: pending is loading, a
    // refusal is error, nothing in scope is empty.
    let gateway_error = snapshot.error.clone();
    // The overview's state has three inputs — the cluster-client read's own
    // refusal, a transport failure from the last refresh, and the pending
    // flag. The model owns the precedence, so the desktop paints the same
    // four states from the same rule.
    let overview_error = last_error().or_else(|| snapshot.overview_error.clone());
    let overview_state = snapshot
        .overview
        .state(snapshot.busy || pending(), overview_error.as_deref());
    let secrets_state = if snapshot.busy {
        "loading"
    } else if gateway_error.is_some() {
        "error"
    } else if secrets.is_empty() {
        "empty"
    } else {
        "ready"
    };

    let shell = ShellState {
        cluster: snapshot.context.clone(),
        namespace,
        connected: snapshot.connected,
        prometheus: None,
    };
    let registrations = RegistrationStore::default();
    let sections = sidebar_model(&registrations);
    let status_entries = status_bar_model(&shell, &registrations, "", push_mode());

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
            // The top bar paints the avatar from this; the account menu below
            // is built on the trigger it renders.
            identity: account.as_ref().map(|account| account.name.clone()),
            "data-surface": "app",
            // The crate's overlays, the same ones the desktop mounts: the
            // palette on this host's single hook (a navigation is a document
            // load) and the switcher, which has no contexts to list here.
            PaletteKeybind {}
            SwitcherKeybind {}
            ClusterSwitcher {}
            CommandPalette { host: palette_host }
            // The account menu hangs off the avatar the shell paints from
            // `identity`; it is this host's overlay, not the crate's.
            if let Some(account) = account.clone() {
                AccountMenu { account, on_refresh: EventHandler::new(on_refresh) }
            }
            // The route chrome the desktop mounts too: each surface below is
            // that route's body.
            RouteView {
                route: route.clone(),
                sections,
                namespaces,
                busy: pending() || snapshot.busy,
                error: gateway_error.clone(),
                content: match route.as_str() {
                    "/workloads" => Some(workload_surface(&snapshot)),
                    "/logs" => Some(logs_surface(&snapshot)),
                    // Declared, not invented: this host has not wired them.
                    "/cluster" | "/config" => None,
                    _ => Some(rsx! {
                    OverviewDashboard {
                        summary: snapshot.overview.clone(),
                        state: overview_state.to_string(),
                        error: overview_error.clone(),
                        on_refresh: EventHandler::new(move |_: ()| refetch()),
                    }
                    section { class: "panel", "data-surface": "capabilities", "data-state": "ready",
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
                    section { class: "panel", "data-surface": "secrets", "data-state": secrets_state,
                        h2 { "Secrets" }
                        if snapshot.busy {
                            div { class: "table-state table-loading", "data-state": "loading",
                                "Loading the gateway's scope…"
                            }
                        } else if let Some(error) = gateway_error.clone() {
                            div { class: "table-state table-error", "data-state": "error", "data-error": "1",
                                "The gateway could not list secrets: {error}"
                            }
                        } else if secrets.is_empty() {
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
    // A table that is still loading is not an empty table: the states are
    // distinct, so they carry distinct values on `data-state` (G4).
    let loading = snapshot.busy || workloads.loading;
    let status = if loading {
        TableStatus::Loading
    } else {
        match &workloads.error {
            Some(error) => TableStatus::Error(format!("The gateway could not list pods: {error}")),
            None => TableStatus::Ready,
        }
    };
    let state = if loading {
        "loading"
    } else if workloads.error.is_some() {
        "error"
    } else if workloads.rows.is_empty() {
        "empty"
    } else {
        "ready"
    };
    rsx! {
        div { "data-surface": "workloads", "data-state": state,
            ResourceTable {
                columns: workloads.columns.clone(),
                rows: workloads.rows.clone(),
                status,
                empty_message: Some("No pods in the gateway's scope".to_string()),
            }
        }
    }
}

/// The `/logs` body: the shared panel mounted on the pod this console is aimed
/// at, or the refusal the log read carried.
///
/// The panel is `openkite_ui::components::logs::LogsView` — the same component
/// the desktop mounts, with its own `.log-panel` markup and level tokens. This
/// host only supplies the pod and the lines; an unreadable log paints the
/// panel's error line inside the same shell rather than an empty body (G4).
fn logs_surface(snapshot: &Snapshot) -> Element {
    let pane = &snapshot.logs;
    if let Some(error) = pane.error.clone() {
        return rsx! {
            div { "data-surface": "logs", "data-state": "error",
                div { class: "log-panel",
                    div { class: "log-body",
                        span { class: "log-level error", "{error}" }
                    }
                }
            }
        };
    }
    let state = if pane.pod.is_some() {
        if pane.lines.is_empty() {
            "empty"
        } else {
            "ready"
        }
    } else {
        "empty"
    };
    rsx! {
        div { "data-surface": "logs", "data-state": state,
            LogsView {}
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
