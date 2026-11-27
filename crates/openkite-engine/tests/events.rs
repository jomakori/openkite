//! Acceptance tests for the events list and its aggregation policy.
use std::time::Duration;

use k8s_openapi::api::core::v1::{Event, ObjectReference};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, ObjectMeta, Time};
use k8s_openapi::jiff::Timestamp;
use openkite_engine::coalesce::{self, CoalesceInput};
use openkite_engine::events::{
    aggregate_rows, field_selector_for_object, list_for_object, EventAggregator, AGGREGATION_WINDOW,
};
use openkite_engine::interest::InterestClass;
use openkite_engine::projection::{Row, RowStatus};
use openkite_engine_testkit::{FakeApiserver, ResourceDef, Scenario};
use serde_json::json;

const POD_UID: &str = "0f6d1f4e-9a1a-4a6f-9f47-2c1f9c9e51aa";
const MESSAGE: &str = "Back-off restarting failed container api in pod api-7d9c4f8b6-2xqzt_demo";
const BASE_MILLIS: i64 = 1_767_520_320_000; // 2026-01-04T08:32:00Z

fn stamp(millis: i64) -> Time {
    Time(Timestamp::from_millisecond(millis).expect("valid timestamp"))
}

fn micro(millis: i64) -> MicroTime {
    MicroTime(Timestamp::from_millisecond(millis).expect("valid timestamp"))
}

fn event(uid: &str, reason: &str, message: &str, count: Option<i32>, at: i64) -> Event {
    Event {
        metadata: ObjectMeta {
            name: Some(format!("api-7d9c4f8b6-2xqzt.{uid}")),
            namespace: Some("demo".to_owned()),
            uid: Some(uid.to_owned()),
            ..ObjectMeta::default()
        },
        involved_object: ObjectReference {
            api_version: Some("v1".to_owned()),
            kind: Some("Pod".to_owned()),
            name: Some("api-7d9c4f8b6-2xqzt".to_owned()),
            namespace: Some("demo".to_owned()),
            uid: Some(POD_UID.to_owned()),
            ..ObjectReference::default()
        },
        type_: Some("Warning".to_owned()),
        reason: Some(reason.to_owned()),
        message: Some(message.to_owned()),
        count,
        first_timestamp: Some(stamp(at)),
        last_timestamp: Some(stamp(at)),
        ..Event::default()
    }
}

fn count_of(row: &Row) -> i32 {
    match &row.status {
        RowStatus::Event(status) => status.count.expect("aggregated rows carry a count"),
        other => panic!("expected an event row, got {other:?}"),
    }
}

#[test]
fn burst_of_5000_events_collapses_to_one_row() {
    let mut aggregator = EventAggregator::new();
    let now = BASE_MILLIS as u64;
    for index in 0..5_000 {
        let uid = format!("ev-{index}");
        aggregator.observe(&event(&uid, "BackOff", MESSAGE, Some(1), BASE_MILLIS), now);
    }

    let rows = aggregator.rows();
    assert_eq!(rows.len(), 1, "one cause, one row");
    assert_eq!(count_of(&rows[0]), 5_000, "every occurrence is counted");
    let RowStatus::Event(status) = &rows[0].status else {
        panic!("expected an event row");
    };
    assert_eq!(status.reason.as_deref(), Some("BackOff"));
    assert_eq!(status.involved_uid.as_deref(), Some(POD_UID));
    assert_eq!(status.first_seen, Some(BASE_MILLIS as u64));
    assert_eq!(status.last_seen, Some(BASE_MILLIS as u64));
}

#[test]
fn a_correlated_event_is_not_counted_twice() {
    let mut aggregator = EventAggregator::new();
    let now = BASE_MILLIS as u64;
    for count in 1..=3 {
        aggregator.observe(
            &event(
                "ev-correlated",
                "BackOff",
                MESSAGE,
                Some(count),
                BASE_MILLIS,
            ),
            now,
        );
    }

    let rows = aggregator.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        count_of(&rows[0]),
        3,
        "the event object's own count is the total, not a per-observation increment"
    );
}

