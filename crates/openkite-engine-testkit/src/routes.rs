//! HTTP surface of the fake apiserver.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use futures_util::stream;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::time::Instant;

use crate::discovery;
use crate::status;
use crate::store::{list_meta, Store, WatchRecord};
use crate::{Event, ResourceDef};

/// Builds the router serving discovery, list, watch and Status responses.
pub(crate) fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/api", get(api_root))
        .route("/apis", get(apis_root))
        .route("/api/{version}", get(core_resources))
        .route("/api/{version}/{plural}", get(list_core))
        .route(
            "/api/{version}/namespaces/{namespace}/{plural}",
            get(list_core_namespaced),
        )
        .route("/apis/{group}/{version}", get(group_resources))
        .route("/apis/{group}/{version}/{plural}", get(list_group))
        .route(
            "/apis/{group}/{version}/namespaces/{namespace}/{plural}",
            get(list_group_namespaced),
        )
        .with_state(store)
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ListQuery {
    limit: Option<u32>,
    #[serde(rename = "continue")]
    continue_token: Option<String>,
    #[serde(rename = "fieldSelector")]
    field_selector: Option<String>,
    #[serde(rename = "labelSelector")]
    label_selector: Option<String>,
    watch: Option<bool>,
    #[serde(rename = "resourceVersion")]
    resource_version: Option<String>,
    #[serde(rename = "timeoutSeconds")]
    timeout_seconds: Option<u64>,
    #[serde(rename = "allowWatchBookmarks")]
    allow_watch_bookmarks: Option<bool>,
}

async fn api_root(State(store): State<Arc<Store>>, headers: HeaderMap) -> Response {
    if wants_aggregated(&headers) {
        Json(discovery::aggregated_core(&store)).into_response()
    } else {
        Json(discovery::api_versions(&store)).into_response()
    }
}

async fn apis_root(State(store): State<Arc<Store>>, headers: HeaderMap) -> Response {
    if wants_aggregated(&headers) {
        Json(discovery::aggregated(&store)).into_response()
    } else {
        Json(discovery::group_list(&store)).into_response()
    }
}

async fn core_resources(State(store): State<Arc<Store>>, Path(version): Path<String>) -> Response {
    if store
        .group_versions("")
        .iter()
        .all(|def| def.version != version)
    {
        return status::not_found("", &version, "");
    }
    Json(discovery::resource_list(&store, "", &version)).into_response()
}

async fn group_resources(
    State(store): State<Arc<Store>>,
    Path((group, version)): Path<(String, String)>,
) -> Response {
    if store
        .group_versions(&group)
        .iter()
        .all(|def| def.version != version)
    {
        return status::not_found(&group, &version, "");
    }
    Json(discovery::resource_list(&store, &group, &version)).into_response()
}

async fn list_core(
    State(store): State<Arc<Store>>,
    Path((version, plural)): Path<(String, String)>,
    headers: HeaderMap,
    query: Query<ListQuery>,
) -> Response {
    list(&store, "", &version, &plural, None, &headers, &query)
}

async fn list_core_namespaced(
    State(store): State<Arc<Store>>,
    Path((version, namespace, plural)): Path<(String, String, String)>,
    headers: HeaderMap,
    query: Query<ListQuery>,
) -> Response {
    list(
        &store,
        "",
        &version,
        &plural,
        Some(&namespace),
        &headers,
        &query,
    )
}

async fn list_group(
    State(store): State<Arc<Store>>,
    Path((group, version, plural)): Path<(String, String, String)>,
    headers: HeaderMap,
    query: Query<ListQuery>,
) -> Response {
    list(&store, &group, &version, &plural, None, &headers, &query)
}

async fn list_group_namespaced(
    State(store): State<Arc<Store>>,
    Path((group, version, namespace, plural)): Path<(String, String, String, String)>,
    headers: HeaderMap,
    query: Query<ListQuery>,
) -> Response {
    list(
        &store,
        &group,
        &version,
        &plural,
        Some(&namespace),
        &headers,
        &query,
    )
}

#[allow(clippy::too_many_arguments)]
fn list(
    store: &Store,
    group: &str,
    version: &str,
    plural: &str,
    namespace: Option<&str>,
    headers: &HeaderMap,
    query: &ListQuery,
) -> Response {
    let Some(def): Option<&ResourceDef> = store.resource(group, version, plural) else {
        return status::not_found(group, version, plural);
    };
    if let Some(accept) = headers.get(header::ACCEPT) {
        let accept = accept.to_str().unwrap_or_default();
        if !accept.contains("json") && !accept.contains('*') {
            return status::unsupported_media_type(accept);
        }
    }
    if query.watch == Some(true) {
        return watch(store, def, namespace, query);
    }

    let mut items: Vec<Value> = store
        .snapshot(def)
        .into_iter()
        .filter(|item| matches_scope(item, def, namespace))
        .filter(|item| matches_field_selector(item, query.field_selector.as_deref()))
        .filter(|item| matches_label_selector(item, query.label_selector.as_deref()))
        .collect();

    let start = query
        .continue_token
        .as_deref()
        .and_then(|token| token.parse::<usize>().ok())
        .unwrap_or(0);
    let limit = query.limit.unwrap_or(500).min(500) as usize;
    items.drain(..start.min(items.len()));
    let remaining = items.len();
    items.truncate(limit);
    let next = (remaining > limit).then(|| (start + limit).to_string());

    let body = json!({
        "kind": format!("{}List", def.kind),
        "apiVersion": api_version(group, version),
        "metadata": list_meta(&store.resource_version(), next.as_deref()),
        "items": items,
    });
    Json(body).into_response()
}

