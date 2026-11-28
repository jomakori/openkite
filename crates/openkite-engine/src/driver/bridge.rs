//! Bridges a `PodExecSession` to the wire contract shells speak: chunked frames in,
//! coalesced [`ExecEvent`]s out.
use kube::Client;
use openkite_api::exec::{ExecEvent, ExecExitCode, ExecOpen, ExecResize, ShellChannel};

use crate::driver::error::DriverError;
use crate::driver::{ExecExit, PodExecRequest, PodExecSession};
use crate::exec::{ExecChannel, ExecStream};

/// Converts an engine-side exit into the wire's channel-free exit code.
pub(crate) fn exit_to_wire(exit: ExecExit) -> ExecExitCode {
    match exit {
        ExecExit::Code(code) => ExecExitCode::Code(code),
        ExecExit::Unknown => ExecExitCode::Unknown,
    }
}

/// Converts an engine-side output channel into the wire's channel.
pub(crate) fn channel_to_wire(channel: ExecChannel) -> ShellChannel {
    match channel {
        ExecChannel::Stdout => ShellChannel::Stdout,
        ExecChannel::Stderr => ShellChannel::Stderr,
    }
}

/// Drains every buffered frame from `stream`, plus a drop event when the stream's drop
/// counter has grown past `last_dropped` since the previous call.
pub(crate) async fn drain_stream(
    stream: &ExecStream,
    channel: ExecChannel,
    last_dropped: &mut u64,
) -> Vec<ExecEvent> {
    let mut events = Vec::new();
    while let Some(frame) = stream.sink().pop().await {
        events.push(ExecEvent::Output {
            channel: channel_to_wire(channel),
            sequence: frame.sequence,
            bytes: frame.bytes,
        });
    }
    let dropped = stream.sink().dropped().await;
    if dropped > *last_dropped {
        events.push(ExecEvent::Dropped {
            channel: channel_to_wire(channel),
            count: dropped - *last_dropped,
        });
        *last_dropped = dropped;
    }
    events
}

/// One open exec session speaking the wire contract: an [`ExecOpen`] in, [`ExecEvent`]s out.
pub struct ShellBridge {
    session: PodExecSession,
    stdout_dropped: u64,
    stderr_dropped: u64,
}

impl ShellBridge {
    /// Opens a pod exec session from a wire-level open request.
    ///
    /// # Errors
    /// Whatever [`PodExecSession::open`] returns.
    pub async fn open(client: Client, open: ExecOpen) -> Result<Self, DriverError> {
        let mut request = PodExecRequest::new(open.namespace, open.pod, open.command);
        if let Some(container) = open.container {
            request = request.container(container);
        }
        if open.tty {
            request = request.tty(true);
        }
        let session = PodExecSession::open(client, request).await?;
        Ok(Self {
            session,
            stdout_dropped: 0,
            stderr_dropped: 0,
        })
    }

    /// Writes stdin bytes to the remote command.
    ///
    /// # Errors
    /// [`DriverError::StdinClosed`] once stdin has closed, [`DriverError::Io`] on a write
    /// failure.
    pub async fn write_stdin(&mut self, bytes: &[u8]) -> Result<usize, DriverError> {
        self.session.write_stdin(bytes).await
    }

    /// Resizes the remote terminal; only valid on a session opened with `tty: true`.
    ///
    /// # Errors
    /// [`DriverError::ResizeNotSupported`] on a non-tty session.
    pub async fn resize(&self, resize: ExecResize) -> Result<(), DriverError> {
        self.session.resize(resize.cols, resize.rows).await
    }

    /// Drains every pump currently buffered, oldest-sequence first per channel.
    pub async fn poll(&mut self) -> Vec<ExecEvent> {
        let mut events = drain_stream(
            self.session.stdout(),
            ExecChannel::Stdout,
            &mut self.stdout_dropped,
        )
        .await;
        if let Some(stderr) = self.session.stderr() {
            events
                .extend(drain_stream(stderr, ExecChannel::Stderr, &mut self.stderr_dropped).await);
        }
        events
    }

    /// Closes stdin only, telling the pod the shell's input ended while output keeps flowing.
    pub async fn close_stdin(&mut self) {
        self.session.close_stdin().await;
    }

    /// Waits for the remote command to end and reports how, as a wire [`ExecEvent::Exit`].
    ///
    /// # Errors
    /// [`DriverError::ExecSessionFailed`] when the session's background task failed.
    pub async fn wait(self) -> Result<ExecEvent, DriverError> {
        let exit = self.session.wait().await?;
        Ok(ExecEvent::Exit(exit_to_wire(exit)))
    }

    /// Tears the whole session down immediately: stdin, both pumps, the connection. This is
    /// what a detach leaves no orphan stream means in practice.
    pub fn close(self) {
        self.session.close();
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt;

    use super::*;
    use crate::exec::ExecChannel as EngineChannel;

    #[test]
    fn exit_code_maps_straight_through() {
        assert_eq!(exit_to_wire(ExecExit::Code(7)), ExecExitCode::Code(7));
        assert_eq!(exit_to_wire(ExecExit::Unknown), ExecExitCode::Unknown);
    }

    #[test]
    fn channels_map_straight_through() {
        assert_eq!(channel_to_wire(EngineChannel::Stdout), ShellChannel::Stdout);
        assert_eq!(channel_to_wire(EngineChannel::Stderr), ShellChannel::Stderr);
    }

    #[tokio::test]
    async fn drain_emits_output_in_sequence_order() {
        let (mut writer, reader) = tokio::io::duplex(64 * 1024);
        let stream = ExecStream::spawn(reader, EngineChannel::Stdout, 16);
        writer.write_all(b"hello").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut last_dropped = 0;
        let events = drain_stream(&stream, EngineChannel::Stdout, &mut last_dropped).await;

        assert_eq!(events.len(), 1);
        match &events[0] {
            ExecEvent::Output {
                channel,
                sequence,
                bytes,
            } => {
                assert_eq!(*channel, ShellChannel::Stdout);
                assert_eq!(*sequence, 0);
                assert_eq!(bytes, b"hello");
            }
            other => panic!("expected an Output event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn drain_reports_drops_once_as_a_delta() {
        let (mut writer, reader) = tokio::io::duplex(64 * 1024);
        let stream = ExecStream::spawn(reader, EngineChannel::Stdout, 2);
        // Each write is its own read() call worth of data, so each becomes its own frame
        // (a single write_all would land in one read and never overflow the ring).
        for byte in [1u8, 2, 3, 4] {
            writer.write_all(&[byte]).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let mut last_dropped = 0;
        let first = drain_stream(&stream, EngineChannel::Stdout, &mut last_dropped).await;
        assert!(first
            .iter()
            .any(|event| matches!(event, ExecEvent::Dropped { count, .. } if *count > 0)));

        let second = drain_stream(&stream, EngineChannel::Stdout, &mut last_dropped).await;
        assert!(second
            .iter()
            .all(|event| !matches!(event, ExecEvent::Dropped { .. })));
    }
}
