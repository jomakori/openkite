//! In-process route tests against `Live::unavailable()` — no kube creds needed.

use std::sync::Arc;

use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use openkite_web::{router, Live};
use tower::ServiceExt;

fn test_router() -> axum::Router {
    router(Arc::new(Live::unavailable()))
}

#[tokio::test]
async fn healthz_reports_ok() {
    let response = test_router()
        .oneshot(
            Request::get("/healthz")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"ok");
}

#[tokio::test]
async fn overview_renders_the_console_shell() {
    let response = test_router()
        .oneshot(Request::get("/").body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("console.css"));
    assert!(html.contains("frost-dark"));
    assert!(html.contains("Cluster overview"));
}

#[tokio::test]
async fn table_renders_the_console_shell() {
    let response = test_router()
        .oneshot(
            Request::get("/table")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("console.css"));
    assert!(html.contains("frost-dark"));
    assert!(html.contains("Pods"));
}

#[tokio::test]
async fn styles_serve_the_console_stylesheet() {
    let response = test_router()
        .oneshot(
            Request::get("/styles/console.css")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/css"
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let css = String::from_utf8(body.to_vec()).unwrap();
    assert!(css.contains("--shadow-terminal"));
}

#[tokio::test]
async fn unknown_paths_404() {
    let response = test_router()
        .oneshot(
            Request::get("/nope")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        html.contains("href=\"/\""),
        "404 page should link home: {html}"
    );
}

#[tokio::test]
async fn events_renders_the_console_shell() {
    let response = test_router()
        .oneshot(
            Request::get("/events")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("console.css"), "{body}");
    assert!(body.contains("frost-dark"), "{body}");
    assert!(body.contains("Events"), "{body}");
    assert!(
        body.contains("No events in the last 15 minutes"),
        "the empty state states the aggregation window: {body}"
    );
}
