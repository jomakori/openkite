//! Log dock model: panes, the follow/pause gate and the per-frame read budget.
//!
//! Rendering lives in [`crate::logs_view`]. Nothing here touches the DOM or a socket: a pane
//! accepts whatever the caller already read from its log stream, which is what makes "pause
//! stops reads" checkable — a paused pane reports zero bytes accepted, so a caller that honours
//! [`LogPane::wants`] never drains its stream at all.

use std::collections::VecDeque;

/// Severity parsed out of the line's own text; drives the colour of the level column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    /// Lowercase CSS token: `error`, `warn`, `info`, `debug`, `trace`.
    pub fn id(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
            Level::Trace => "trace",
        }
    }

    /// Uppercase column label.
    pub fn label(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }

    /// Guesses a level from a raw line, defaulting to `Info`.
    ///
    /// Pods are inconsistent about where the level sits, so this looks for the usual tokens
    /// anywhere in the line rather than assuming a column position.
    pub fn parse(line: &str) -> Level {
        let upper = line.to_ascii_uppercase();
        if upper.contains("ERROR")
            || upper.contains("FATAL")
            || upper.contains("PANIC")
            || upper.contains("EXCEPTION")
        {
            Level::Error
        } else if upper.contains("WARN") {
            Level::Warn
        } else if upper.contains("DEBUG") {
            Level::Debug
        } else if upper.contains("TRACE") {
            Level::Trace
        } else {
            Level::Info
        }
    }
}

/// Splits a leading `HH:MM:SS(.mmm)` token off a line, as kubelet writes it.
///
/// Returns the stamp and the remainder. Lines without a leading stamp keep their text whole.
fn split_stamp(line: &str) -> (Option<String>, &str) {
    let stamp_len = {
        let bytes = line.as_bytes();
        let mut at = 0;
        let mut digits = 0;
        let mut colons = 0;
        let mut dot = false;
        while at < bytes.len() {
            match bytes[at] {
                b'0'..=b'9' => digits += 1,
                b':' if colons < 2 && !dot => colons += 1,
                b'.' if colons == 2 && !dot => dot = true,
                b' ' if colons == 2 && digits >= 6 => break,
                _ => return (None, line),
            }
            at += 1;
        }
        if colons == 2 && digits >= 6 && at < bytes.len() {
            at
        } else {
            return (None, line);
        }
    };
    (
        Some(line[..stamp_len].to_string()),
        line[stamp_len..].trim_start(),
    )
}

/// One rendered log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    /// Monotonic id, so the view can key rows across eviction.
    pub seq: u64,
    /// Timestamp column, already formatted by the caller; `None` hides it.
    pub stamp: Option<String>,
    pub level: Level,
    pub text: String,
}

/// A pod the dock can follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodChoice {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub containers: Vec<String>,
}

impl PodChoice {
    /// A pod with a single container.
    pub fn new(id: &str, name: &str, namespace: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            namespace: namespace.to_string(),
            containers: vec!["main".to_string()],
        }
    }

    /// Replaces the container list.
    pub fn with_containers(mut self, containers: &[&str]) -> Self {
        self.containers = containers.iter().map(|c| c.to_string()).collect();
        self
    }
}

/// Bytes a pane may absorb in one frame.
///
/// 128 KiB leaves better than 50% headroom over the 5 MB/s the ticket requires at 60 fps
/// (5 MB/s ÷ 60 = 85,334 B per frame), so a burst cannot push a frame past its budget.
pub const FRAME_BUDGET_BYTES: usize = 128 * 1024;

/// Lines a pane keeps before it evicts from the front.
pub const PANE_CAPACITY: usize = 5_000;

/// One log pane: a pod, a container, and the toggles that decide whether it reads at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogPane {
    pub pod: PodChoice,
    pub container: String,
    /// Temps panes are replaced by the next pod the user opens; pinned panes stay.
    pub pinned: bool,
    pub follow: bool,
    pub paused: bool,
    pub timestamps: bool,
    pub filter: String,
    pub lines: VecDeque<LogLine>,
    bytes_in: u64,
    dropped: u64,
    next_seq: u64,
}

