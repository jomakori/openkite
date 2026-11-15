//! Pod exec driver: opens a remote command in a container and feeds the exec pumps.
//!
//! The kube API hands back an [`AttachedProcess`] whose stream halves are decoupled from
//! the WebSocket: stdout/stderr are readers this driver hands to [`ExecStream`], stdin is a
//! writer whose *drop* is what tells the server stdin reached EOF, and terminal sizes are
//! sent on their own mpsc channel. The driver therefore owns four pieces of state, and the
//! three lifecycle events the caller needs — stdin EOF, terminal resize, session end with an
//! exit status — map onto them one-for-one instead of onto a single socket handle.
use std::future::Future;
use std::pin::Pin;

use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Status;
use kube::api::{AttachParams, AttachedProcess, TerminalSize};
use kube::{Api, Client};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc::Sender as ResizeSender;

use crate::driver::error::DriverError;
use crate::exec::{ExecChannel, ExecStream};
use crate::ring::DEFAULT_RING_CAPACITY;

/// The status the API server sends when the remote command ends.
///
/// kube resolves this future with `None` when the connection closes without a status frame.
type StatusFuture = Pin<Box<dyn Future<Output = Option<Status>> + Send>>;

/// Slack in front of the resize forwarder; only the newest size matters, so it stays small.
const RESIZE_QUEUE_DEPTH: usize = 4;

/// How a pod exec session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecExit {
    /// The remote command's exit status; `0` is success.
    Code(i32),
    /// The session ended without the server reporting a usable exit status.
    Unknown,
}

/// What to run in a pod, and how the session should be wired up.
#[derive(Debug, Clone)]
pub struct PodExecRequest {
    namespace: String,
    pod: String,
    container: Option<String>,
    command: Vec<String>,
    tty: bool,
    ring_capacity: usize,
}

impl PodExecRequest {
    /// A request for `command` in `pod`, defaulting to the pod's sole container and no tty.
    pub fn new<N, P, C, I>(namespace: N, pod: P, command: I) -> Self
    where
        N: Into<String>,
        P: Into<String>,
        C: Into<String>,
        I: IntoIterator<Item = C>,
    {
        Self {
            namespace: namespace.into(),
            pod: pod.into(),
            container: None,
            command: command.into_iter().map(Into::into).collect(),
            tty: false,
            ring_capacity: DEFAULT_RING_CAPACITY,
        }
    }

    /// Run in this container instead of the pod's sole container.
    #[must_use]
    pub fn container<C: Into<String>>(mut self, container: C) -> Self {
        self.container = Some(container.into());
        self
    }

    /// Allocate a tty for the session, which also makes the terminal resizable.
    ///
    /// A tty multiplexes stdout and stderr onto one stream, so the session exposes no
    /// separate stderr pump when this is set.
    #[must_use]
    pub fn tty(mut self, enable: bool) -> Self {
        self.tty = enable;
        self
    }

    /// Frames each exec ring may hold before the oldest are dropped.
    #[must_use]
    pub fn ring_capacity(mut self, capacity: usize) -> Self {
        self.ring_capacity = capacity;
        self
    }
}

/// One open exec session: the pumps its output lands in, plus the handles its lifecycle needs.
pub struct PodExecSession {
    process: Option<AttachedProcess>,
    stdout: ExecStream,
    stderr: Option<ExecStream>,
    stdin: Option<Box<dyn AsyncWrite + Unpin + Send>>,
    resize: Option<ResizeSender<TerminalSize>>,
    status: Option<StatusFuture>,
}

impl PodExecSession {
    /// Opens the session and starts pumping its output into per-channel exec streams.
    ///
    /// # Errors
    /// [`DriverError::Kube`] when the server rejects the exec call, and
    /// [`DriverError::MissingExecStdout`] when the response carried no stdout stream.
    pub async fn open(client: Client, request: PodExecRequest) -> Result<Self, DriverError> {
        let api: Api<Pod> = Api::namespaced(client, &request.namespace);

        let mut params = AttachParams::default()
            .stdin(true)
            .stdout(true)
            .stderr(!request.tty)
            .tty(request.tty);
        if let Some(container) = request.container.as_deref() {
            params = params.container(container);
        }

        let mut process = api.exec(&request.pod, request.command, &params).await?;

        // Take the halves before anything else: `terminal_size` and `take_status` consume
        // their ends of the session, and stdin keeps working only while its writer is alive.
        let stdin = process
            .stdin()
            .map(|writer| Box::new(writer) as Box<dyn AsyncWrite + Unpin + Send>);
        let stdout = process.stdout().ok_or(DriverError::MissingExecStdout)?;
        let stderr = process.stderr();
        // kube hands back its own (futures-channel) resize sender, which is a private
        // dependency of the client crate. Forward into a tokio channel we can name, so the
        // driver's public surface stays free of that type.
        let resize = process.terminal_size().map(|mut kube_resize| {
            let (sender, mut receiver) =
                tokio::sync::mpsc::channel::<TerminalSize>(RESIZE_QUEUE_DEPTH);
            tokio::spawn(async move {
                while let Some(size) = receiver.recv().await {
                    // A full queue means the server is behind: the newest size wins, so drop
                    // this one rather than block a resize call behind the socket.
                    let _ = kube_resize.try_send(size);
                    if kube_resize.is_closed() {
                        break;
                    }
                }
            });
            sender
        });
        let status = process
            .take_status()
            .map(|future| Box::pin(future) as StatusFuture);

        Ok(Self {
            stdout: ExecStream::spawn(stdout, ExecChannel::Stdout, request.ring_capacity),
            stderr: stderr.map(|reader| {
                ExecStream::spawn(reader, ExecChannel::Stderr, request.ring_capacity)
            }),
            stdin,
            resize,
            status,
            process: Some(process),
        })
    }

