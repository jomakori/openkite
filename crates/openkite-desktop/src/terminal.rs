//! Terminal host helpers: coalesced output buffering.
//!
//! The surface itself lives in `openkite_ui::components::terminal` (the shell
//! it execs into, the picker and the xterm.js mount point); what stays here is
//! the PTY-side buffer the host owns.

/// Coalesces terminal output into bounded chunks for the eval bridge.
///
/// The PTY reader appends small writes; a timer tick drains a chunk (at most
/// `chunk_size` bytes) so the JS bridge never receives an unbounded payload.
pub struct OutputBuffer {
    buf: Vec<u8>,
    chunk_size: usize,
}

impl OutputBuffer {
    /// A buffer emitting chunks of at most `chunk_size` bytes.
    pub fn new(chunk_size: usize) -> Self {
        Self {
            buf: Vec::new(),
            chunk_size,
        }
    }

    /// Append output from the PTY.
    pub fn push(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// Drain up to one chunk; `None` when empty.
    pub fn next_chunk(&mut self) -> Option<Vec<u8>> {
        if self.buf.is_empty() {
            return None;
        }
        let n = self.buf.len().min(self.chunk_size);
        Some(self.buf.drain(..n).collect())
    }

    /// Drain everything remaining.
    pub fn flush(&mut self) -> Option<Vec<u8>> {
        if self.buf.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.buf))
        }
    }
}
