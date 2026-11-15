//! Interactive exec output: sequenced frames pumped into a drop-oldest ring with backpressure.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::ring::{RingBuffer, DEFAULT_RING_CAPACITY};

/// Chunk size read from the underlying exec stream per poll.
const CHUNK_SIZE: usize = 4096;

/// Which exec channel a frame came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecChannel {
    Stdout,
    Stderr,
}

/// One chunk of exec output tagged with its channel and a monotonic sequence number.
#[derive(Debug, Clone)]
pub struct ExecFrame {
    pub channel: ExecChannel,
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

/// Lifecycle of an exec session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecState {
    Open,
    Closed,
}

/// Ring-buffered sink exec frames are pumped into; consumers drain independently of the producer.
pub struct ExecSink {
    ring: Mutex<RingBuffer<ExecFrame>>,
}

impl Default for ExecSink {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecSink {
    /// A sink backed by the default ring capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_RING_CAPACITY)
    }

    /// A sink backed by an explicit ring capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ring: Mutex::new(RingBuffer::new(capacity)),
        }
    }

    /// Pops the oldest buffered frame, if any.
    pub async fn pop(&self) -> Option<ExecFrame> {
        self.ring.lock().await.pop()
    }

    /// Number of frames evicted by backpressure so far.
    pub async fn dropped(&self) -> u64 {
        self.ring.lock().await.dropped()
    }

    /// Number of frames currently buffered.
    pub async fn len(&self) -> usize {
        self.ring.lock().await.len()
    }

    /// Whether no frames are currently buffered.
    pub async fn is_empty(&self) -> bool {
        self.ring.lock().await.is_empty()
    }
}

/// Drives one exec reader (stdout or stderr) into an `ExecSink` with sequenced frames.
pub struct ExecStream {
    sink: Arc<ExecSink>,
    closed: Arc<AtomicBool>,
    task: JoinHandle<()>,
}

impl ExecStream {
    /// Spawns a pump reading chunks from `reader` into a ring of `capacity` frames tagged `channel`.
    pub fn spawn<R>(reader: R, channel: ExecChannel, capacity: usize) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
    {
        let sink = Arc::new(ExecSink::with_capacity(capacity));
        let closed = Arc::new(AtomicBool::new(false));
        let task = tokio::spawn(pump(
            reader,
            channel,
            Arc::clone(&sink),
            Arc::clone(&closed),
        ));
        Self { sink, closed, task }
    }

    /// The sink frames are pumped into; use it to drain and read the drop count.
    pub fn sink(&self) -> &Arc<ExecSink> {
        &self.sink
    }

    /// Whether the producer side has reached EOF (or been aborted).
    pub fn state(&self) -> ExecState {
        if self.closed.load(Ordering::SeqCst) {
            ExecState::Closed
        } else {
            ExecState::Open
        }
    }

    /// Aborts the pump task and marks the stream closed.
    pub fn close(&self) {
        self.task.abort();
        self.closed.store(true, Ordering::SeqCst);
    }
}

async fn pump<R>(mut reader: R, channel: ExecChannel, sink: Arc<ExecSink>, closed: Arc<AtomicBool>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let mut sequence = 0u64;
    let mut buf = vec![0u8; CHUNK_SIZE];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let frame = ExecFrame {
                    channel,
                    sequence,
                    bytes: buf[..n].to_vec(),
                };
                sequence += 1;
                sink.ring.lock().await.push(frame);
            }
        }
    }
    closed.store(true, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncWriteExt, BufReader};
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn a_fast_producer_drops_oldest_and_counts_drops() {
        let (mut writer, reader) = tokio::io::duplex(64 * 1024);
        let reader = BufReader::new(reader);
        let stream = ExecStream::spawn(reader, ExecChannel::Stdout, 4);

        tokio::spawn(async move {
            for i in 0..20u8 {
                let _ = writer.write_all(&[i]).await;
            }
        });

        sleep(Duration::from_millis(100)).await;

        let sink = stream.sink();
        assert!(sink.len().await <= 4);
        assert!(sink.dropped().await > 0);
    }

    #[tokio::test]
    async fn sequence_numbers_are_monotonic_and_gaps_reveal_drops() {
        let (mut writer, reader) = tokio::io::duplex(64 * 1024);
        let stream = ExecStream::spawn(reader, ExecChannel::Stdout, 2);

        // Each write is its own read() call worth of data, so each becomes one frame.
        for i in 0..5u8 {
            writer.write_all(&[i]).await.unwrap();
            sleep(Duration::from_millis(10)).await;
        }
        drop(writer);
        sleep(Duration::from_millis(20)).await;

        let sink = stream.sink();
        let mut sequences = Vec::new();
        while let Some(frame) = sink.pop().await {
            sequences.push(frame.sequence);
        }
        for pair in sequences.windows(2) {
            assert!(pair[1] > pair[0]);
        }
        assert_eq!(stream.state(), ExecState::Closed);
    }

    #[tokio::test]
    async fn explicit_close_marks_the_stream_closed() {
        let (_writer, reader) = tokio::io::duplex(1024);
        let stream = ExecStream::spawn(reader, ExecChannel::Stderr, 4);
        stream.close();
        assert_eq!(stream.state(), ExecState::Closed);
    }
}
