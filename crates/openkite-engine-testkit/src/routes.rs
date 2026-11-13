//! HTTP surface of the fake apiserver.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::discovery;
use crate::status;
use crate::store::{list_meta, Store};
use crate::ResourceDef;

/// Builds the router serving discovery, list and Status responses.
pub(crate) fn router(store: Store) -> Router {
    let state = Arc::new(store);
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
        .with_state(state)
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
        return status::watch_not_served();
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
