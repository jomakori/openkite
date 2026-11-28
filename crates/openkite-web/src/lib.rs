//! Browser shell: axum server that server-renders the openkite-ui console from
//! live cluster data (see `live.rs` for the cluster adapter).

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use dioxus::prelude::*;
use openkite_ui::overview::{BoardAction, OverviewData, OverviewLayout};
use openkite_ui::table::{Row, TableState};
use openkite_ui::{AppShell, Connection, Counts, Overview, ResourceTable};

#[cfg(feature = "fixtures")]
pub mod fixtures;
mod live;
pub use live::Live;

/// Version baked in at build time; the release workflow passes the git tag.
const BUILD_VERSION: &str = match option_env!("OPENKITE_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

/// Console stylesheet, served verbatim at `/styles/console.css`.
const CONSOLE_CSS: &str = include_str!("../../openkite-ui/assets/console.css");
/// Chart stylesheet, served at `/styles/charts.css`. It lives beside the chart component
/// rather than in the shared console sheet, and only adds the states the console lacks.
const CHARTS_CSS: &str = include_str!("../../openkite-ui/src/chart/chart.css");
/// Icon sprite the shell markup references by fragment id (`#i-name`).
const ICONS_SVG: &str = include_str!("../../openkite-ui/assets/icons.svg");
/// Absolute path the `asset!` macro bakes into icon hrefs at compile time inside
/// `openkite-ui` (no bundler runs for this plain `cargo` build); stripped below so
/// the inline sprite's fragment ids resolve instead of a dead filesystem link.
const UI_CRATE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../openkite-ui");

#[derive(Clone)]
struct AppState {
    live: Arc<Live>,
}

/// Routes served by the browser shell.
pub fn router(live: Arc<Live>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/", get(overview))
        .route("/table", get(table))
        .route("/events", get(events))
        .route("/styles/console.css", get(styles))
        .route("/styles/charts.css", get(chart_styles))
        .fallback(not_found)
        .with_state(AppState { live })
}

/// Serve until the process receives SIGTERM or Ctrl-C.
pub async fn serve() -> std::io::Result<()> {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080);
    let address = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    let listener = tokio::net::TcpListener::bind(address).await?;
    let live = Arc::new(Live::connect().await);
    tracing::info!(%address, version = version(), "openkite-web listening");

    axum::serve(listener, router(live))
        .with_graceful_shutdown(shutdown_signal())
        .await
}

/// Release tag from the environment, falling back to the build-time value.
pub fn version() -> String {
    std::env::var("OPENKITE_VERSION").unwrap_or_else(|_| BUILD_VERSION.to_owned())
}

async fn healthz() -> &'static str {
    "ok"
}

async fn styles() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], CONSOLE_CSS)
}

async fn chart_styles() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], CHARTS_CSS)
}

async fn overview(State(state): State<AppState>) -> impl IntoResponse {
    let data = state.live.overview().await;
    let connection = state.live.connection();
    let cluster = state.live.cluster();
    Html(render_overview(data, connection, cluster))
}

async fn table(State(state): State<AppState>) -> impl IntoResponse {
    // The pods page renders the trend column by default, so this fetch asks for readings;
    // hiding the column stops the drawing, and the feed pauses with it.
    let rows = state.live.table(true).await;
    let connection = state.live.connection();
    let cluster = state.live.cluster();
    let permissions = state.live.permissions("", "pods", None).await;
    Html(render_table(rows, connection, cluster, permissions))
}

/// Events page query: `?uid=<object uid>` scopes the list to one involved object, the same
/// scope the per-object Events tab asks the apiserver for.
#[derive(serde::Deserialize)]
struct EventsQuery {
    uid: Option<String>,
}

async fn events(
    State(state): State<AppState>,
    Query(query): Query<EventsQuery>,
) -> impl IntoResponse {
    let uid = query.uid.filter(|uid| !uid.is_empty());
    let rows = state.live.events(uid.as_deref()).await;
    let connection = state.live.connection();
    let cluster = state.live.cluster();
    Html(render_events(rows, connection, cluster, uid))
}

async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Html(not_found_html()))
}

async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    let terminate = async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            match signal(SignalKind::terminate()) {
                Ok(mut stream) => {
                    stream.recv().await;
                }
                Err(error) => tracing::error!(%error, "cannot watch SIGTERM"),
            }
        }
        #[cfg(not(unix))]
        std::future::pending::<()>().await;
    };

    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
    tracing::info!("shutdown signal received");
}

/// Builds `Counts` from the counts map `Live::overview` already filled.
fn counts_from_data(data: &OverviewData) -> Counts {
    let mut counts = Counts::new();
    for (kind, rows) in &data.counts {
        counts = counts.with(kind, *rows);
    }
    counts
}

#[derive(Props, Clone, PartialEq)]
struct OverviewPageProps {
    data: OverviewData,
    cluster: String,
    connection: Connection,
}

