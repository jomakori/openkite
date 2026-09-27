//! Terminal helpers: shell resolution and coalesced output buffering.

/// Resolve the user's shell: `$SHELL`, falling back to `cmd` (Windows) or
/// `sh` (Unix) when unset or empty.
pub fn resolve_shell(shell_env: Option<&str>, is_windows: bool) -> String {
    shell_env
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if is_windows {
                "cmd".to_string()
            } else {
                "sh".to_string()
            }
        })
}

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
