//! Router + host adapter: core routes plus a root wildcard that dispatches
//! unknown paths through the plugin route table; the `/openkite` bridge asset
//! handler; one-time JS plugin bundle evaluation; and the desktop half of the
//! app shell.
//!
//! The shell chrome itself — the frame, sidebar, top bar, status footer and the
//! overlays the frame mounts — is rendered by
//! [`openkite_ui::components::shell`], the same code the browser console
//! mounts. What stays here is what only a desktop host can do: the host hooks
//! (registration mirror, settings apply, push channel, live watch), the plugin
//! registration, and the `document::eval` surfaces — the plugin bundles, the
//! key listeners, and the overlays those listeners drive. Those are handed to
//! the crate frame as its chrome slot, so a host that cannot provide them
//! renders the crate's explicit stand-in instead.
//!
//! Native RSX surfaces kept on purpose:
//!
//! - [`Route::Logs`] → `openkite_ui::components::logs::LogsView` — the
//!   crate-rendered viewer reads the shared `LOGS_BUFFER`; the desktop host
//!   streams the pod's kube log into that buffer (see [`Logs`]).
//! - [`Route::Terminal`] → [`crate::views::terminal::TerminalView`] —
//!   xterm.js host + pod/container picker + reconnect state machine, on
//!   top of the vendored bundle in `openkite-ui/assets/vendored/xterm/`.
//!   The exec transport is deferred (Phase 1); the view renders the typed
//!   input and surfaces the bridge-pending hint. No crate-rendered
//!   replacement exists.
//! - [`Route::Plugin`] wildcard → [`Plugin`] dispatcher — Rust SDK
//!   plugin routes (`ROUTE_TABLE`) plus JS plugin renderers (the
//!   `JsRouteSlot` mount). The console does not own plugin routing;
//!   declaring the wildcard native is what keeps the SDK contract
//!   and the JS bundle eval surface where their owners (the desktop
//!   host) can evolve them independently.
//!
//! After OKT-137 the console lives only in `crates/openkite-web` (SSR +
//! wasm hydration in the browser image); the desktop keeps these native
//! surfaces, and the routes the browser console serves render a placeholder.

#![allow(non_snake_case)]

use crate::bridge::Bridge;
#[cfg(feature = "desktop")]
use crate::plugin_api::{ApiRequest, ApiResponse, BridgeRequest};
use crate::runtime::{bridge as shared_bridge, js_plugins, REGISTRATIONS};
#[cfg(feature = "desktop")]
use dioxus::desktop::wry;
#[cfg(feature = "desktop")]
use dioxus::desktop::wry::http::Response as AssetHttpResponse;
#[cfg(feature = "desktop")]
use dioxus::desktop::{use_asset_handler, AssetRequest, RequestAsyncResponder};
use dioxus::prelude::*;
use openkite_plugin_sdk::SidebarSection;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::palette::{CommandPalette, PaletteKeybind};
use crate::switcher::{ClusterSwitcher, SwitcherKeybind};
use openkite_ui::components::shell::{AppShell as ShellFrame, NamespaceChip};

/// Plugin sidebar sections (static Rust SDK plugins), populated at startup.
static PLUGIN_SECTIONS: GlobalSignal<Vec<SidebarSection>> = Signal::global(Vec::new);

/// Plugin route table keyed by full path, populated at startup.
static ROUTE_TABLE: GlobalSignal<HashMap<String, fn() -> Element>> = Signal::global(HashMap::new);

/// Evaluated-plugin guard: JS bundles eval exactly once per process, on the
/// first AppShell mount (after the asset handler is registered).
static EVALUATED_JS_PLUGINS: OnceLock<()> = OnceLock::new();

/// Ping channel for the registration mirror.
///
/// `dispatch_bridge_post` answers `/openkite` on a **tokio worker thread**, but
/// mirroring the registration store writes the reactive [`REGISTRATIONS`]
/// signal, and Dioxus signals require the Dioxus runtime — calling one
/// off-runtime panics with `Must be called from inside a Dioxus runtime`
/// (`dioxus-core/src/runtime.rs:100`).
///
/// The runtime itself cannot be carried across the thread boundary (it is held
/// as a `Rc`, which is `!Send + !Sync`), so the tokio side only *pings* this
/// channel. The mirror write runs on the Dioxus side, in the receiver task
/// started by [`AppShell`].
///
/// Latent until now: no JS plugin ever registered (`js plugins discovered
/// count=0`), so nothing had exercised the bridge dispatch end-to-end. The
/// first JS-registered plugin is the first caller. See OKT-94.
static MIRROR_TX: OnceLock<tokio::sync::mpsc::UnboundedSender<()>> = OnceLock::new();

