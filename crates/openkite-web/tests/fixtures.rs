//! The fixture-backed shell renders the seeded cluster: a preview with no cluster
//! credential shows the same objects the checks assert on.

#![cfg(feature = "fixtures")]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use openkite_web::{fixtures, router, Live};
use tower::ServiceExt;

/// Fetches a route from a router built over `live` and returns status plus body.
async fn fetch(live: Arc<Live>, uri: &str) -> (StatusCode, String) {
    let response = router(live)
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .expect("router answers");
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn seeded_pods_reach_the_table() {
    let live = Arc::new(fixtures::connect().await);
    assert_eq!(live.cluster(), fixtures::CONTEXT);

    let (status, body) = fetch(live, "/table").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("api-7d9f-abcde"),
        "seeded pod missing from the table"
    );
}

#[tokio::test]
async fn overview_counts_the_seeded_cluster() {
    let live = Arc::new(fixtures::connect().await);

    let (status, body) = fetch(live, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(fixtures::CONTEXT), "cluster label missing");
    assert!(
        !body.contains("unavailable"),
        "fixtures must not read as an empty console"
    );
}
