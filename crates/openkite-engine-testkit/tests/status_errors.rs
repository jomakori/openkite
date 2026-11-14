//! `Status` bodies: 404 for an unserved resource, 415 for an unserved media type, 501 for watch.

mod common;

use kube::api::{Api, ListParams};
use kube::core::{ApiResource, DynamicObject, GroupVersionKind};
use kube::Error;

#[tokio::test]
async fn an_unserved_resource_is_a_404_status() {
    let (server, _) = common::with_pods(1).await;
    let resource =
        ApiResource::from_gvk_with_plural(&GroupVersionKind::gvk("", "v1", "Widget"), "widgets");
    let api: Api<DynamicObject> = Api::all_with(server.client(), &resource);

    let error = api
        .list(&ListParams::default())
        .await
        .expect_err("unknown resource must fail");
    match error {
        Error::Api(status) => {
            assert_eq!(status.code, 404);
            assert_eq!(status.reason, "NotFound");
            assert!(status.message.contains("widgets"));
        }
        other => panic!("expected an Api error, got {other}"),
    }

    server.shutdown().await;
}

#[tokio::test]
async fn an_unsupported_accept_header_is_a_415_status() {
    let (server, _) = common::with_pods(1).await;

    let response = common::raw_get(server.base_url(), "/api/v1/pods", "text/plain")
        .await
        .expect("raw request");

    assert!(response.starts_with("HTTP/1.1 415"), "response: {response}");
    assert!(response.contains("UnsupportedMediaType"));
    server.shutdown().await;
}

#[tokio::test]
async fn a_watch_request_streams_instead_of_refusing() {
    let (server, _) = common::with_pods(1).await;

    let response = common::raw_get(
        server.base_url(),
        "/api/v1/pods?watch=true&timeoutSeconds=1",
        "application/json",
    )
    .await
    .expect("raw request");

    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");
    server.shutdown().await;
}
