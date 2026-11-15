//! Port-forward session: open/close lifecycle over one forwarded port's duplex stream.
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// Lifecycle of a port-forward session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortForwardState {
    Open,
    Closed,
}

/// Wraps one forwarded port's duplex stream, closing it cleanly exactly once.
pub struct PortForwardSession<S> {
    port: u16,
    stream: Option<S>,
    closed: AtomicBool,
}

impl<S> PortForwardSession<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    /// Wraps an already-open duplex stream for `port`.
    pub fn new(port: u16, stream: S) -> Self {
        Self {
            port,
            stream: Some(stream),
            closed: AtomicBool::new(false),
        }
    }

    /// The forwarded port this session carries.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Current lifecycle state.
    pub fn state(&self) -> PortForwardState {
        if self.closed.load(Ordering::SeqCst) {
            PortForwardState::Closed
        } else {
            PortForwardState::Open
        }
    }

    /// Takes the underlying stream for reading and writing; returns it at most once.
    pub fn take_stream(&mut self) -> Option<S> {
        self.stream.take()
    }

    /// Shuts the stream down so the peer observes EOF rather than an orphan socket; idempotent.
    pub async fn close(&mut self) {
        if let Some(mut stream) = self.stream.take() {
            let _ = stream.shutdown().await;
        }
        self.closed.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn new_session_starts_open() {
        let (a, _b) = tokio::io::duplex(64);
        let session = PortForwardSession::new(5000, a);
        assert_eq!(session.state(), PortForwardState::Open);
        assert_eq!(session.port(), 5000);
    }

    #[tokio::test]
    async fn close_shuts_down_the_stream_and_the_peer_observes_eof() {
        let (a, mut b) = tokio::io::duplex(64);
        let mut session = PortForwardSession::new(5000, a);

        session.close().await;
        assert_eq!(session.state(), PortForwardState::Closed);

        let mut buf = [0u8; 8];
        let n = b.read(&mut buf).await.expect("peer read");
        assert_eq!(n, 0, "peer must observe EOF, never an orphan socket");
    }

    #[tokio::test]
    async fn close_is_idempotent() {
        let (a, _b) = tokio::io::duplex(64);
        let mut session = PortForwardSession::new(5000, a);
        session.close().await;
        session.close().await;
        assert_eq!(session.state(), PortForwardState::Closed);
    }

    #[tokio::test]
    async fn take_stream_returns_it_at_most_once() {
        let (a, _b) = tokio::io::duplex(64);
        let mut session = PortForwardSession::new(5000, a);
        assert!(session.take_stream().is_some());
        assert!(session.take_stream().is_none());
    }
}
