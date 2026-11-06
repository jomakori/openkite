//! HTTP surface the console calls.
//!
//! Each route mirrors a transport the desktop already has: `POST /openkite` is
//! the bridge envelope (there it is the wry asset handler's job), `POST
//! /openkite-spike` carries the console's context and settings ops, and every
//! other path serves the hydration bundle, falling back to the same document
//! `GET /` renders — with the path resolved to the console's route contract, so
//! `GET /workloads` paints the workloads route. `POST /api/gateway` is the
//! same-origin round-trip the hydrating client refreshes through, and `GET
//! /assets/fonts/{file}` is where the stylesheet's `@font-face` URLs land.

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{OriginalUri, Path as UrlPath, RawQuery, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::stream::Stream;
use k8s_openapi::api::core::v1::Pod;
use openkite_api::bridge::{ApiRequest, ApiResponse};
use openkite_host::bridge::Bridge;
use openkite_ui::components::route_views::resolve_route;
use tower_http::services::ServeDir;

use crate::assets::{Bundle, DOCUMENT, IMMUTABLE};
use crate::spike;
use crate::ssr;

/// The shared bridge every bridge request dispatches through.
pub type SharedBridge = Arc<Bridge>;

/// What a handler needs: the bridge, and the hydration pair this process serves.
///
/// The bundle rides in the state because the document has to name the same
/// version the asset route will answer — a document rendered against one
/// version and served assets from another is the mismatch this closes.
#[derive(Clone)]
pub struct HostState {
    /// The bridge every console call dispatches through.
    pub bridge: SharedBridge,
    /// The hashed hydration pair in `OPENKITE_WEB_ROOT`.
    pub bundle: Arc<Bundle>,
    /// Where that pair is staged.
    pub web_root: Arc<PathBuf>,
}

/// Build the host router: the crate-rendered root, the console's endpoints,
/// the versioned hydration pair, the vendored typefaces, then the bundle root.
///
/// The runtime image ships no static document, so a path the bundle has no file
/// for is answered by the console itself rather than a file that is not there.
pub fn router(bridge: SharedBridge, web_root: &Path) -> Router {
    let state = HostState {
        bridge: bridge.clone(),
        bundle: Arc::new(Bundle::load(web_root)),
        web_root: Arc::new(web_root.to_path_buf()),
    };
    let shell: Router = Router::new()
        .route("/{*path}", get(ssr_root))
        .with_state(state.clone());
    let assets = ServeDir::new(web_root).fallback(shell);
    Router::new()
        .route("/", get(ssr_root))
        .route("/assets/fonts/{file}", get(font_get))
        .route("/assets/bundle/{version}/{*file}", get(bundle_get))
        .route("/openkite", post(bridge_post))
        .route("/openkite/events", get(events_get))
        .route("/openkite-spike", post(spike_post))
        .route("/api/gateway", post(gateway_post))
        .fallback_service(assets)
        .with_state(state)
}

/// Serve one file of the hydration set under the version the document names.
///
/// The version is checked against this process's own hash, so a browser that
/// kept a URL from an older build gets a 404 rather than this build's bytes
/// cached under that older name — and the page it loaded cannot hydrate with a
/// wasm or snippet from a different build. Everything the glue reaches by
/// relative import (`_bg.wasm`, `snippets/…`) is under the same version, so the
/// whole module graph is one build. The bytes are immutable by construction,
/// which is what makes the long cache safe.
async fn bundle_get(
    State(state): State<HostState>,
    UrlPath((version, file)): UrlPath<(String, String)>,
) -> Response {
    let Some(file) = state.bundle.resolve(&version, &file) else {
        return (StatusCode::NOT_FOUND, "unknown bundle version").into_response();
    };
    match tokio::fs::read(state.web_root.join(&file)).await {
        Ok(bytes) => (
            [
                (CONTENT_TYPE, Bundle::content_type(&file)),
                (CACHE_CONTROL, IMMUTABLE),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "bundle file is not staged").into_response(),
    }
}

/// Serve one vendored typeface the shell stylesheet declares.
///
/// The bytes are immutable for the life of the binary, so the client is told it
/// never has to ask twice: a browser fetches each face once per install.
async fn font_get(UrlPath(file): UrlPath<String>) -> Response {
    match openkite_ui::assets::face_by_file(&file) {
        Some(face) => (
            [
                (CONTENT_TYPE, "font/woff2"),
                (CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            face.bytes,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "unknown typeface").into_response(),
    }
}

/// Dispatch one bridge POST through the shared [`Bridge`].
///
/// Always answers HTTP 200 with an `ok`/`error` envelope: the JS side reads the
/// envelope, and any non-2xx is treated as "no host here", which would drop the
/// console back to fixtures.
async fn bridge_post(State(state): State<HostState>, body: String) -> Json<ApiResponse> {
    Json(state.bridge.handle_post(&body).await)
}

/// Answer one console context/settings POST, in the same envelope.
async fn spike_post(State(state): State<HostState>, body: String) -> Json<ApiResponse> {
    Json(spike::handle(&body, state.bridge.client().is_some()))
}

/// Stream live push messages to the browser over SSE, so the console stops
/// polling `/api/gateway` for a snapshot and instead reacts to the host's
/// pushes as they happen.
///
/// Each [`openkite_host::push::PushMessage`] is sent as one `message` event
/// whose `data` is the message JSON-encoded; a dropped/lagged broadcast
/// receiver resumes from the newest message rather than closing the stream
/// (see [`openkite_host::push::recv_sse`]).
async fn events_get() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = openkite_host::push::sse_subscribe();
    let stream = async_stream::stream! {
        while let Some(msg) = openkite_host::push::recv_sse(&mut rx).await {
            let data = serde_json::to_string(&msg).unwrap_or_default();
            yield Ok(Event::default().event("message").data(data));
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Render the console root server-side from the snapshot the host holds.
///
/// The page is SSR + hydrate: the snapshot and the markup are in the document
/// so a deep link paints before any JavaScript runs, and the client bundle in
/// `OPENKITE_WEB_ROOT` then attaches to the same tree, which is what makes a
/// route reached by URL interactive in the browser host.
///
/// The address is part of the render: the route the path resolved to and the
/// resource detail pane's selection both ride the snapshot, so the server
/// paints the page a reload or a shared link asked for rather than waiting for
/// a client to restore it.
async fn ssr_root(
    State(state): State<HostState>,
    OriginalUri(uri): OriginalUri,
    RawQuery(query): RawQuery,
) -> Response {
    let route = resolve_route(uri.path(), &sidebar_sections());
    // `?fixture=` drives the states a surface claims to render (G4). It is read
    // only where there is no cluster credential to disagree with it: a host
    // that has a client answers from the cluster, not from a query string.
    let fixture = if state.bridge.client().is_some() {
        crate::fixture::FixtureState::Ready
    } else {
        crate::fixture::FixtureState::from_query(query_param(query.as_deref(), "fixture"))
    };
    let selection = query
        .as_deref()
        .and_then(openkite_ui::components::resource_pane::ResourceRef::from_query);
    let snapshot = ssr_snapshot(&state.bridge, &route, fixture)
        .await
        .with_selection(selection);
    let page = ssr::render_page(
        &snapshot,
        &ssr::RenderOptions::hydrating(state.bundle.client_url()),
    );
    (
        [
            (CONTENT_TYPE, "text/html; charset=utf-8"),
            (CACHE_CONTROL, DOCUMENT),
        ],
        page,
    )
        .into_response()
}

/// Answer the hydrating client's refresh: a fresh snapshot in the envelope
/// `GatewayResponse` defines, carrying the table the workloads route paints.
async fn gateway_post(State(state): State<HostState>, body: String) -> Response {
    let request: Result<crate::app::GatewayRequest, _> = serde_json::from_str(&body);
    let snapshot = match request {
        Ok(_) => ssr_snapshot(&state.bridge, "/", crate::fixture::FixtureState::Ready)
            .await
            .with_workloads(
                workload_table(&state.bridge, crate::fixture::FixtureState::Ready).await,
            ),
        Err(_) => ssr::Snapshot::default(),
    };
    let response = crate::app::GatewayResponse { snapshot };
    Json(response).into_response()
}

/// Build the snapshot the SSR pass renders against: the host's capabilities,
/// identity and route, plus the workloads the route paints.
async fn ssr_snapshot(
    bridge: &Bridge,
    route: &str,
    fixture: crate::fixture::FixtureState,
) -> ssr::Snapshot {
    let client = bridge.client();
    let connected = client.is_some();
    let context = if crate::in_cluster() {
        Some("in-cluster".to_string())
    } else {
        kube::Config::infer()
            .await
            .ok()
            .map(|_| "kubeconfig".to_string())
    };
    // The overview's identity line is the same context the snapshot carries;
    // clone it before `from_gateway` takes ownership.
    let context_label = context.clone().unwrap_or_default();
    let snapshot = if let Some(client) = client {
        let gateway: Arc<dyn openkite_api::gateway::Gateway> =
            Arc::new(openkite_host::gateway::KubeGateway::server_side(client));
        ssr::Snapshot::from_gateway(&gateway, connected, context).await
    } else {
        let gateway = crate::fixture::make_fixture_gateway_in(fixture);
        ssr::Snapshot::from_gateway(&gateway, connected, context).await
    };
    let snapshot = snapshot.with_route(route).with_busy(fixture.is_loading());

    // The landing dashboard counts what the cluster client can see. Other
    // routes do not pay for those list calls.
    let snapshot = if route == "/" {
        let (tiles, overview_error) = overview_summary(bridge, fixture).await;
        let summary = openkite_ui::components::overview::OverviewSummary {
            context: context_label,
            gateway: if connected {
                "server-side".to_string()
            } else {
                "in-process".to_string()
            },
            connection: if connected {
                "Connected".to_string()
            } else {
                "Disconnected".to_string()
            },
            tiles: tiles.tiles,
        };
        snapshot
            .with_overview(summary)
            .with_overview_error(overview_error)
    } else {
        snapshot
    };

    if route == "/workloads" {
        snapshot.with_workloads(workload_table(bridge, fixture).await)
    } else {
        snapshot
    }
}

/// The `key=value` a raw query string carries, undecoded: the fixture states
/// are plain words and the address is otherwise someone else's to parse.
fn query_param<'a>(query: Option<&'a str>, key: &str) -> Option<&'a str> {
    query?.trim_start_matches('?').split('&').find_map(|pair| {
        let (raw_key, value) = pair.split_once('=').unwrap_or((pair, ""));
        (raw_key == key).then_some(value)
    })
}

/// The `/workloads` table: the cluster's pods, the fixture's answer, or the
/// gateway's refusal. A loading answer paints the loading state, never an empty
/// table — the two say different things to the person reading them (G4).
async fn workload_table(
    bridge: &Bridge,
    fixture: crate::fixture::FixtureState,
) -> ssr::WorkloadsTable {
    use crate::fixture::FixtureState;
    let columns = openkite::workloads::pod_columns();
    match fixture {
        FixtureState::Loading => {
            return ssr::WorkloadsTable {
                columns,
                rows: Vec::new(),
                error: None,
                loading: true,
            }
        }
        FixtureState::Error => {
            return ssr::WorkloadsTable {
                columns,
                rows: Vec::new(),
                error: Some(crate::fixture::FIXTURE_ERROR.to_string()),
                loading: false,
            }
        }
        FixtureState::Empty => {
            return ssr::WorkloadsTable {
                columns,
                rows: Vec::new(),
                error: None,
                loading: false,
            }
        }
        FixtureState::Ready => {}
    }
    let request = ApiRequest::List {
        kind: "pods".to_string(),
        ns: None,
    };
    match bridge.execute("console", request).await {
        ApiResponse::Ok { result } => match pods(&result) {
            Ok(pods) => ssr::WorkloadsTable {
                columns,
                rows: pods.iter().map(openkite::workloads::pod_row).collect(),
                error: None,
                loading: false,
            },
            Err(error) => ssr::WorkloadsTable {
                columns,
                rows: Vec::new(),
                error: Some(error),
                loading: false,
            },
        },
        ApiResponse::Error { error } => ssr::WorkloadsTable {
            columns,
            rows: Vec::new(),
            error: Some(error),
            loading: false,
        },
    }
}

/// The sidebar model the shell renders, so a path resolves against it.
fn sidebar_sections() -> Vec<openkite_ui::shell::ShellSection> {
    openkite_ui::shell::sidebar_model(&openkite_ui::plugin_api::RegistrationStore::default())
}

/// The overview's tiles: counts the cluster client reports, or the fixture's
/// answer, or the client's refusal.
///
/// The four lists are asked for together — the landing surface should cost one
/// round of reads, not four serial ones. A refusal on any of them paints the
/// error state rather than a partial summary: a dashboard that quietly drops
/// the half it could not read is worse than one that says it failed.
async fn overview_summary(
    bridge: &Bridge,
    fixture: crate::fixture::FixtureState,
) -> (
    openkite_ui::components::overview::OverviewSummary,
    Option<String>,
) {
    use crate::fixture::FixtureState;
    use openkite_ui::components::overview::{OverviewSummary, OverviewTile, TileTone};

    match fixture {
        FixtureState::Loading | FixtureState::Error | FixtureState::Empty => {
            // The identity line is filled by the caller; the tiles are what the
            // fixture decides here, and loading/error/empty all carry none.
            let error = match fixture {
                FixtureState::Error => Some(crate::fixture::FIXTURE_ERROR.to_string()),
                _ => None,
            };
            return (OverviewSummary::default(), error);
        }
        FixtureState::Ready => {}
    }

    let (pods, nodes, deployments, services) = tokio::join!(
        list_items(bridge, "pods"),
        list_items(bridge, "nodes"),
        list_items(bridge, "deployments"),
        list_items(bridge, "services"),
    );

    let mut error = None;
    for refusal in [&pods, &nodes, &deployments, &services] {
        if let Err(message) = refusal {
            error.get_or_insert_with(|| message.clone());
        }
    }
    if let Some(error) = error {
        return (OverviewSummary::default(), Some(error));
    }

    let pods = pods.unwrap_or_default();
    let nodes = nodes.unwrap_or_default();
    let deployments = deployments.unwrap_or_default();
    let services = services.unwrap_or_default();

    let running = pods.iter().filter(|p| phase(p) == "Running").count();
    let pending = pods.iter().filter(|p| phase(p) == "Pending").count();
    let failed = pods
        .iter()
        .filter(|p| matches!(phase(p).as_str(), "Failed" | "Unknown"))
        .count();
    let pod_tone = if pods.is_empty() {
        TileTone::Neutral
    } else if failed > 0 {
        TileTone::Bad
    } else if pending > 0 {
        TileTone::Warn
    } else if running == pods.len() {
        TileTone::Ok
    } else {
        TileTone::Neutral
    };

    let ready_nodes = nodes.iter().filter(|node| node_is_ready(node)).count();
    let node_tone = if nodes.is_empty() {
        TileTone::Neutral
    } else if ready_nodes == nodes.len() {
        TileTone::Ok
    } else if ready_nodes == 0 {
        TileTone::Bad
    } else {
        TileTone::Warn
    };

    let ready_deployments = deployments
        .iter()
        .filter(|deployment| deployment_is_available(deployment))
        .count();
    let deployment_tone = if deployments.is_empty() {
        TileTone::Neutral
    } else if ready_deployments == deployments.len() {
        TileTone::Ok
    } else {
        TileTone::Warn
    };

    let tiles = vec![
        OverviewTile::new(
            "pods",
            "Pods",
            format!("{running}/{}", pods.len()),
            format!("running · {pending} pending · {failed} failing"),
            pod_tone,
        ),
        OverviewTile::new(
            "nodes",
            "Nodes",
            format!("{ready_nodes}/{}", nodes.len()),
            "ready".to_string(),
            node_tone,
        ),
        OverviewTile::new(
            "deployments",
            "Deployments",
            format!("{ready_deployments}/{}", deployments.len()),
            "fully available".to_string(),
            deployment_tone,
        ),
        OverviewTile::new(
            "services",
            "Services",
            services.len().to_string(),
            "in the cluster".to_string(),
            TileTone::Neutral,
        ),
    ];

    (
        OverviewSummary {
            tiles,
            ..Default::default()
        },
        None,
    )
}

/// One `list` against the cluster client, as the items it returned.
async fn list_items(bridge: &Bridge, kind: &str) -> Result<Vec<serde_json::Value>, String> {
    let request = ApiRequest::List {
        kind: kind.to_string(),
        ns: None,
    };
    match bridge.execute("console", request).await {
        ApiResponse::Ok { result } => result
            .get("items")
            .and_then(serde_json::Value::as_array)
            .map(|items| items.to_vec())
            .ok_or_else(|| format!("the {kind} list carried no items")),
        ApiResponse::Error { error } => Err(error),
    }
}

/// A pod's phase, or `Unknown` when the list carried none.
fn phase(pod: &serde_json::Value) -> String {
    pod.get("status")
        .and_then(|status| status.get("phase"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Unknown")
        .to_string()
}

/// Whether a node reports `Ready=True`.
fn node_is_ready(node: &serde_json::Value) -> bool {
    node.get("status")
        .and_then(|status| status.get("conditions"))
        .and_then(serde_json::Value::as_array)
        .map(|conditions| {
            conditions.iter().any(|condition| {
                condition.get("type").and_then(serde_json::Value::as_str) == Some("Ready")
                    && condition.get("status").and_then(serde_json::Value::as_str) == Some("True")
            })
        })
        .unwrap_or(false)
}

/// Whether a deployment has as many available replicas as it asked for.
fn deployment_is_available(deployment: &serde_json::Value) -> bool {
    let desired = deployment
        .get("spec")
        .and_then(|spec| spec.get("replicas"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1);
    let available = deployment
        .get("status")
        .and_then(|status| status.get("availableReplicas"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    available >= desired
}

/// The pods a `list pods` answer carries.
fn pods(result: &serde_json::Value) -> Result<Vec<Pod>, String> {
    let items = result
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "the pod list carried no items".to_string())?;
    items
        .iter()
        .map(|item| serde_json::from_value::<Pod>(item.clone()))
        .collect::<Result<Vec<Pod>, _>>()
        .map_err(|err| format!("a pod did not parse: {err}"))
}