impl LogPane {
    /// A following, timestamped pane for `pod`'s first container.
    pub fn new(pod: PodChoice) -> Self {
        let container = pod.containers.first().cloned().unwrap_or_default();
        Self {
            pod,
            container,
            pinned: false,
            follow: true,
            paused: false,
            timestamps: true,
            filter: String::new(),
            lines: VecDeque::with_capacity(256),
            bytes_in: 0,
            dropped: 0,
            next_seq: 0,
        }
    }

    /// Whether the caller should read from this pane's stream before the next frame.
    ///
    /// Paused panes and panes that are not following return `false`: there is no point reading
    /// bytes that would only be thrown away.
    pub fn wants(&self) -> bool {
        self.follow && !self.paused
    }

    /// Bytes this pane has accepted since it opened; a paused pane leaves this untouched.
    pub fn bytes_in(&self) -> u64 {
        self.bytes_in
    }

    /// Lines evicted from the front to stay inside [`PANE_CAPACITY`].
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Accepts already-read lines, keeping at most [`FRAME_BUDGET_BYTES`] of them.
    ///
    /// Returns the bytes taken. A paused or non-following pane takes nothing, and lines past the
    /// budget are refused rather than buffered — the ring in the engine is the only buffer.
    pub fn ingest(&mut self, lines: &[String]) -> usize {
        if !self.wants() {
            return 0;
        }
        let mut taken = 0usize;
        for line in lines {
            let size = line.len();
            if taken + size > FRAME_BUDGET_BYTES {
                break;
            }
            taken += size;
            self.push(line);
        }
        self.bytes_in += taken as u64;
        taken
    }

    fn push(&mut self, line: &str) {
        let (stamp, text) = split_stamp(line);
        self.lines.push_back(LogLine {
            seq: self.next_seq,
            stamp,
            level: Level::parse(text),
            text: text.to_string(),
        });
        self.next_seq += 1;
        while self.lines.len() > PANE_CAPACITY {
            self.lines.pop_front();
            self.dropped += 1;
        }
    }

    /// Lines that survive the search filter, newest last.
    pub fn visible(&self) -> impl Iterator<Item = &LogLine> {
        let needle = self.filter.trim().to_lowercase();
        self.lines.iter().filter(move |line| {
            needle.is_empty()
                || line.text.to_lowercase().contains(&needle)
                || line.level.label().to_lowercase().contains(&needle)
        })
    }

    /// How many visible lines carry a warning or worse — the dock's badge.
    pub fn problems(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.level >= Level::Warn)
            .count()
    }

    /// Switches container, dropping lines that belong to the old one.
    pub fn set_container(&mut self, container: &str) {
        if self.container == container {
            return;
        }
        self.container = container.to_string();
        self.lines.clear();
        self.dropped = 0;
        self.bytes_in = 0;
    }

    /// Points this pane at another pod, keeping its toggles and filter.
    pub fn retarget(&mut self, pod: PodChoice) {
        self.container = pod.containers.first().cloned().unwrap_or_default();
        self.pod = pod;
        self.lines.clear();
        self.dropped = 0;
        self.bytes_in = 0;
    }

    /// Drops buffered lines without touching the stream or the counters.
    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

/// What a dock control asks the caller to change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockAction {
    Toggle,
    Close,
    Focus(usize),
    ToggleFollow,
    TogglePause,
    ToggleTimestamps,
    TogglePin,
    SetFilter(String),
    SetContainer(String),
    Clear,
    Split,
    Merge,
    PickPod(String),
}

/// The dock: one or two panes plus the state that decides which of them is being edited.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogDock {
    pub open: bool,
    pub panes: Vec<LogPane>,
    /// Index of the pane the toolbar acts on; always in range while `panes` is non-empty.
    pub focused: usize,
    /// Pods offered by the picker, in the order the caller supplied them.
    pub choices: Vec<PodChoice>,
}

impl LogDock {
    /// Whether two panes are on screen; the ticket caps this at two.
    pub fn split(&self) -> bool {
        self.panes.len() > 1
    }