/// Install plugin navigation + routes from the registry (once, in `main`).
pub fn install_plugins(sections: Vec<SidebarSection>, routes: HashMap<String, fn() -> Element>) {
    *PLUGIN_SECTIONS.write() = sections;
    *ROUTE_TABLE.write() = routes;
}

/// App entry: render the router.
pub fn app() -> Element {
    rsx! { Router::<Route> {} }
}

/// Convert a plugin route path string to the wildcard `Route` variant.
fn plugin_route(path: &str) -> Route {
    Route::Plugin {
        path: path
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
    }
}

/// Resolve a URL path string to the matching [`Route`].
///
/// Core routes map 1:1 from their `#[route("/...")]` paths; anything
/// else falls through to the plugin wildcard. Used by the
/// `OPENKITE_ROUTE` test hook (boot the app directly onto a route for
/// deterministic E2E/visual-baseline captures) and by callers resolving
/// a render target from a path.
pub fn route_from_path(path: &str) -> Route {
    match path.trim().trim_matches('/') {
        "" | "home" => Route::Home {},
        "cluster" => Route::Cluster {},
        "workloads" => Route::Workloads {},
        "logs" => Route::Logs {},
        "terminal" => Route::Terminal {},
        "config" => Route::Config {},
        other => plugin_route(other),
    }
}

/// Reconstruct a full path from wildcard segments.
fn full_path(path: &[String]) -> String {
    format!("/{}", path.join("/"))
}

/// Placeholder routes the desktop binary keeps on its surface so the
/// navigation entries stay live: the rich console moved to the SSR/wasm
/// crate and lives in the browser preview image. Removing the routes
/// entirely would shrink the navigation; surfacing the same stub instead
/// keeps every NavItem clickable.
#[derive(Routable, Clone, Debug, PartialEq, Eq)]
pub enum Route {
    #[layout(AppShell)]
    #[route("/")]
    Home {},
    #[route("/cluster")]
    Cluster {},
    #[route("/workloads")]
    Workloads {},
    #[route("/logs")]
    Logs {},
    #[route("/terminal")]
    Terminal {},
    #[route("/config")]
    Config {},
    #[route("/:..path")]
    Plugin { path: Vec<String> },
}