#[test]
fn series_count_and_last_observed_time_win_over_count() {
    let mut aggregator = EventAggregator::new();
    let mut correlated = event("ev-series", "BackOff", MESSAGE, Some(2), BASE_MILLIS);
    correlated.series = Some(k8s_openapi::api::core::v1::EventSeries {
        count: Some(9),
        last_observed_time: Some(micro(BASE_MILLIS + 60_000)),
    });

    let row = aggregator.observe(&correlated, (BASE_MILLIS + 60_000) as u64);
    assert_eq!(count_of(&row), 9);
    let RowStatus::Event(status) = &row.status else {
        panic!("expected an event row");
    };
    assert_eq!(status.last_seen, Some((BASE_MILLIS + 60_000) as u64));
}

#[test]
fn different_reasons_messages_or_objects_stay_separate() {
    let mut aggregator = EventAggregator::new();
    let now = BASE_MILLIS as u64;
    aggregator.observe(&event("a", "BackOff", MESSAGE, Some(1), BASE_MILLIS), now);
    aggregator.observe(
        &event("b", "FailedScheduling", MESSAGE, Some(1), BASE_MILLIS),
        now,
    );
    aggregator.observe(
        &event("c", "BackOff", "other message", Some(1), BASE_MILLIS),
        now,
    );
    let mut elsewhere = event("d", "BackOff", MESSAGE, Some(1), BASE_MILLIS);
    elsewhere.involved_object.uid = Some("another-uid".to_owned());
    aggregator.observe(&elsewhere, now);

    assert_eq!(aggregator.rows().len(), 4, "four distinct causes");
}

#[test]
fn an_object_without_a_uid_is_keyed_by_kind_namespace_name() {
    let mut aggregator = EventAggregator::new();
    let now = BASE_MILLIS as u64;
    let mut without_uid = event("a", "BackOff", MESSAGE, Some(1), BASE_MILLIS);
    without_uid.involved_object.uid = None;
    aggregator.observe(&without_uid, now);
    let mut second = event("b", "BackOff", MESSAGE, Some(1), BASE_MILLIS);
    second.involved_object.uid = None;
    aggregator.observe(&second, now);

    let rows = aggregator.rows();
    assert_eq!(rows.len(), 1, "the fallback identity still de-duplicates");
    assert_eq!(count_of(&rows[0]), 2);
}

#[test]
fn a_cause_quiet_for_the_window_starts_a_new_row() {
    let mut aggregator = EventAggregator::with_window(Duration::from_secs(60));
    let first = aggregator.observe(
        &event("a", "BackOff", MESSAGE, Some(4), BASE_MILLIS),
        BASE_MILLIS as u64,
    );
    let first_uid = first.meta.uid.clone();

    assert!(
        aggregator.expire(BASE_MILLIS as u64 + 60_000).is_empty(),
        "a row stays open inside the window"
    );
    assert_eq!(
        aggregator.expire(BASE_MILLIS as u64 + 60_001),
        vec![first_uid.clone()],
        "the window closes the row"
    );

    let restarted = aggregator.observe(
        &event("b", "BackOff", MESSAGE, Some(1), BASE_MILLIS + 61_000),
        (BASE_MILLIS + 61_000) as u64,
    );
    assert_ne!(
        restarted.meta.uid, first_uid,
        "a new row, not a revived count"
    );
    assert_eq!(count_of(&restarted), 1);
    let RowStatus::Event(status) = &restarted.status else {
        panic!("expected an event row");
    };
    assert_eq!(status.first_seen, Some((BASE_MILLIS + 61_000) as u64));
}

#[test]
fn the_default_window_is_fifteen_minutes() {
    let mut aggregator = EventAggregator::new();
    let now = BASE_MILLIS as u64;
    aggregator.observe(&event("a", "BackOff", MESSAGE, Some(1), BASE_MILLIS), now);
    assert!(aggregator
        .expire(now + AGGREGATION_WINDOW.as_millis() as u64)
        .is_empty());
    assert_eq!(
        aggregator
            .expire(now + AGGREGATION_WINDOW.as_millis() as u64 + 1)
            .len(),
        1
    );
}

