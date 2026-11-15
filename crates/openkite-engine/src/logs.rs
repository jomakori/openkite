//! Pod log stream: pumps lines into a drop-oldest ring with an observable backpressure counter.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufRead, AsyncBufReadExt};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::ring::{RingBuffer, DEFAULT_RING_CAPACITY};

/// Lifecycle of a log stream's background pump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStreamState {
    Open,
    Closed,
}

/// Ring-buffered sink lines are pumped into; consumers drain independently of the producer.
pub struct LogSink {
    ring: Mutex<RingBuffer<String>>,
}

impl Default for LogSink {
    fn default() -> Self {
        Self::new()
    }
}

impl LogSink {
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

    /// Pops the oldest buffered line, if any.
    pub async fn pop(&self) -> Option<String> {
        self.ring.lock().await.pop()
    }

    /// Number of lines evicted by backpressure so far.
    pub async fn dropped(&self) -> u64 {
        self.ring.lock().await.dropped()
    }

    /// Number of lines currently buffered.
    pub async fn len(&self) -> usize {
        self.ring.lock().await.len()
    }

    /// Whether no lines are currently buffered.
    pub async fn is_empty(&self) -> bool {
        self.ring.lock().await.is_empty()
    }
}

/// Drives a pod log source into a `LogSink`, tracking open/closed state.
pub struct LogStream {
    sink: Arc<LogSink>,
    closed: Arc<AtomicBool>,
    task: JoinHandle<()>,
}

impl LogStream {
    /// Spawns a pump reading newline-delimited bytes from `reader` into a ring of `capacity` lines.
    pub fn spawn<R>(reader: R, capacity: usize) -> Self
    where
        R: AsyncBufRead + Unpin + Send + 'static,
    {
        let sink = Arc::new(LogSink::with_capacity(capacity));
        let closed = Arc::new(AtomicBool::new(false));
        let task = tokio::spawn(pump(reader, Arc::clone(&sink), Arc::clone(&closed)));
        Self { sink, closed, task }
    }

    /// The sink lines are pumped into; use it to drain and read the drop count.
    pub fn sink(&self) -> &Arc<LogSink> {
        &self.sink
    }

    /// Whether the producer side has reached EOF (or been aborted).
    pub fn state(&self) -> LogStreamState {
        if self.closed.load(Ordering::SeqCst) {
            LogStreamState::Closed
        } else {
            LogStreamState::Open
        }
    }

    /// Aborts the pump task and marks the stream closed.
    pub fn close(&self) {
        self.task.abort();
        self.closed.store(true, Ordering::SeqCst);
    }
}

async fn pump<R>(reader: R, sink: Arc<LogSink>, closed: Arc<AtomicBool>)
where
    R: AsyncBufRead + Unpin + Send + 'static,
{
    let mut lines = reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        sink.ring.lock().await.push(line);
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
        let stream = LogStream::spawn(reader, 4);

        let produced = 20usize;
        tokio::spawn(async move {
            for i in 0..produced {
                let _ = writer.write_all(format!("line-{i}\n").as_bytes()).await;
            }
        });

        // Give the pump time to drain the writer without draining the sink itself.
        sleep(Duration::from_millis(100)).await;

        let sink = stream.sink();
        assert!(sink.len().await <= 4);
        assert!(sink.dropped().await > 0);
    }

    #[tokio::test]
    async fn eof_closes_the_stream() {
        let (writer, reader) = tokio::io::duplex(1024);
        let reader = BufReader::new(reader);
        let stream = LogStream::spawn(reader, 4);
        drop(writer);

        sleep(Duration::from_millis(50)).await;
        assert_eq!(stream.state(), LogStreamState::Closed);
    }

    #[tokio::test]
    async fn explicit_close_marks_the_stream_closed() {
        let (_writer, reader) = tokio::io::duplex(1024);
        let reader = BufReader::new(reader);
        let stream = LogStream::spawn(reader, 4);
        stream.close();
        assert_eq!(stream.state(), LogStreamState::Closed);
    }
}
