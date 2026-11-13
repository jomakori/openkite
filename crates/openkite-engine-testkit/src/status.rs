//! Kubernetes `Status` bodies for the failure responses the client must handle.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// A 404 `Status` naming the resource the client asked for.
pub(crate) fn not_found(group: &str, version: &str, plural: &str) -> Response {
    let group_version = if group.is_empty() {
        version.to_owned()
    } else {
        format!("{group}/{version}")
    };
    let body = json!({
        "kind": "Status",
        "apiVersion": "v1",
        "status": "Failure",
        "message": format!("the server could not find the requested resource {plural}"),
        "reason": "NotFound",
        "details": {
            "name": plural,
            "group": group,
            "kind": plural,
            "groupVersion": group_version,
        },
        "code": 404,
    });
    (StatusCode::NOT_FOUND, Json(body)).into_response()
}

/// A 501 `Status` for a watch request; watch is served by its own slice.
pub(crate) fn watch_not_served() -> Response {
    let body = json!({
        "kind": "Status",
        "apiVersion": "v1",
        "status": "Failure",
        "message": "watch is not served by this fake apiserver slice",
        "reason": "NotImplemented",
        "code": 501,
    });
    (StatusCode::NOT_IMPLEMENTED, Json(body)).into_response()
}

/// A 415 `Status` for an `Accept` header the server cannot satisfy.
pub(crate) fn unsupported_media_type(accept: &str) -> Response {
    let body = json!({
        "kind": "Status",
        "apiVersion": "v1",
        "status": "Failure",
        "message": format!("the server does not support the requested media type {accept}"),
        "reason": "UnsupportedMediaType",
        "code": 415,
    });
    (StatusCode::UNSUPPORTED_MEDIA_TYPE, Json(body)).into_response()
}
