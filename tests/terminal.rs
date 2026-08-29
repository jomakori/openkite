//! Integration tests for terminal helpers.

use openkite::terminal::{resolve_shell, OutputBuffer};

#[test]
fn resolve_shell_prefers_env() {
    assert_eq!(resolve_shell(Some("/bin/zsh"), false), "/bin/zsh");
}

#[test]
fn resolve_shell_falls_back_by_platform() {
    assert_eq!(resolve_shell(None, false), "sh");
    assert_eq!(resolve_shell(None, true), "cmd");
    assert_eq!(resolve_shell(Some(""), false), "sh");
}

#[test]
fn next_chunk_respects_chunk_size() {
    let mut buf = OutputBuffer::new(8);
    // 16 bytes: two full chunks.
    buf.push(b"abcdefghijklmnop");
    assert_eq!(buf.next_chunk().unwrap(), b"abcdefgh");
    assert_eq!(buf.next_chunk().unwrap(), b"ijklmnop");
    assert!(buf.next_chunk().is_none());
}

#[test]
fn partial_chunk_drains_below_chunk_size() {
    let mut buf = OutputBuffer::new(4);
    buf.push(b"ab");
    assert_eq!(buf.next_chunk().unwrap(), b"ab");
    assert!(buf.next_chunk().is_none());
}

#[test]
fn flush_returns_remaining_and_clears() {
    let mut buf = OutputBuffer::new(8);
    buf.push(b"abc");
    assert_eq!(buf.flush().unwrap(), b"abc");
    assert!(buf.flush().is_none());
}
