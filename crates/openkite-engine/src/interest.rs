//! Refcounted interest registry: tracks what to watch and when to park.
use kube::core::GroupVersionKind;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterestClass {
    Pinned,
    Visible,
    Adjacent,
    Probe,
}

impl InterestClass {
    pub fn park_grace(&self) -> Option<Duration> {
        match self {
            InterestClass::Pinned => None,
            InterestClass::Visible => Some(Duration::from_secs(5)),
            InterestClass::Adjacent => Some(Duration::from_secs(30)),
            InterestClass::Probe => Some(Duration::from_secs(300)),
        }
    }

    /// How long upserts are batched before being broadcast to subscribers of this class.
    pub fn coalesce_interval(&self) -> Duration {
        match self {
            InterestClass::Pinned => Duration::from_secs(1),
            InterestClass::Visible => Duration::from_millis(120),
            InterestClass::Adjacent => Duration::from_secs(1),
            InterestClass::Probe => Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scope {
    Cluster,
    Namespace(String),
}

#[derive(Debug, Clone)]
pub struct Interest {
    pub gvk: GroupVersionKind,
    pub scope: Scope,
    pub selector: Option<String>,
    pub class: InterestClass,
}

impl Interest {
    pub(crate) fn key(&self) -> (GroupVersionKind, Scope, Option<String>) {
        (self.gvk.clone(), self.scope.clone(), self.selector.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InformerState {
    Running,
    Parked,
    Frozen,
}

pub struct InterestRegistry {
    entries: HashMap<(GroupVersionKind, Scope, Option<String>), (usize, InformerState)>,
}

impl InterestRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn acquire(&mut self, interest: &Interest) -> usize {
        let key = interest.key();
        let entry = self
            .entries
            .entry(key)
            .or_insert((0, InformerState::Running));
        entry.0 += 1;
        entry.0
    }

    pub fn release(&mut self, interest: &Interest) -> Option<Duration> {
        let key = interest.key();
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.0 = entry.0.saturating_sub(1);
            if entry.0 == 0 {
                return interest.class.park_grace();
            }
        }
        None
    }

    pub fn set_state(&mut self, interest: &Interest, state: InformerState) {
        let key = interest.key();
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.1 = state;
        }
    }

    pub fn state(&self, interest: &Interest) -> Option<InformerState> {
        let key = interest.key();
        self.entries.get(&key).map(|(_, state)| *state)
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for InterestRegistry {
    fn default() -> Self {
        Self::new()
    }
}
