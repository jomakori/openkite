//! Access reviews answered by the fake apiserver.

use openkite_engine::access::{Gate, Review, Verb};
use openkite_engine_testkit::{FakeApiserver, Scenario};

#[tokio::test]
async fn allowed_and_denied_verbs_come_from_the_rule_matrix() {
    let server = FakeApiserver::start(Scenario::demo()).await;
    server.allow_access("list", "", "pods", Some("demo"));
    server.allow_access("get", "", "pods", Some("demo"));
    server.deny_access(
        "delete",
        "",
        "pods",
        Some("demo"),
        "pods \"web\" is forbidden: User cannot delete resource",
    );
    let client = server.client();
    let gate = Gate::with_default_ttl();

    let capabilities = gate
        .capabilities(&client, "", "pods", Some("demo"))
        .await
        .expect("review pods");

    assert!(capabilities.can_list());
    assert!(capabilities.can_get());
    assert!(!capabilities.can_delete());
    assert!(!capabilities.can_create());
    assert!(capabilities
        .reason(Verb::Delete)
        .expect("denial reason")
        .contains("cannot delete"));

    server.shutdown().await;
}

#[tokio::test]
async fn a_repeated_question_is_answered_from_the_cache() {
    let server = FakeApiserver::start(Scenario::demo()).await;
    server.allow_access("list", "apps", "deployments", None);
    let client = server.client();
    let gate = Gate::with_default_ttl();
    let review = Review::new(Verb::List, "apps", "deployments");

    assert!(gate.allows(&client, &review).await.expect("first review"));
    assert!(gate.allows(&client, &review).await.expect("cached review"));
    assert_eq!(server.access_review_count(), 1);

    gate.invalidate();
    assert!(gate.allows(&client, &review).await.expect("fresh review"));
    assert_eq!(server.access_review_count(), 2);

    server.shutdown().await;
}

#[tokio::test]
async fn a_namespaced_rule_does_not_answer_another_namespace() {
    let server = FakeApiserver::start(Scenario::demo()).await;
    server.allow_access("list", "", "pods", Some("demo"));
    let client = server.client();
    let gate = Gate::with_default_ttl();

    let other = Review::new(Verb::List, "", "pods").in_namespace("production");
    assert!(!gate.allows(&client, &other).await.expect("review"));

    server.shutdown().await;
}