#[component]
fn AppShell() -> Element {
    use_hook(|| {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let _ = MIRROR_TX.set(tx);
        spawn(async move {
            while rx.recv().await.is_some() {
                if let Some(bridge) = shared_bridge() {
                    refresh_registrations(&bridge);
                }
            }
        });
    });

    use_hook(|| {
        if let Some(mut rx) = crate::runtime::install_settings_apply() {
            spawn(async move {
                while rx.recv().await.is_some() {
                    crate::runtime::apply_persisted_os_settings();
                }
            });
        }
    });

    use_hook(|| {
        if let Some(mut rx) = crate::push::install() {
            spawn(async move {
                while let Some(msg) = rx.recv().await {
                    document::eval(&msg.to_js());
                }
            });
        }
    });

    use_effect(move || {
        if let Some(client) = crate::runtime::client() {
            if !crate::state::live::is_watching() {
                crate::state::live::start(client);
            }
        }
    });

    let nav = use_navigator();
    {
        let mut routed = use_signal(|| false);
        use_effect(move || {
            if !*routed.read() {
                routed.set(true);
                if let Ok(path) = std::env::var("OPENKITE_ROUTE") {
                    let path = path.trim().to_string();
                    if !path.is_empty() {
                        let target = route_from_path(&path);
                        tracing::info!(route = %path, "OPENKITE_ROUTE: booting onto route");
                        nav.push(target);
                    }
                }
            }
        });
    }

    #[cfg(feature = "desktop")]
    use_asset_handler(
        "openkite",
        |req: AssetRequest, responder: RequestAsyncResponder| {
            dispatch_bridge_post(req, responder);
        },
    );

    use_effect(move || {
        if EVALUATED_JS_PLUGINS.set(()).is_ok() {
            for bundle in js_plugins() {
                match crate::plugin_js::load_source(&bundle) {
                    Ok(source) => {
                        tracing::info!(plugin = %bundle.name, "evaluating js plugin bundle");
                        let wrapped = format!(
                            "window.__openkite_plugin = {:?};\n{}\nwindow.__openkite_plugin = null;",
                            bundle.name, source,
                        );
                        document::eval(&wrapped);
                    }
                    Err(error) => {
                        tracing::warn!(plugin = %bundle.name, %error, "js plugin bundle load failed");
                    }
                }
            }
        }
    });

    // The route the sidebar marks active; `Routable`'s `Display` writes the
    // URL path, which is the same vocabulary the entries carry.
    let current_route = use_route::<Route>().to_string();

    rsx! {
        ShellFrame {
            sections: shell_sections(),
            current_route,
            namespaces: namespace_chips(),
            status: status_entries(),
            on_navigate: Some(EventHandler::new(move |route: String| {
                nav.push(route_from_path(&route));
            })),
            on_toggle_namespace: Some(EventHandler::new(|ns: String| {
                crate::runtime::toggle_namespace(ns);
            })),
            // Host-only chrome: the key listeners and the overlays they drive.
            // The crate frame renders it when the host advertises native
            // window chrome (this host always does) and states what is missing
            // when it does not.
            chrome: rsx! {
                SwitcherKeybind {}
                PaletteKeybind {}
                ClusterSwitcher {}
                CommandPalette {}
            },
            Outlet::<Route> {}
        }
    }
}

/// The sidebar the crate shell renders: the desktop's flat core navigation
/// (the terminal entry gated on the host capability), then the static Rust-SDK
/// plugin sections, then the JS-plugin sections mirrored from the bridge.
///
/// The nav divider separates each of those blocks from the previous one rather
/// than being drawn per section, which is what the desktop has always rendered.
fn shell_sections() -> Vec<crate::shell::ShellSection> {
    use crate::shell::{core_nav, plugin_sections, ShellNavItem, ShellSection};

    let mut sections = vec![ShellSection {
        label: String::new(),
        accent: None,
        divider: false,
        items: core_nav(openkite_ui::runtime::terminal_can_render()),
    }];

    let sdk_sections = PLUGIN_SECTIONS.read();
    for (index, section) in sdk_sections.iter().enumerate() {
        sections.push(ShellSection {
            label: section.label.clone(),
            accent: Some(
                section
                    .accent_color
                    .clone()
                    .unwrap_or_else(|| "var(--accent)".into()),
            ),
            divider: index == 0,
            items: section
                .entries
                .iter()
                .map(|entry| ShellNavItem {
                    label: entry.label.clone(),
                    route: entry.route.clone(),
                    plugin: Some(section.label.clone()),
                })
                .collect(),
        });
    }

    let registrations = REGISTRATIONS.read();
    for (index, section) in plugin_sections(&registrations).into_iter().enumerate() {
        sections.push(ShellSection {
            divider: index == 0,
            ..section
        });
    }

    sections
}

/// The top bar's chips: one per namespace on the active cluster, active when
/// the namespace is in the selected set.
fn namespace_chips() -> Vec<NamespaceChip> {
    let namespaces = crate::runtime::NAMESPACES.read().clone();
    let selected = crate::runtime::SELECTED_NAMESPACES.read().clone();
    namespaces
        .into_iter()
        .map(|namespace| NamespaceChip {
            active: selected.iter().any(|selected| selected == &namespace),
            label: namespace,
            context: false,
        })
        .collect()
}

/// The status footer's entries for the current connection state.
fn status_entries() -> Vec<crate::shell::StatusBarEntry> {
    let state = crate::shell::ShellState {
        cluster: crate::runtime::CONTEXT.read().clone(),
        namespace: "default".into(),
        connected: crate::runtime::CLIENT.read().is_some(),
        prometheus: crate::runtime::PROMETHEUS.read().clone(),
    };
    let version = crate::version::reported();
    crate::shell::status_bar_model(
        &state,
        &REGISTRATIONS.read(),
        version.as_deref().unwrap_or_default(),
    )
}

