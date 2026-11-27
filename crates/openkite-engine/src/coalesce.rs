//! Coalesces informer events into bounded, backpressure-safe delta frames.
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use k8s_openapi::api::core::v1::Event;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

use crate::events::{now_millis, EventAggregator};
use crate::informer::InformerEvent;
use crate::interest::InterestClass;
use crate::projection::Row;

/// Bound on buffered events between a watcher and its coalescer; overflow never grows this.
pub const CHANNEL_CAPACITY: usize = 8192;

/// Bound on frames buffered between a coalescer and its frame consumer.
const FRAME_CHANNEL_CAPACITY: usize = 64;

/// One projected update entering the coalescer: a reset, an upserted row, a raw event to
/// aggregate, or a delete by uid.
#[derive(Debug, Clone)]
pub enum CoalesceInput {
    Reset,
    Upsert(Box<Row>),
    /// A raw Event, aggregated by reason/object/message before it reaches a frame.
    Event(Box<Event>),
    Delete(String),
}

/// Upsert/delete counts carried on a frame.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct FrameCounts {
    pub upserts: usize,
    pub deletes: usize,
}

/// Backpressure health reported alongside a frame.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct FrameHealth {
    pub overflowed: bool,
}

/// One emitted delta: a reset snapshot or the dirty subset accumulated since the last frame.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Frame {
    pub seq: u64,
    pub generated_at: u64,
    pub reset: bool,
    pub upsert: Vec<Row>,
    pub delete: Vec<String>,
    pub counts: FrameCounts,
    pub health: FrameHealth,
}

/// Producer-side handle: enqueues events on the bounded channel, never blocking or growing it.
#[derive(Clone)]
pub struct CoalescerInput {
    tx: mpsc::Sender<CoalesceInput>,
    dirty_all: Arc<AtomicBool>,
}

impl CoalescerInput {
    /// Builds a fresh bounded channel of capacity `CHANNEL_CAPACITY` with no consumer attached.
    pub fn bounded() -> (Self, mpsc::Receiver<CoalesceInput>) {
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        let input = Self {
            tx,
            dirty_all: Arc::new(AtomicBool::new(false)),
        };
        (input, rx)
    }

    /// Enqueues an event; on a full channel this sets the dirty-all flag and drops it instead.
    pub fn push(&self, input: CoalesceInput) {
        if self.tx.try_send(input).is_err() {
            self.dirty_all.store(true, Ordering::SeqCst);
        }
    }

    /// Whether an overflow has set the dirty-all flag since the last frame drained it.
    pub fn is_overflowed(&self) -> bool {
        self.dirty_all.load(Ordering::SeqCst)
    }
}

/// Projects a raw informer event into coalescer input and forwards it, never blocking.
pub fn forward(
    event: InformerEvent,
    project: impl Fn(&Value) -> Option<Row>,
    input: &CoalescerInput,
) {
    match event {
        InformerEvent::Reset => input.push(CoalesceInput::Reset),
        InformerEvent::Upsert(value) => {
            if let Some(row) = project(&value) {
                input.push(CoalesceInput::Upsert(Box::new(row)));
            }
        }
        InformerEvent::Delete(uid) => input.push(CoalesceInput::Delete(uid)),
    }
}

/// Forwards a raw Event watch event for aggregation, never blocking. Unparsable objects are
/// dropped with a debug log rather than failing the subscription.
pub fn forward_event(event: InformerEvent, input: &CoalescerInput) {
    match event {
        InformerEvent::Reset => input.push(CoalesceInput::Reset),
        InformerEvent::Upsert(value) => match serde_json::from_value::<Event>(value) {
            Ok(event) => input.push(CoalesceInput::Event(Box::new(event))),
            Err(error) => tracing::debug!(%error, "skipping unparsable event"),
        },
        InformerEvent::Delete(uid) => tracing::trace!(uid, "ignoring raw event deletion"),
    }
}

/// Spawns the coalescing loop for one subscription class; returns the input handle and frame stream.
pub fn spawn(class: InterestClass) -> (CoalescerInput, mpsc::Receiver<Frame>) {
    let (input, rx) = CoalescerInput::bounded();
    let dirty_all = Arc::clone(&input.dirty_all);
    let (frame_tx, frame_rx) = mpsc::channel(FRAME_CHANNEL_CAPACITY);
    tokio::spawn(run(class, rx, dirty_all, frame_tx));
    (input, frame_rx)
}

/// Accumulates dirty rows per interval and emits one frame per tick when there's something to say.
#[derive(Default)]
struct CoalesceState {
    seq: u64,
    reset: bool,
    rows: HashMap<String, Row>,
    deletes: HashSet<String>,
    /// Raw events collapse here before they ever reach a frame.
    events: EventAggregator,
}

impl CoalesceState {
    fn apply(&mut self, input: CoalesceInput) {
        match input {
            CoalesceInput::Reset => {
                self.reset = true;
                self.rows.clear();
                self.deletes.clear();
            }
            CoalesceInput::Upsert(row) => {
                let uid = row.meta.uid.clone();
                self.deletes.remove(&uid);
                self.rows.insert(uid, *row);
            }
            CoalesceInput::Event(event) => {
                let row = self.events.observe(&event, now_millis());
                let uid = row.meta.uid.clone();
                self.deletes.remove(&uid);
                self.rows.insert(uid, row);
            }
            CoalesceInput::Delete(uid) => {
                self.rows.remove(&uid);
                self.deletes.insert(uid);
            }
        }
    }

    /// Builds the pending frame, if any, and clears the accumulated dirty state.
    fn drain_frame(&mut self, dirty_all: &AtomicBool) -> Option<Frame> {
        let overflowed = dirty_all.swap(false, Ordering::SeqCst);
        let reset = self.reset || overflowed;

        if reset {
            self.reset = false;
            self.rows.clear();
            self.deletes.clear();
            self.seq += 1;
            return Some(Frame {
                seq: self.seq,
                generated_at: now_millis(),
                reset: true,
                upsert: Vec::new(),
                delete: Vec::new(),
                counts: FrameCounts::default(),
                health: FrameHealth { overflowed },
            });
        }

        for uid in self.events.expire(now_millis()) {
            self.rows.remove(&uid);
            self.deletes.insert(uid);
        }

        if self.rows.is_empty() && self.deletes.is_empty() {
            return None;
        }

        let upsert: Vec<Row> = self.rows.drain().map(|(_, row)| row).collect();
        let delete: Vec<String> = self.deletes.drain().collect();
        let counts = FrameCounts {
            upserts: upsert.len(),
            deletes: delete.len(),
        };
        self.seq += 1;
        Some(Frame {
            seq: self.seq,
            generated_at: now_millis(),
            reset: false,
            upsert,
            delete,
            counts,
            health: FrameHealth { overflowed: false },
        })
    }
}

async fn run(
    class: InterestClass,
    mut rx: mpsc::Receiver<CoalesceInput>,
    dirty_all: Arc<AtomicBool>,
    frame_tx: mpsc::Sender<Frame>,
) {
    let mut state = CoalesceState::default();
    let mut ticker = tokio::time::interval(class.coalesce_interval());
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(input) => state.apply(input),
                    None => break,
                }
            }
            _ = ticker.tick() => {
                if let Some(frame) = state.drain_frame(&dirty_all) {
                    if frame_tx.send(frame).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}
