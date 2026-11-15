//! Watch admission, client-side QPS shaping, and load shedding, with a per-cluster snapshot.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use kube::core::GroupVersionKind;

use crate::interest::{InformerState, Interest, InterestClass, InterestRegistry, Scope};

/// Default concurrent-watch ceiling before admission starts parking.
pub const DEFAULT_WATCH_LIMIT: usize = 16;
/// Default sustained client-side QPS.
pub const DEFAULT_QPS: f64 = 20.0;
/// Default burst allowance above the sustained QPS.
pub const DEFAULT_QPS_BURST: f64 = 40.0;
/// Cap on retained warning strings so the snapshot never grows unbounded.
const MAX_WARNINGS: usize = 32;

type Key = (GroupVersionKind, Scope, Option<String>);

/// Outcome of a watch admission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Admitted,
    Parked,
}

/// Which bucket a QPS-governed call falls in: writes queue, probes shed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    Write,
    Probe,
}

/// Outcome of a QPS check for one call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Queued,
    Shed,
}

/// Per-`InterestClass` tally used in the budget snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClassCounts {
    pub pinned: usize,
    pub visible: usize,
    pub adjacent: usize,
    pub probe: usize,
}

impl ClassCounts {
    fn bump(&mut self, class: InterestClass) {
        match class {
            InterestClass::Pinned => self.pinned += 1,
            InterestClass::Visible => self.visible += 1,
            InterestClass::Adjacent => self.adjacent += 1,
            InterestClass::Probe => self.probe += 1,
        }
    }
}

/// Per-cluster budget state the UI can read: limits, usage, shed/queued counters, warnings.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetSnapshot {
    pub watch_limit: usize,
    pub watches_running: usize,
    pub watches_parked: usize,
    pub running_by_class: ClassCounts,
    pub parked_by_class: ClassCounts,
    pub qps_limit: f64,
    pub qps_burst: f64,
    pub queued_writes: usize,
    pub shed_probes: usize,
    pub warnings: Vec<String>,
}

/// Lower rank parks first when the concurrent-watch limit is hit.
fn class_rank(class: InterestClass) -> u8 {
    match class {
        InterestClass::Probe => 0,
        InterestClass::Adjacent => 1,
        InterestClass::Visible => 2,
        InterestClass::Pinned => 3,
    }
}

struct GovernorState {
    registry: InterestRegistry,
    watches: HashMap<Key, Interest>,
    tokens: f64,
    last_refill: Instant,
    queued_writes: usize,
    shed_probes: usize,
    warnings: Vec<String>,
}

impl GovernorState {
    fn refill(&mut self, rate: f64, burst: f64) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * rate).min(burst);
        self.last_refill = now;
    }

    fn warn(&mut self, message: String) {
        self.warnings.push(message);
        if self.warnings.len() > MAX_WARNINGS {
            self.warnings.remove(0);
        }
    }
}

/// Admission gate and QPS shaper for one cluster's watches.
pub struct Governor {
    watch_limit: usize,
    qps_limit: f64,
    qps_burst: f64,
    state: Mutex<GovernorState>,
}

impl Governor {
    /// A governor with the given concurrent-watch limit and QPS/burst.
    pub fn new(watch_limit: usize, qps_limit: f64, qps_burst: f64) -> Self {
        Self {
            watch_limit,
            qps_limit,
            qps_burst,
            state: Mutex::new(GovernorState {
                registry: InterestRegistry::new(),
                watches: HashMap::new(),
                tokens: qps_burst,
                last_refill: Instant::now(),
                queued_writes: 0,
                shed_probes: 0,
                warnings: Vec::new(),
            }),
        }
    }