#[test]
fn rows_are_newest_first() {
    let mut aggregator = EventAggregator::new();
    let now = BASE_MILLIS as u64 + 120_000;
    aggregator.observe(&event("a", "BackOff", "older", Some(1), BASE_MILLIS), now);
    aggregator.observe(
        &event(
            "b",
            "FailedScheduling",
            "newer",
            Some(1),
            BASE_MILLIS + 60_000,
        ),
        now,
    );

    let rows = aggregator.rows();
    let RowStatus::Event(first) = &rows[0].status else {
        panic!("expected an event row");
    };
    assert_eq!(first.message.as_deref(), Some("newer"));
}

#[test]
fn aggregate_rows_keeps_a_capped_number_of_sources() {
    let mut aggregator = EventAggregator::new();
    let now = BASE_MILLIS as u64;
    for index in 0..600 {
        let uid = format!("ev-{index}");
        aggregator.observe(&event(&uid, "BackOff", MESSAGE, Some(2), BASE_MILLIS), now);
    }

    let rows = aggregator.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        count_of(&rows[0]),
        1_200,
        "folded sources keep their counts"
    );
}

#[test]
fn aggregate_rows_collapses_a_list() {
    let events: Vec<Event> = (0..10)
        .map(|index| {
            event(
                &format!("ev-{index}"),
                "BackOff",
                MESSAGE,
                Some(1),
                BASE_MILLIS,
            )
        })
        .collect();
    let rows = aggregate_rows(&events, BASE_MILLIS as u64);
    assert_eq!(rows.len(), 1);
    assert_eq!(count_of(&rows[0]), 10);
}

#[test]
fn the_selector_names_the_involved_object_uid() {
    assert_eq!(
        field_selector_for_object(POD_UID),
        format!("involvedObject.uid={POD_UID}")
    );
}

fn events_def() -> ResourceDef {
    ResourceDef::new("", "v1", "Event", "events", true)
}

#[tokio::test]
async fn per_object_list_returns_only_that_objects_events() {
    let events = events_def();
    let mine = serde_json::to_value(event("ev-1", "BackOff", MESSAGE, Some(1), BASE_MILLIS))
        .expect("event json");
    let mut other = event("ev-2", "BackOff", MESSAGE, Some(1), BASE_MILLIS);
    other.involved_object.uid = Some("a-different-uid".to_owned());
    other.metadata.name = Some("other".to_owned());
    let other = serde_json::to_value(other).expect("event json");

    let scenario = Scenario::new()
        .with_resource(events.clone())
        .add(&events, mine)
        .add(&events, other);
    let server = FakeApiserver::start(scenario).await;

    let listed = list_for_object(&server.client(), POD_UID)
        .await
        .expect("list events for the object");
    assert_eq!(listed.len(), 1, "the field selector scopes the list");
    assert_eq!(listed[0].metadata.uid.as_deref(), Some("ev-1"));
}

#[tokio::test]
async fn the_frame_stream_carries_one_aggregated_row_per_cause() {
    let live = openkite_engine::events::now_millis() as i64;
    let (input, mut frames) = coalesce::spawn(InterestClass::Visible);
    for index in 0..5_000 {
        let raw = serde_json::to_value(event(
            &format!("ev-{index}"),
            "BackOff",
            MESSAGE,
            Some(1),
            live,
        ))
        .expect("event json");
        input.push(CoalesceInput::Event(Box::new(
            serde_json::from_value(raw).expect("event"),
        )));
    }

    tokio::time::sleep(Duration::from_millis(400)).await;
    drop(input);

    let mut delivered: std::collections::HashMap<String, Row> = std::collections::HashMap::new();
    let mut frame_count = 0;
    while let Ok(frame) = frames.try_recv() {
        frame_count += 1;
        assert!(!frame.reset, "a burst of rows does not force a reset");
        for row in frame.upsert {
            delivered.insert(row.meta.uid.clone(), row);
        }
    }

    assert!(
        (1..=200).contains(&frame_count),
        "5000 events coalesce into a bounded number of frames, saw {frame_count}"
    );
    assert_eq!(delivered.len(), 1, "one cause, one row");
    let row = delivered.values().next().expect("one row");
    assert_eq!(count_of(row), 5_000);
}

#[test]
fn an_unparsable_event_does_not_poison_the_aggregator() {
    let mut aggregator = EventAggregator::new();
    assert!(aggregator
        .observe_value(&json!({"kind": "Event", "metadata": {"name": 7}}), 1)
        .is_none());
    assert!(aggregator.is_empty());
}