    /// The pane the toolbar and keyboard act on, if any.
    pub fn focused_pane(&self) -> Option<&LogPane> {
        self.panes.get(self.focused)
    }

    /// Mutable form of [`Self::focused_pane`].
    pub fn focused_pane_mut(&mut self) -> Option<&mut LogPane> {
        let at = self.focused;
        self.panes.get_mut(at)
    }

    /// Opens `pod`, replacing the focused pane when it is a temp one and splitting otherwise.
    pub fn open_pod(&mut self, pod: PodChoice) -> &mut LogPane {
        self.open = true;
        match self.panes.iter().position(|pane| !pane.pinned) {
            Some(at) if !self.split() || self.focused == at => {
                self.focused = at;
                self.panes[at].retarget(pod);
            }
            _ if !self.split() => {
                self.panes.push(LogPane::new(pod));
                self.focused = self.panes.len() - 1;
            }
            _ => {
                let at = self.focused;
                self.panes[at].retarget(pod);
            }
        }
        &mut self.panes[self.focused]
    }

    /// Replaces the choices the pod picker shows.
    pub fn set_choices(&mut self, choices: Vec<PodChoice>) {
        self.choices = choices;
    }

    /// Applies a control intent, returning whether anything changed.
    pub fn apply(&mut self, action: DockAction) -> bool {
        match action {
            DockAction::Toggle => {
                self.open = !self.open;
                return true;
            }
            DockAction::Close => {
                self.open = false;
                return true;
            }
            DockAction::Focus(at) => {
                if at < self.panes.len() && at != self.focused {
                    self.focused = at;
                    return true;
                }
                return false;
            }
            DockAction::Split => {
                if self.split() {
                    return false;
                }
                let next = self
                    .focused_pane()
                    .map(|pane| LogPane {
                        lines: VecDeque::with_capacity(64),
                        bytes_in: 0,
                        dropped: 0,
                        next_seq: 0,
                        ..pane.clone()
                    })
                    .unwrap_or_else(|| {
                        LogPane::new(
                            self.choices
                                .first()
                                .cloned()
                                .unwrap_or_else(|| PodChoice::new("pod/", "-", "-")),
                        )
                    });
                self.panes.push(next);
                self.focused = self.panes.len() - 1;
                return true;
            }
            DockAction::Merge => {
                if !self.split() {
                    return false;
                }
                self.panes.truncate(1);
                self.focused = 0;
                return true;
            }
            DockAction::PickPod(id) => {
                let Some(pod) = self.choices.iter().find(|c| c.id == id).cloned() else {
                    return false;
                };
                self.open_pod(pod);
                return true;
            }
            _ => {}
        }
        let Some(pane) = self.focused_pane_mut() else {
            return false;
        };
        match action {
            DockAction::ToggleFollow => pane.follow = !pane.follow,
            DockAction::TogglePause => pane.paused = !pane.paused,
            // The column the pane renders, not the buffer: stamps stay parsed either way.
            DockAction::ToggleTimestamps => pane.timestamps = !pane.timestamps,
            DockAction::TogglePin => pane.pinned = !pane.pinned,
            DockAction::SetFilter(text) => pane.filter = text,
            DockAction::SetContainer(name) => pane.set_container(&name),
            DockAction::Clear => pane.clear(),
            DockAction::Toggle
            | DockAction::Close
            | DockAction::Focus(_)
            | DockAction::Split
            | DockAction::Merge
            | DockAction::PickPod(_) => unreachable!("handled above"),
        }
        true
    }

    /// Reads one frame's worth of lines for every pane that wants them.
    ///
    /// `read` is called only for panes that are following and not paused, which is the whole
    /// point: a paused pane costs nothing, not even the read.
    pub fn pump<F>(&mut self, mut read: F) -> usize
    where
        F: FnMut(&LogPane) -> Vec<String>,
    {
        let mut total = 0;
        for pane in &mut self.panes {
            if !pane.wants() {
                continue;
            }
            let lines = read(pane);
            total += pane.ingest(&lines);
        }
        total
    }

    /// Panes that would read on the next frame.
    pub fn readers(&self) -> usize {
        self.panes.iter().filter(|pane| pane.wants()).count()
    }
}
