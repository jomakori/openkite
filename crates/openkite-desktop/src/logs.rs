//! Pod log streaming: follow, container selection, and the kube log stream.

use k8s_openapi::api::core::v1::Pod;
use kube::api::LogParams;
use kube::Api;

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