/// Dispatch one bridge POST from the webview.
///
/// Non-POST requests and a missing bridge answer immediately with an error
/// envelope; real work runs on the ambient tokio runtime (the same runtime
/// dioxus uses for its own protocol handlers) and answers through the async
/// responder, keeping the UI thread free.
#[cfg(feature = "desktop")]
fn dispatch_bridge_post(req: AssetRequest, responder: RequestAsyncResponder) {
    if req.method() != wry::http::Method::POST {
        responder.respond(json_response(ApiResponse::Error {
            error: "method not allowed: bridge requests must be POST".into(),
        }));
        return;
    }
    let Some(bridge) = shared_bridge() else {
        responder.respond(json_response(ApiResponse::Error {
            error: "bridge not installed".into(),
        }));
        return;
    };
    let Ok(text) = std::str::from_utf8(req.body()) else {
        responder.respond(json_response(ApiResponse::Error {
            error: "request body is not utf-8".into(),
        }));
        return;
    };
    let text = text.to_string();
    let is_register = serde_json::from_str::<BridgeRequest>(&text)
        .map(|envelope| matches!(envelope.request, ApiRequest::Register { .. }))
        .unwrap_or(false);
    tokio::spawn(async move {
        let resp = bridge.handle_post(&text).await;
        if is_register {
            let envelope =
                serde_json::to_string(&resp).unwrap_or_else(|err| format!("serialize: {err}"));
            tracing::info!(envelope = %envelope, "bridge register response");
        }
        responder.respond(json_response(resp));
        match MIRROR_TX.get() {
            Some(tx) => {
                let _ = tx.send(());
            }
            None => {
                tracing::warn!("registration mirror skipped: Dioxus mirror task not started yet")
            }
        }
    });
}

/// Mirror the bridge's registration store into the reactive `REGISTRATIONS`
/// signal; the sidebar + status footer render from the mirror.
///
/// A failed borrow just means a render holds the signal right now — safe to
/// skip, the next register POST re-mirrors.
fn refresh_registrations(bridge: &Arc<Bridge>) {
    match REGISTRATIONS.try_write_unchecked() {
        Ok(mut mirror) => {
            let snapshot = bridge.snapshot();
            if *mirror != snapshot {
                *mirror = snapshot;
                tracing::info!(plugins = ?mirror.plugins(), "registration mirror updated");
            }
        }
        Err(_) => {
            tracing::warn!("registration mirror busy; sidebar refresh deferred to next register")
        }
    }
}

/// Serialize an [`ApiResponse`] into a JSON HTTP response for the webview.
#[cfg(feature = "desktop")]
pub(crate) fn json_response(resp: ApiResponse) -> AssetHttpResponse<Vec<u8>> {
    let body = serde_json::to_vec(&resp).unwrap_or_else(|err| {
        serde_json::to_vec(&ApiResponse::Error {
            error: format!("serialize response: {err}"),
        })
        .expect("serializing an error fallback cannot fail")
    });
    AssetHttpResponse::builder()
        .header("Content-Type", "application/json")
        .body(body)
        .expect("static response parts")
}

#[component]
fn Home() -> Element {
    rsx! { BrowserOnlySurface {} }
}

#[component]
fn Cluster() -> Element {
    rsx! { BrowserOnlySurface {} }
}

#[component]
fn Workloads() -> Element {
    rsx! { BrowserOnlySurface {} }
}