/// Serves the watch stream: logged events newer than the client's resourceVersion, then live ones.
fn watch(store: &Store, def: &ResourceDef, namespace: Option<&str>, query: &ListQuery) -> Response {
    let from = query
        .resource_version
        .as_deref()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    if store.is_stale(from) {
        return status::expired(&def.plural, &store.resource_version());
    }

    let filter = Filter {
        def: def.clone(),
        namespace: namespace.map(str::to_owned),
        field_selector: query.field_selector.clone(),
        label_selector: query.label_selector.clone(),
    };
    let replay = if from == 0 {
        Vec::new()
    } else {
        store.replay(&def.gvk(), from)
    };
    let last_seen = replay.last().map_or(from, |record| record.resource_version);
    let state = WatchState {
        replay: replay.into(),
        events: store.subscribe(),
        last_seen,
        deadline: Instant::now() + Duration::from_secs(query.timeout_seconds.unwrap_or(30)),
        bookmarks: query.allow_watch_bookmarks.unwrap_or(true),
        bookmarked: false,
        closed: false,
        filter,
        current: store.resource_version(),
    };

    let stream = stream::unfold(state, |mut state| async move {
        loop {
            if state.closed {
                return None;
            }
            if let Some(record) = state.replay.pop_front() {
                if state.filter.matches(&record.object) {
                    state.last_seen = record.resource_version;
                    return Some((Ok::<Bytes, Infallible>(record.line()), state));
                }
                continue;
            }
            tokio::select! {
                received = state.events.recv() => match received {
                    Ok(record) if record.resource_version <= state.last_seen => continue,
                    Ok(record) => {
                        if !state.filter.matches(&record.object) {
                            continue;
                        }
                        state.last_seen = record.resource_version;
                        return Some((Ok(record.line()), state));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        state.closed = true;
                        return Some((Ok(error_line(&state.filter.def.plural, &state.current)), state));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                },
                _ = tokio::time::sleep_until(state.deadline) => {
                    state.closed = true;
                    if state.bookmarks && !state.bookmarked {
                        state.bookmarked = true;
                        let line = bookmark_line(&state.filter.def, &state.current);
                        return Some((Ok(line), state));
                    }
                    return None;
                }
            }
        }
    });

    Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from_stream(stream))
        .expect("watch response")
}

struct WatchState {
    replay: VecDeque<WatchRecord>,
    events: tokio::sync::broadcast::Receiver<WatchRecord>,
    last_seen: u64,
    deadline: Instant,
    bookmarks: bool,
    bookmarked: bool,
    closed: bool,
    filter: Filter,
    current: String,
}

#[derive(Clone)]
struct Filter {
    def: ResourceDef,
    namespace: Option<String>,
    field_selector: Option<String>,
    label_selector: Option<String>,
}

impl Filter {
    fn matches(&self, object: &Value) -> bool {
        matches_scope(object, &self.def, self.namespace.as_deref())
            && matches_field_selector(object, self.field_selector.as_deref())
            && matches_label_selector(object, self.label_selector.as_deref())
    }
}

impl WatchRecord {
    fn line(&self) -> Bytes {
        let body = json!({
            "type": match self.event {
                Event::Added => "ADDED",
                Event::Modified => "MODIFIED",
                Event::Deleted => "DELETED",
            },
            "object": self.object,
        });
        Bytes::from(format!("{body}\n"))
    }
}

fn bookmark_line(def: &ResourceDef, resource_version: &str) -> Bytes {
    let body = json!({
        "type": "BOOKMARK",
        "object": {
            "apiVersion": api_version(&def.group, &def.version),
            "kind": def.kind,
            "metadata": { "resourceVersion": resource_version },
        },
    });
    Bytes::from(format!("{body}\n"))
}

fn error_line(plural: &str, resource_version: &str) -> Bytes {
    let body = json!({
        "type": "ERROR",
        "object": status::expired_body(plural, resource_version),
    });
    Bytes::from(format!("{body}\n"))
}

fn matches_scope(item: &Value, def: &ResourceDef, namespace: Option<&str>) -> bool {
    if !def.namespaced {
        return true;
    }
    match namespace {
        Some(namespace) => field(item, "namespace").is_some_and(|value| value == namespace),
        None => true,
    }
}

fn matches_field_selector(item: &Value, selector: Option<&str>) -> bool {
    let Some(selector) = selector else {
        return true;
    };
    split_selector(selector).all(|(key, wanted)| {
        let field = key.strip_prefix("metadata.").unwrap_or(key.as_str());
        field_value(item, field).is_some_and(|value| value == wanted)
    })
}

fn matches_label_selector(item: &Value, selector: Option<&str>) -> bool {
    let Some(selector) = selector else {
        return true;
    };
    split_selector(selector).all(|(key, wanted)| {
        item.pointer(&format!("/metadata/labels/{key}"))
            .and_then(Value::as_str)
            .is_some_and(|value| value == wanted)
    })
}

fn split_selector(selector: &str) -> impl Iterator<Item = (String, &str)> {
    selector.split(',').filter_map(|clause| {
        let (key, value) = clause.split_once('=')?;
        Some((key.trim().to_owned(), value.trim()))
    })
}

fn field_value(item: &Value, key: &str) -> Option<String> {
    match key {
        "name" => field(item, "name"),
        "namespace" => field(item, "namespace"),
        other => item
            .pointer(&format!("/metadata/{other}"))
            .and_then(Value::as_str)
            .map(str::to_owned),
    }
}

fn field(item: &Value, key: &str) -> Option<String> {
    item.pointer(&format!("/metadata/{key}"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn wants_aggregated(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("apidiscovery.k8s.io"))
}

fn api_version(group: &str, version: &str) -> String {
    if group.is_empty() {
        version.to_owned()
    } else {
        format!("{group}/{version}")
    }
}
