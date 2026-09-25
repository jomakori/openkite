//! Pod log streaming: follow, container selection, and a capped line buffer.

use k8s_openapi::api::core::v1::Pod;
use kube::api::LogParams;
use kube::Api;

/// The maximum number of lines retained in a [`LineBuffer`].
pub const MAX_LINES: usize = 10_000;

/// Options controlling a pod log request.
#[derive(Debug, Clone, Default)]
pub struct LogOptions {
    /// Restrict logs to this container (defaults to the pod's first container).
    pub container: Option<String>,
    /// Keep the stream open and emit new lines as they arrive.
    pub follow: bool,
    /// Return only the last N lines.
    pub tail_lines: Option<i64>,
    /// Prefix each line with its RFC 3339 timestamp.
    pub timestamps: bool,
}

impl LogOptions {
    /// Convert these options into kube-rs log parameters.
    pub fn to_params(&self) -> LogParams {
        LogParams {
            container: self.container.clone(),
            follow: self.follow,
            tail_lines: self.tail_lines,
            timestamps: self.timestamps,
            ..LogParams::default()
        }
    }
}

/// A handle to a pod's log stream, opened on demand against a live cluster.
pub struct LogStream {
    api: Api<Pod>,
    name: String,
    options: LogOptions,
}

impl LogStream {
    /// Create a log stream for `name` using `options`.
    pub fn new(api: Api<Pod>, name: impl Into<String>, options: LogOptions) -> Self {
        Self {
            api,
            name: name.into(),
            options,
        }
    }

    /// The pod this stream reads from.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Open the underlying byte stream (requires a live cluster).
    pub async fn open(&self) -> kube::Result<impl futures::AsyncBufRead> {
        self.api
            .log_stream(&self.name, &self.options.to_params())
            .await
    }
}

/// A fixed-capacity buffer of log lines that drops the oldest when full.
#[derive(Debug, Clone, Default)]
pub struct LineBuffer {
    lines: Vec<String>,
}

impl LineBuffer {
    /// Append a line, evicting the oldest line if the buffer is at capacity.
    pub fn push(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
        if self.lines.len() > MAX_LINES {
            let overflow = self.lines.len() - MAX_LINES;
            self.lines.drain(..overflow);
        }
    }

    /// The retained lines, oldest first.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Number of retained lines.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the buffer holds no lines.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Drop all retained lines.
    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

/// The follow/pause state of a live log view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowState {
    /// New lines are shown as they arrive.
    Following,
    /// New lines are buffered but the view stays pinned (user scrolled up).
    Paused,
}

impl FollowState {
    /// Pause following — used when the user scrolls up.
    pub fn pause(&mut self) {
        *self = Self::Paused;
    }

    /// Resume following — used when the user scrolls back to the bottom.
    pub fn resume(&mut self) {
        *self = Self::Following;
    }

    /// Whether new lines should be shown immediately.
    pub fn is_following(&self) -> bool {
        matches!(self, Self::Following)
    }
}
