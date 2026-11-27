//! Event list v1: raw Kubernetes Events collapsed by cause inside a 15-minute window.
//!
//! Kubelet and the controller manager re-emit the same Event for every retry, so a
//! crash-looping pod can push thousands of rows a minute. The list shows one row per
//! `(reason, involved object, message)` with a count and a first/last seen span; a cause
//! not observed for [`AGGREGATION_WINDOW`] closes and starts a fresh row.
//!
//! Counts use each Event object's own `count`/`series.count`, keyed by that object's uid,
//! so an Event Kubernetes correlates and increments is never counted twice. Aggregates are
//! held in memory only: a raw Event object's deletion does not retract its count, the
//! window expires the whole row instead.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use k8s_openapi::api::core::v1::{Event, ObjectReference};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, Time};
use k8s_openapi::jiff::Timestamp;
use kube::api::{Api, ListParams};
use kube::Client;

use crate::projection::{EventStatusRow, Row, RowMeta, RowStatus};

/// Sliding window an aggregate stays open; a cause not seen for this long starts a new row.
pub const AGGREGATION_WINDOW: Duration = Duration::from_secs(15 * 60);

/// Upper bound on distinct aggregates held at once; the oldest last-seen entry is evicted.
pub const MAX_AGGREGATES: usize = 2_048;

/// Upper bound on source Event objects tracked per aggregate before their counts are folded.
const MAX_SOURCES: usize = 512;

/// The identity a row is de-duplicated on: same reason, same object, same message.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AggregateKey {
    pub reason: String,
    /// The involved object's uid, or `kind/namespace/name` when the uid is absent.
    pub object: String,
    pub message: String,
}

/// One open aggregate: a cause, its count, and the span it was observed over.
#[derive(Debug, Clone, PartialEq)]
struct Aggregate {
    id: String,
    key: AggregateKey,
    type_: Option<String>,
    object_kind: Option<String>,
    object_name: Option<String>,
    object_namespace: Option<String>,
    object_uid: Option<String>,
    /// Counts folded in from sources beyond [`MAX_SOURCES`].
    folded: i64,
    /// Source Event object uid to the highest count that object reported.
    sources: HashMap<String, i64>,
    first_seen: u64,
    last_seen: u64,
}

impl Aggregate {
    fn count(&self) -> i64 {
        self.folded + self.sources.values().sum::<i64>()
    }
}

/// Aggregates raw Events into at most one row per cause, per [`AGGREGATION_WINDOW`].
#[derive(Debug, Clone)]
pub struct EventAggregator {
    window_millis: u64,
    entries: HashMap<AggregateKey, Aggregate>,
}

impl Default for EventAggregator {
    fn default() -> Self {
        Self::new()
    }
}

impl EventAggregator {
    /// An aggregator with the default 15-minute window.
    pub fn new() -> Self {
        Self::with_window(AGGREGATION_WINDOW)
    }

    /// An aggregator with an explicit window; tests use it to step across the boundary.
    pub fn with_window(window: Duration) -> Self {
        Self {
            window_millis: window.as_millis() as u64,
            entries: HashMap::new(),
        }
    }

    /// Folds one Event in at wall-clock `now` (milliseconds since the epoch) and returns the
    /// updated aggregate row.
    pub fn observe(&mut self, event: &Event, now: u64) -> Row {
        let key = aggregate_key(event);
        let observed_count = event_count(event);
        let source = source_id(event);
        let (first, last) = observed_span(event, now);
        let stale = self
            .entries
            .get(&key)
            .is_some_and(|entry| now.saturating_sub(entry.last_seen) > self.window_millis);
        if stale {
            self.entries.remove(&key);
        }

        let row = {
            let entry = self.entries.entry(key).or_insert_with(|| {
                let key = aggregate_key(event);
                Aggregate {
                    id: row_id(&key, first),
                    key,
                    type_: event.type_.clone(),
                    object_kind: event.involved_object.kind.clone(),
                    object_name: event.involved_object.name.clone(),
                    object_namespace: event.involved_object.namespace.clone(),
                    object_uid: event.involved_object.uid.clone(),
                    folded: 0,
                    sources: HashMap::new(),
                    first_seen: first,
                    last_seen: last,
                }
            });

            entry.first_seen = entry.first_seen.min(first);
            entry.last_seen = entry.last_seen.max(last);
            if entry.type_.is_none() {
                entry.type_ = event.type_.clone();
            }

            match entry.sources.get(&source) {
                Some(previous) => {
                    let raised = observed_count.max(*previous);
                    entry.sources.insert(source, raised);
                }
                None if entry.sources.len() < MAX_SOURCES => {
                    entry.sources.insert(source, observed_count);
                }
                None => entry.folded += observed_count,
            }

            row_for(entry)
        };

        self.enforce_bound(now);
        row
    }

    /// Folds a raw object as the streaming layer received it.
    pub fn observe_value(&mut self, value: &serde_json::Value, now: u64) -> Option<Row> {
        match serde_json::from_value::<Event>(value.clone()) {
            Ok(event) => Some(self.observe(&event, now)),
            Err(error) => {
                tracing::debug!(%error, "skipping unparsable event");
                None
            }
        }
    }

    /// Closes aggregates past the window and returns the row ids that closed.
    pub fn expire(&mut self, now: u64) -> Vec<String> {
        let window = self.window_millis;
        let mut closed = Vec::new();
        self.entries.retain(|_, entry| {
            let open = now.saturating_sub(entry.last_seen) <= window;
            if !open {
                closed.push(entry.id.clone());
            }
            open
        });
        closed
    }