#[component]
fn Logs() -> Element {
    use crate::logs::{LogOptions, LogStream};
    use futures::{AsyncBufReadExt, StreamExt};
    use k8s_openapi::api::core::v1::Pod;
    use kube::Api;
    use openkite_ui::runtime::{LOGS_BUFFER, LOGS_CONTAINER, SELECTED_POD};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tokio::task::JoinHandle;

    // Host-side stream controller: drain the selected pod's container logs
    // into the shared buffer the crate view renders.
    let mut task_slot = use_hook(|| CopyValue::new(None::<JoinHandle<()>>));

    use_effect(move || {
        if let Some(handle) = task_slot.write().take() {
            handle.abort();
        }
        let Some(pod) = SELECTED_POD.read().clone() else {
            return;
        };
        let container_name = LOGS_CONTAINER.cloned();
        if container_name.is_empty() {
            return;
        }
        let Some(client) = crate::runtime::client() else {
            return;
        };
        LOGS_BUFFER.write().clear();

        let api: Api<Pod> = Api::namespaced(
            client,
            &pod.namespace.clone().unwrap_or_else(|| "default".into()),
        );
        let name = pod.name.clone();
        let cont = container_name.clone();
        let pending: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        let handle = tokio::spawn(async move {
            let opts = LogOptions {
                container: Some(cont),
                follow: true,
                tail_lines: Some(5000),
                timestamps: true,
            };
            let reader = match LogStream::new(api, name, opts).open().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "log stream open failed");
                    return;
                }
            };

            let mut line_stream = reader.lines();
            let mut ticker = tokio::time::interval(Duration::from_millis(50));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    biased;
                    next = line_stream.next() => {
                        match next {
                            Some(Ok(text)) => pending.lock().await.push(text),
                            Some(Err(e)) => {
                                tracing::warn!(error = %e, "log line read failed");
                                break;
                            }
                            None => break,
                        }
                    }
                    _ = ticker.tick() => {}
                }
                let drained: Vec<String> = {
                    let mut guard = pending.lock().await;
                    std::mem::take(&mut *guard)
                };
                if !drained.is_empty() {
                    let mut buf = LOGS_BUFFER.write();
                    for line in drained {
                        buf.push(line);
                    }
                }
            }
        });

        *task_slot.write() = Some(handle);
    });

    rsx! { openkite_ui::components::logs::LogsView {} }
}

#[component]
fn Terminal() -> Element {
    if !openkite_ui::runtime::terminal_can_render() {
        return rsx! { crate::views::terminal::TerminalUnsupported {} };
    }
    rsx! { crate::views::terminal::TerminalView {} }
}

#[component]
fn Config() -> Element {
    rsx! { BrowserOnlySurface {} }
}

/// Placeholder every browse route renders after the React console was
/// decommissioned (OKT-137): the rich console now lives in the browser
/// preview image, where `crates/openkite-web` server-renders it from the
/// shared UI crate. The routes stay in the [`Route`] enum so the
/// navigation entries the sidebar renders still resolve to a real surface
/// rather than 404, and the native chrome (Logs, Terminal, the plugin
/// wildcard) stays available where it always has been.
#[component]
fn BrowserOnlySurface() -> Element {
    rsx! {
        div { class: "not-found",
            h2 { "OpenKite" }
            p { "The console is served from the browser preview image (openkite-web)." }
        }
    }
}

/// Wildcard dispatcher: reconstruct the path, look it up in the static
/// Rust SDK route table, then fall back to the JS-registered renderer
/// paths. A JS match renders a `JsRouteSlot`; otherwise the 404 fallback.
#[component]
fn Plugin(path: Vec<String>) -> Element {
    let full = full_path(&path);

    *crate::runtime::CURRENT_ROUTE.write() = full.clone();

    let table = ROUTE_TABLE.read();
    if let Some(render) = table.get(&full) {
        return (render)();
    }
    drop(table);

    let registrations = REGISTRATIONS.read();
    let is_js_route = registrations
        .all_renderer_paths()
        .iter()
        .any(|(_, p)| *p == full);
    drop(registrations);
    if is_js_route {
        return rsx! { JsRouteSlot { path: full } };
    }

    rsx! {
        div { class: "not-found",
            h2 { "404" }
            p { "No view for /{full}" }
            Link { to: Route::Home {}, "Back home" }
        }
    }
}

