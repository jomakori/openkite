//! Acceptance tests for the coalescer: frame budget, latency, and bounded backpressure.
use std::collections::HashMap;
use std::time::{Duration, Instant};

use openkite_engine::coalesce::{self, CoalesceInput, CoalescerInput, Frame, CHANNEL_CAPACITY};
use openkite_engine::interest::InterestClass;
use openkite_engine::projection::{Row, RowMeta, RowStatus};

fn row(uid: &str) -> Row {
    Row {
        meta: RowMeta {
            uid: uid.to_owned(),
            name: uid.to_owned(),
            namespace: None,
            kind: "Pod".to_owned(),
            created: None,
            labels_hash: 0,
            resource_version: None,
        },
        status: RowStatus::Metadata,
    }
}

/// Pushes `event_count` unique upserts as fast as possible and collects every frame
/// emitted in response, pairing each surfaced row with its own push-to-frame latency.
async fn run_burst(event_count: usize) -> (Vec<Frame>, Vec<Duration>) {
    let (input, mut frames) = coalesce::spawn(InterestClass::Visible);
    let mut pushed_at = HashMap::with_capacity(event_count);
    for index in 0..event_count {
        let uid = format!("uid-{index}");
        pushed_at.insert(uid.clone(), Instant::now());
        input.push(CoalesceInput::Upsert(Box::new(row(&uid))));
    }

    let mut collected_frames = Vec::new();
    let mut latencies = Vec::new();
    let mut seen = 0usize;
    let deadline = Instant::now() + Duration::from_secs(5);
    while seen < event_count && Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), frames.recv()).await {
            Ok(Some(frame)) => {
                let received_at = Instant::now();
                for frame_row in &frame.upsert {
                    if let Some(sent_at) = pushed_at.get(&frame_row.meta.uid) {
                        latencies.push(received_at.duration_since(*sent_at));
                        seen += 1;
                    }
                }
                collected_frames.push(frame);
            }
            _ => break,
        }
    }
    (collected_frames, latencies)
}

#[tokio::test]
async fn burst_of_5000_events_produces_at_most_200_frames() {
    let (frames, _latencies) = run_burst(5_000).await;
    assert!(
        frames.len() <= 200,
        "expected at most 200 frames, got {}",
        frames.len()
    );
}

#[tokio::test]
async fn event_to_frame_p99_latency_is_bounded() {
    let (_frames, mut latencies) = run_burst(5_000).await;
    assert_eq!(
        latencies.len(),
        5_000,
        "every pushed event must surface in exactly one frame"
    );
    latencies.sort();
    let p99_index = ((latencies.len() as f64) * 0.99).ceil() as usize;
    let p99 = latencies[p99_index.min(latencies.len() - 1).max(0)];
    assert!(
        p99 <= Duration::from_millis(300),
        "p99 event-to-frame latency {p99:?} exceeds 300ms"
    );
}

#[test]
fn overflow_at_capacity_sets_dirty_all_without_growing_the_queue() {
    let (input, mut rx) = CoalescerInput::bounded();
    for index in 0..CHANNEL_CAPACITY {
        input.push(CoalesceInput::Delete(format!("uid-{index}")));
    }
    assert!(
        !input.is_overflowed(),
        "channel exactly at capacity has not overflowed yet"
    );

    input.push(CoalesceInput::Delete("one-too-many".to_owned()));
    assert!(
        input.is_overflowed(),
        "a push beyond capacity must set dirty_all instead of growing the queue"
    );

    let mut drained = 0usize;
    while rx.try_recv().is_ok() {
        drained += 1;
    }
    assert_eq!(
        drained, CHANNEL_CAPACITY,
        "a bounded channel yields exactly its capacity, never more"
    );
}

#[tokio::test]
async fn overflow_forces_the_next_frame_to_be_a_reset() {
    let (input, mut frames) = coalesce::spawn(InterestClass::Pinned);
    for index in 0..(CHANNEL_CAPACITY + 1_000) {
        input.push(CoalesceInput::Upsert(Box::new(row(&format!(
            "uid-{index}"
        )))));
    }
    assert!(
        input.is_overflowed(),
        "a burst beyond capacity must set dirty_all"
    );

    let frame = tokio::time::timeout(Duration::from_secs(3), frames.recv())
        .await
        .expect("a frame must arrive within the class's coalesce interval")
        .expect("the frame channel must not close");
    assert!(
        frame.reset,
        "overflow must force the next frame to be a reset"
    );
    assert!(
        frame.health.overflowed,
        "the reset frame must report the overflow in its health"
    );
}