#[allow(non_snake_case)]
fn OverviewPage(props: OverviewPageProps) -> Element {
    let on_action = use_callback(|_: BoardAction| {});
    let counts = counts_from_data(&props.data);
    rsx! {
        AppShell {
            counts,
            active_path: "/overview".to_string(),
            cluster: props.cluster.clone(),
            connection: props.connection,
            children: rsx! {
                Overview {
                    layout: OverviewLayout::default(),
                    data: props.data.clone(),
                    namespaces: Vec::new(),
                    on_action,
                }
            },
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct TablePageProps {
    rows: Vec<Row>,
    cluster: String,
    connection: Connection,
    permissions: openkite_ui::access::Permissions,
}

#[allow(non_snake_case)]
fn TablePage(props: TablePageProps) -> Element {
    let on_open = use_callback(|_: String| {});
    let on_state = use_callback(|_: TableState| {});
    let on_scroll = use_callback(|_: f64| {});
    let counts = Counts::new().with("pods", props.rows.len());
    let state = TableState::default();
    rsx! {
        AppShell {
            counts,
            active_path: "/workloads/pods".to_string(),
            cluster: props.cluster.clone(),
            connection: props.connection,
            children: rsx! {
                div { class: "page-head",
                    div {
                        h1 { "Pods" }
                        p { "All pods in {props.cluster}" }
                    }
                }
                ResourceTable {
                    kind: "pods".to_string(),
                    label: "Pods".to_string(),
                    rows: props.rows.clone(),
                    state: state.clone(),
                    namespaces: Vec::new(),
                    on_open,
                    on_state,
                    scroll_top: 0.0,
                    on_scroll,
                    permissions: props.permissions.clone(),
                }
            },
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct EventsPageProps {
    rows: Vec<openkite_ui::inspector::Event>,
    cluster: String,
    connection: Connection,
    object_uid: Option<String>,
}

#[allow(non_snake_case)]
fn EventsPage(props: EventsPageProps) -> Element {
    let subtitle = match &props.object_uid {
        Some(uid) => format!("Events involving object {uid} in {}", props.cluster),
        None => format!("All events in {}", props.cluster),
    };
    let counts = Counts::new().with("events", props.rows.len());
    rsx! {
        AppShell {
            counts,
            active_path: "/events".to_string(),
            cluster: props.cluster.clone(),
            connection: props.connection,
            children: rsx! {
                div { class: "page-head",
                    div {
                        h1 { "Events" }
                        p { "{subtitle}" }
                    }
                }
                if props.rows.is_empty() {
                    p { class: "empty-hint", "No events in the last 15 minutes." }
                }
                ul { class: "ev-list",
                    for event in props.rows.iter() {
                        li { class: if event.kind == "Warning" { "ev warn" } else { "ev" },
                            span { class: "dot" }
                            span { class: "ev-reason mono", "{event.reason}" }
                            if event.count > 1 {
                                span { class: "ev-count muted mono", "x{event.count}" }
                            }
                            span { class: "ev-msg", "{event.message}" }
                            span { class: "muted mono",
                                if event.span.is_empty() {
                                    "{event.age}"
                                } else {
                                    "{event.span} / {event.age}"
                                }
                            }
                        }
                    }
                }
            },
        }
    }
}

/// Renders a page component to a full HTML document, patching the icon sprite's
/// hrefs the same way `openkite-ui`'s `dump_*` examples do.
fn html_document(title: &str, body_inner: String) -> String {
    let mut body = body_inner;
    body = body.replace(&format!("{UI_CRATE_DIR}/assets/icons.svg#"), "#");
    body = body.replace(&format!("{UI_CRATE_DIR}#"), "#");
    format!(
        "<!doctype html>\n<html lang=\"en\" data-appearance=\"dark\" data-theme=\"frost-dark\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n<link rel=\"stylesheet\" href=\"/styles/console.css\">\n<link rel=\"stylesheet\" href=\"/styles/charts.css\">\n</head>\n<body>\n{ICONS_SVG}\n{body}\n</body>\n</html>\n"
    )
}

fn render_overview(data: OverviewData, connection: Connection, cluster: String) -> String {
    let mut dom = VirtualDom::new_with_props(
        OverviewPage,
        OverviewPageProps {
            data,
            cluster,
            connection,
        },
    );
    dom.rebuild_in_place();
    html_document("openkite · Overview", dioxus_ssr::render(&dom))
}

fn render_table(
    rows: Vec<Row>,
    connection: Connection,
    cluster: String,
    permissions: openkite_ui::access::Permissions,
) -> String {
    let mut dom = VirtualDom::new_with_props(
        TablePage,
        TablePageProps {
            rows,
            cluster,
            connection,
            permissions,
        },
    );
    dom.rebuild_in_place();
    html_document("openkite · Pods", dioxus_ssr::render(&dom))
}

fn render_events(
    rows: Vec<openkite_ui::inspector::Event>,
    connection: Connection,
    cluster: String,
    object_uid: Option<String>,
) -> String {
    let mut dom = VirtualDom::new_with_props(
        EventsPage,
        EventsPageProps {
            rows,
            cluster,
            connection,
            object_uid,
        },
    );
    dom.rebuild_in_place();
    html_document("openkite · Events", dioxus_ssr::render(&dom))
}

fn not_found_html() -> String {
    "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><title>Not found</title></head><body><main><h1>Not found</h1><p><a href=\"/\">Back to the console</a></p></main></body></html>\n".to_owned()
}