/// Mount node for a JS-owned route. Renders a `<div
/// data-js-route-mount={path}>` inside the host main outlet (NOT a
/// `position: fixed` overlay), then dispatches
/// `window.openkite._renderRoute(path, container)` via `document::eval`
/// in a `use_effect` that re-runs on every path change. The plugin's
/// render fn is responsible for idempotency (call the previous unmount
/// before mounting new UI; storing the unmount on the container keeps
/// re-runs cheap).
#[component]
fn JsRouteSlot(path: String) -> Element {
    let source = format!(
        r#"(function() {{
          var el = document.querySelector('[data-js-route-mount="{}"]');
          if (!el) return;
          if (window.openkite && typeof window.openkite._renderRoute === 'function') {{
            window.openkite._renderRoute('{}', el);
          }}
        }})();"#,
        path.replace('\\', "\\\\").replace('\'', "\\'"),
        path.replace('\\', "\\\\").replace('\'', "\\'")
    );

    use_effect(move || {
        document::eval(&source);
    });

    rsx! {
        div { class: "js-route-slot", "data-js-route-mount": "{path}" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_route_splits_path_into_segments() {
        match plugin_route("/argocd/apps") {
            Route::Plugin { path } => assert_eq!(path, vec!["argocd", "apps"]),
            other => panic!("expected Plugin variant, got {other:?}"),
        }
    }

    #[test]
    fn plugin_route_strips_trailing_slash() {
        match plugin_route("/argocd/apps/") {
            Route::Plugin { path } => assert_eq!(path, vec!["argocd", "apps"]),
            other => panic!("expected Plugin variant, got {other:?}"),
        }
    }

    #[test]
    fn full_path_round_trips_through_plugin_route() {
        let Route::Plugin { path } = plugin_route("/argocd/apps") else {
            panic!("expected Plugin variant");
        };
        assert_eq!(full_path(&path), "/argocd/apps");
    }

    #[test]
    fn registration_renderers_are_exposed_by_path() {
        use crate::plugin_api::{PluginRegistration, RegistrationStore, RendererSpec};
        let mut store = RegistrationStore::new();
        store.upsert(
            "argocd",
            PluginRegistration {
                renderers: vec![RendererSpec {
                    path: "/argocd/apps".into(),
                }],
                ..PluginRegistration::default()
            },
        );
        let renderers = store.all_renderer_paths();
        assert_eq!(renderers, vec![("argocd", "/argocd/apps")]);
    }

    #[test]
    fn route_from_path_maps_core_routes() {
        assert_eq!(route_from_path("/"), Route::Home {});
        assert_eq!(route_from_path(""), Route::Home {});
        assert_eq!(route_from_path("home"), Route::Home {});
        assert_eq!(route_from_path("/home"), Route::Home {});
        assert_eq!(route_from_path("/cluster"), Route::Cluster {});
        assert_eq!(route_from_path("cluster"), Route::Cluster {});
        assert_eq!(route_from_path("/workloads"), Route::Workloads {});
        assert_eq!(route_from_path("/logs"), Route::Logs {});
        assert_eq!(route_from_path("/terminal"), Route::Terminal {});
        assert_eq!(route_from_path("/config"), Route::Config {});
    }

    #[test]
    fn route_from_path_falls_through_to_plugin_wildcard() {
        match route_from_path("/argocd/apps") {
            Route::Plugin { path } => assert_eq!(path, vec!["argocd", "apps"]),
            other => panic!("expected Plugin variant, got {other:?}"),
        }
        match route_from_path("argocd") {
            Route::Plugin { path } => assert_eq!(path, vec!["argocd"]),
            other => panic!("expected Plugin variant, got {other:?}"),
        }
    }

    #[test]
    fn plugin_route_all_slashes_yields_no_segments() {
        let Route::Plugin { path } = plugin_route("///") else {
            panic!("expected Plugin variant");
        };
        assert!(path.is_empty());
        assert_eq!(full_path(&path), "/");
    }

    #[test]
    fn json_response_serializes_ok_and_error_envelopes() {
        let ok = json_response(ApiResponse::Ok {
            result: serde_json::json!({ "items": [] }),
        });
        assert_eq!(
            ok.headers().get("content-type").map(|v| v.as_bytes()),
            Some(&b"application/json"[..])
        );
        let body: ApiResponse = serde_json::from_slice(ok.body()).unwrap();
        assert_eq!(
            body,
            ApiResponse::Ok {
                result: serde_json::json!({ "items": [] })
            }
        );

        let err = json_response(ApiResponse::Error {
            error: "bridge not installed".into(),
        });
        let body: ApiResponse = serde_json::from_slice(err.body()).unwrap();
        assert_eq!(
            body,
            ApiResponse::Error {
                error: "bridge not installed".into()
            }
        );
    }
}