    /// Snapshot of every open aggregate, newest first.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<(u64, Row)> = self
            .entries
            .values()
            .map(|entry| (entry.last_seen, row_for(entry)))
            .collect();
        rows.sort_by_key(|(last_seen, _)| std::cmp::Reverse(*last_seen));
        rows.into_iter().map(|(_, row)| row).collect()
    }

    /// Number of open aggregates.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no aggregate is open.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drops the oldest aggregate once the cap is reached, so a hostile cluster cannot grow
    /// the list without bound.
    fn enforce_bound(&mut self, _now: u64) {
        while self.entries.len() > MAX_AGGREGATES {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_seen)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }
}

/// Collapses a list of Events into aggregated rows, newest first.
pub fn aggregate_rows(events: &[Event], now: u64) -> Vec<Row> {
    let mut aggregator = EventAggregator::new();
    for event in events {
        aggregator.observe(event, now);
    }
    aggregator.rows()
}

/// Field selector that scopes a list to one involved object, for the per-object tab.
pub fn field_selector_for_object(object_uid: &str) -> String {
    format!("involvedObject.uid={object_uid}")
}

/// Lists the Events the apiserver holds for one involved object.
pub async fn list_for_object(client: &Client, object_uid: &str) -> Result<Vec<Event>, kube::Error> {
    let params = ListParams::default().fields(&field_selector_for_object(object_uid));
    Api::<Event>::all(client.clone())
        .list(&params)
        .await
        .map(|list| list.items)
}

/// Identity an Event is aggregated under.
fn aggregate_key(event: &Event) -> AggregateKey {
    AggregateKey {
        reason: event.reason.clone().unwrap_or_default(),
        object: object_identity(&event.involved_object),
        message: event.message.clone().unwrap_or_default(),
    }
}

/// The involved object's uid when present, else its kind/namespace/name.
fn object_identity(reference: &ObjectReference) -> String {
    match reference.uid.as_deref().filter(|uid| !uid.is_empty()) {
        Some(uid) => uid.to_owned(),
        None => format!(
            "{}/{}/{}",
            reference.kind.as_deref().unwrap_or_default(),
            reference.namespace.as_deref().unwrap_or_default(),
            reference.name.as_deref().unwrap_or_default(),
        ),
    }
}

/// Row id: the object, the window's first observation, and a digest of the full key. The
/// window start is part of the identity so a reopened cause is a new row, never a revived one.
fn row_id(key: &AggregateKey, first_seen: u64) -> String {
    let mut hasher = DefaultHasher::new();
    key.reason.hash(&mut hasher);
    key.object.hash(&mut hasher);
    key.message.hash(&mut hasher);
    format!("event/{}/{}/{first_seen:016x}", key.object, hasher.finish())
}

/// The Event object's own identity, so repeated observations of one object are not summed.
fn source_id(event: &Event) -> String {
    event
        .metadata
        .uid
        .clone()
        .filter(|uid| !uid.is_empty())
        .or_else(|| event.metadata.name.clone())
        .unwrap_or_default()
}

/// Occurrences this Event object reports: its series count, else its count, else one.
fn event_count(event: &Event) -> i64 {
    let count = event
        .series
        .as_ref()
        .and_then(|series| series.count)
        .or(event.count)
        .unwrap_or(1);
    i64::from(count.max(1))
}

/// First and last observation of this Event object, falling back to `now`.
fn observed_span(event: &Event, now: u64) -> (u64, u64) {
    let last = event
        .series
        .as_ref()
        .and_then(|series| series.last_observed_time.as_ref())
        .and_then(micro_millis)
        .or_else(|| time_millis(event.last_timestamp.as_ref()))
        .or_else(|| event.event_time.as_ref().and_then(micro_millis))
        .unwrap_or(now);
    let first = time_millis(event.first_timestamp.as_ref())
        .or_else(|| event.event_time.as_ref().and_then(micro_millis))
        .or_else(|| time_millis(event.metadata.creation_timestamp.as_ref()))
        .unwrap_or(last);
    (first.min(last), last)
}

pub(crate) fn time_millis(time: Option<&Time>) -> Option<u64> {
    time.map(|time| time.0.as_millisecond().max(0) as u64)
}

pub(crate) fn micro_millis(time: &MicroTime) -> Option<u64> {
    Some(time.0.as_millisecond().max(0) as u64)
}

/// RFC 3339 rendering of a millisecond timestamp, for the row's creation field.
fn rfc3339(millis: u64) -> Option<String> {
    Timestamp::from_millisecond(millis as i64)
        .ok()
        .map(|stamp| stamp.to_string())
}

fn row_for(entry: &Aggregate) -> Row {
    let count = entry.count().clamp(1, i64::from(i32::MAX)) as i32;
    Row {
        meta: RowMeta {
            uid: entry.id.clone(),
            name: entry
                .object_name
                .clone()
                .unwrap_or_else(|| entry.key.object.clone()),
            namespace: entry.object_namespace.clone(),
            kind: "Event".to_owned(),
            created: rfc3339(entry.last_seen),
            labels_hash: 0,
            resource_version: None,
        },
        status: RowStatus::Event(EventStatusRow {
            type_: entry.type_.clone(),
            reason: Some(entry.key.reason.clone()),
            count: Some(count),
            involved_kind: entry.object_kind.clone(),
            involved_name: entry.object_name.clone(),
            involved_namespace: entry.object_namespace.clone(),
            involved_uid: entry.object_uid.clone(),
            message: Some(entry.key.message.clone()),
            first_seen: Some(entry.first_seen),
            last_seen: Some(entry.last_seen),
        }),
    }
}

/// Current wall-clock time as milliseconds since the Unix epoch.
pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
