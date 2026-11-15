//! Errors from kube client drivers: exec/attach, logs, port-forward, and exec credential plugins.
use thiserror::Error;

/// All failure modes the kube client drivers can surface.
#[derive(Debug, Error)]
pub enum DriverError {
    /// The underlying kube API call failed.
    #[error("kube api error: {0}")]
    Kube(#[from] kube::Error),

    /// A requested port-forward stream was not returned by the server.
    #[error("port-forward did not open a stream for port {0}")]
    MissingPortStream(u16),

    /// A pod exec session did not return the stdout stream it was opened with.
    #[error("pod exec did not return a stdout stream")]
    MissingExecStdout,

    /// Resizing a terminal was requested on a session that was not opened with a tty.
    #[error("terminal resize is only available on sessions opened with tty=true")]
    ResizeNotSupported,

    /// The resize channel's receiver has gone away.
    #[error("terminal resize channel is closed")]
    ResizeChannelClosed,

    /// Writing to a session's stdin after it was closed.
    #[error("pod exec stdin is already closed")]
    StdinClosed,

    /// The exec session's background task failed while the command was running.
    ///
    /// kube's attach error type is private to the client crate, so only its message survives.
    #[error("pod exec session failed: {0}")]
    ExecSessionFailed(String),

    /// I/O error shutting down a stream.
    #[error("i/o error: {0}")]
    Io(#[source] std::io::Error),

    /// The kubeconfig's exec credential plugin entry has no command set.
    #[error("exec credential plugin has no command configured")]
    ExecMissingCommand,

    /// Failed to serialize the `KUBERNETES_EXEC_INFO` payload for the plugin.
    #[error("failed to serialize exec plugin info: {0}")]
    ExecSerialize(#[source] serde_json::Error),

    /// Failed to spawn the exec credential plugin process.
    #[error("failed to spawn exec credential plugin: {0}")]
    ExecSpawn(#[source] std::io::Error),

    /// The exec credential plugin exited with a non-zero status.
    #[error("exec credential plugin '{command}' exited with {status}: {stderr}")]
    ExecFailed {
        command: String,
        status: std::process::ExitStatus,
        stderr: String,
    },

    /// The exec credential plugin's stdout did not parse as an `ExecCredential`.
    #[error("failed to parse exec credential plugin output: {0}")]
    ExecParse(#[source] serde_json::Error),

    /// The exec credential plugin emitted no `status` field.
    #[error("exec credential plugin emitted no status")]
    ExecNoStatus,

    /// The exec credential plugin's status carried neither a token nor a client certificate.
    #[error("exec credential plugin emitted no usable credential")]
    ExecNoCredential,
}