    /// The stdout pump; drain it through its sink.
    pub fn stdout(&self) -> &ExecStream {
        &self.stdout
    }

    /// The stderr pump, absent on tty sessions where the server multiplexes both streams.
    pub fn stderr(&self) -> Option<&ExecStream> {
        self.stderr.as_ref()
    }

    /// Whether stdin is still open, i.e. the pod has not been told about EOF yet.
    pub fn stdin_is_open(&self) -> bool {
        self.stdin.is_some()
    }

    /// Writes to the remote stdin, returning how many bytes were accepted.
    ///
    /// # Errors
    /// [`DriverError::StdinClosed`] once stdin has been closed, and [`DriverError::Io`] on
    /// a write failure.
    pub async fn write_stdin(&mut self, bytes: &[u8]) -> Result<usize, DriverError> {
        let Some(stdin) = self.stdin.as_mut() else {
            return Err(DriverError::StdinClosed);
        };
        AsyncWriteExt::write(stdin.as_mut(), bytes)
            .await
            .map_err(DriverError::Io)
    }

    /// Closes stdin only, so the pod sees EOF while the session keeps producing output.
    ///
    /// Dropping the writer half is exactly the signal kube forwards to the server as a
    /// stdin close, which is why this must not tear down the rest of the session.
    pub fn close_stdin(&mut self) {
        self.stdin = None;
    }

    /// Sends a terminal size update; the session must have been opened with `tty`.
    ///
    /// # Errors
    /// [`DriverError::ResizeNotSupported`] on a non-tty session and
    /// [`DriverError::ResizeChannelClosed`] once the session has ended.
    pub async fn resize(&self, width: u16, height: u16) -> Result<(), DriverError> {
        let Some(sender) = self.resize.as_ref() else {
            return Err(DriverError::ResizeNotSupported);
        };
        sender
            .send(TerminalSize { width, height })
            .await
            .map_err(|_| DriverError::ResizeChannelClosed)
    }

    /// Waits for the remote command to finish and reports how it ended.
    ///
    /// Call this once the output pumps are drained: kube drops the stream halves before it
    /// awaits its background task, so joining early can truncate late output.
    ///
    /// # Errors
    /// [`DriverError::ExecSessionFailed`] when the session's background task failed.
    pub async fn wait(mut self) -> Result<ExecExit, DriverError> {
        let joined = match self.process.take() {
            Some(process) => process
                .join()
                .await
                .map_err(|error| DriverError::ExecSessionFailed(error.to_string())),
            None => Ok(()),
        };

        // The status was taken before joining, so this resolves as soon as the session ends.
        let status = match self.status.take() {
            Some(future) => future.await,
            None => None,
        };

        joined?;

        // The server sends a status frame only when the command fails, so a session that
        // joined cleanly without one exited zero.
        Ok(status.as_ref().map_or(ExecExit::Code(0), exit_from_status))
    }

    /// Tears the whole session down: stdin, both pumps and the underlying connection.
    pub fn close(mut self) {
        self.stdin = None;
        self.stdout.close();
        if let Some(stderr) = self.stderr.as_ref() {
            stderr.close();
        }
        if let Some(process) = self.process.take() {
            process.abort();
        }
    }
}

/// Reads the exit status out of the status frame the server sends when a command ends.
///
/// A non-zero exit arrives as `status: Failure`, `reason: NonZeroExitCode` and an
/// `ExitCode` cause whose message carries the code; there is no dedicated field for it.
fn exit_from_status(status: &Status) -> ExecExit {
    if status.status.as_deref() == Some("Success") {
        return ExecExit::Code(0);
    }

    let code = status
        .details
        .as_ref()
        .and_then(|details| details.causes.as_ref())
        .and_then(|causes| {
            causes
                .iter()
                .find(|cause| cause.reason.as_deref() == Some("ExitCode"))
        })
        .and_then(|cause| cause.message.as_deref())
        .and_then(|message| message.trim().parse::<i32>().ok());

    code.map_or(ExecExit::Unknown, ExecExit::Code)
}

#[cfg(test)]
mod tests {
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{StatusCause, StatusDetails};

    use super::*;

    #[test]
    fn an_exit_code_cause_is_the_remote_exit_status() {
        let status = Status {
            status: Some("Failure".to_owned()),
            reason: Some("NonZeroExitCode".to_owned()),
            details: Some(StatusDetails {
                causes: Some(vec![StatusCause {
                    reason: Some("ExitCode".to_owned()),
                    message: Some("17".to_owned()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(exit_from_status(&status), ExecExit::Code(17));
    }

    #[test]
    fn a_success_status_is_a_zero_exit() {
        let status = Status {
            status: Some("Success".to_owned()),
            ..Default::default()
        };

        assert_eq!(exit_from_status(&status), ExecExit::Code(0));
    }

    #[test]
    fn a_failure_without_an_exit_code_cause_stays_unknown() {
        let status = Status {
            status: Some("Failure".to_owned()),
            reason: Some("NonZeroExitCode".to_owned()),
            message: Some("command terminated with non-zero exit code".to_owned()),
            ..Default::default()
        };

        assert_eq!(exit_from_status(&status), ExecExit::Unknown);
    }

    #[test]
    fn a_request_defaults_to_no_tty_and_the_default_ring() {
        let request = PodExecRequest::new("ns", "pod", ["sh", "-c", "true"]);

        assert_eq!(request.namespace, "ns");
        assert_eq!(request.pod, "pod");
        assert_eq!(request.command, ["sh", "-c", "true"]);
        assert!(!request.tty);
        assert!(request.container.is_none());
        assert_eq!(request.ring_capacity, DEFAULT_RING_CAPACITY);
    }
}