    /// Refcount+1 on `interest`; parks it (or evicts a lower-class watch to admit it) at the cap.
    pub fn acquire(&self, interest: &Interest) -> Admission {
        let mut state = self.state.lock().expect("governor mutex poisoned");
        let key = interest.key();

        if let Some(existing) = state.watches.get(&key).cloned() {
            state.registry.acquire(&existing);
            return match state.registry.state(&existing) {
                Some(InformerState::Parked) => Admission::Parked,
                _ => Admission::Admitted,
            };
        }

        let running: Vec<Interest> = state
            .watches
            .values()
            .filter(|watch| state.registry.state(watch) == Some(InformerState::Running))
            .cloned()
            .collect();

        if running.len() < self.watch_limit {
            state.registry.acquire(interest);
            state.registry.set_state(interest, InformerState::Running);
            state.watches.insert(key, interest.clone());
            return Admission::Admitted;
        }

        let victim = running
            .into_iter()
            .min_by_key(|watch| class_rank(watch.class));

        match victim {
            Some(victim) if class_rank(victim.class) < class_rank(interest.class) => {
                state.registry.set_state(&victim, InformerState::Parked);
                state.registry.acquire(interest);
                state.registry.set_state(interest, InformerState::Running);
                state.watches.insert(key, interest.clone());
                state.warn(format!(
                    "watch limit {} reached: parked a lower-class informer to admit a higher-class watch",
                    self.watch_limit
                ));
                Admission::Admitted
            }
            _ => {
                state.registry.acquire(interest);
                state.registry.set_state(interest, InformerState::Parked);
                state.watches.insert(key, interest.clone());
                state.warn(format!(
                    "watch limit {} reached: parked instead of opening",
                    self.watch_limit
                ));
                Admission::Parked
            }
        }
    }

    /// Drops a refcount; parks the interest immediately once nothing holds it.
    pub fn release(&self, interest: &Interest) {
        let mut state = self.state.lock().expect("governor mutex poisoned");
        if state.registry.release(interest).is_some() {
            state.registry.set_state(interest, InformerState::Parked);
        }
    }

    /// Forwards a state change, keeping the tracked `Interest` (and its class) current.
    pub fn set_state(&self, interest: &Interest, new_state: InformerState) {
        let mut state = self.state.lock().expect("governor mutex poisoned");
        state.registry.set_state(interest, new_state);
        state.watches.insert(interest.key(), interest.clone());
    }

    /// The live state for `interest`, straight from the backing registry.
    pub fn state(&self, interest: &Interest) -> Option<InformerState> {
        self.state
            .lock()
            .expect("governor mutex poisoned")
            .registry
            .state(interest)
    }

    /// How many distinct interests the governor has ever tracked.
    pub fn count(&self) -> usize {
        self.state
            .lock()
            .expect("governor mutex poisoned")
            .registry
            .count()
    }

    /// QPS-gates one call: allow immediately, queue (writes), or shed (probes).
    pub fn check(&self, kind: RequestKind) -> Decision {
        let mut state = self.state.lock().expect("governor mutex poisoned");
        state.refill(self.qps_limit, self.qps_burst);
        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            return Decision::Allow;
        }
        match kind {
            RequestKind::Write => {
                state.queued_writes += 1;
                Decision::Queued
            }
            RequestKind::Probe => {
                state.shed_probes += 1;
                Decision::Shed
            }
        }
    }

    /// Releases as many queued writes as current tokens allow; returns how many drained.
    pub fn drain_queue(&self) -> usize {
        let mut state = self.state.lock().expect("governor mutex poisoned");
        state.refill(self.qps_limit, self.qps_burst);
        let drained = (state.tokens.floor() as usize).min(state.queued_writes);
        state.tokens -= drained as f64;
        state.queued_writes -= drained;
        drained
    }

    /// The current per-cluster budget snapshot: limits, usage, shed/queued counts, warnings.
    pub fn snapshot(&self) -> BudgetSnapshot {
        let state = self.state.lock().expect("governor mutex poisoned");
        let mut running_by_class = ClassCounts::default();
        let mut parked_by_class = ClassCounts::default();
        let mut watches_running = 0usize;
        let mut watches_parked = 0usize;

        for watch in state.watches.values() {
            match state.registry.state(watch) {
                Some(InformerState::Running) => {
                    watches_running += 1;
                    running_by_class.bump(watch.class);
                }
                Some(InformerState::Parked) => {
                    watches_parked += 1;
                    parked_by_class.bump(watch.class);
                }
                Some(InformerState::Frozen) | None => {}
            }
        }

        BudgetSnapshot {
            watch_limit: self.watch_limit,
            watches_running,
            watches_parked,
            running_by_class,
            parked_by_class,
            qps_limit: self.qps_limit,
            qps_burst: self.qps_burst,
            queued_writes: state.queued_writes,
            shed_probes: state.shed_probes,
            warnings: state.warnings.clone(),
        }
    }
}

impl Default for Governor {
    fn default() -> Self {
        Self::new(DEFAULT_WATCH_LIMIT, DEFAULT_QPS, DEFAULT_QPS_BURST)
    }
}
