//! Integration tests for the log streaming module.

use openkite::logs::{FollowState, LineBuffer, LogOptions, MAX_LINES};

#[test]
fn line_buffer_appends_and_reads_back() {
    let mut buf = LineBuffer::default();
    buf.push("first");
    buf.push("second");
    assert_eq!(buf.len(), 2);
    assert_eq!(buf.lines(), &["first".to_string(), "second".to_string()]);
}

#[test]
fn line_buffer_caps_and_drops_oldest() {
    let mut buf = LineBuffer::default();
    for i in 0..(MAX_LINES + 5) {
        buf.push(format!("line {i}"));
    }
    assert_eq!(buf.len(), MAX_LINES);
    assert_eq!(buf.lines()[0], "line 5");
    assert_eq!(
        buf.lines()[MAX_LINES - 1],
        format!("line {}", MAX_LINES + 4)
    );
}

#[test]
fn line_buffer_clear_empties() {
    let mut buf = LineBuffer::default();
    buf.push("a");
    buf.push("b");
    buf.clear();
    assert!(buf.is_empty());
}

#[test]
fn follow_state_pause_resume() {
    let mut state = FollowState::Following;
    assert!(state.is_following());
    state.pause();
    assert_eq!(state, FollowState::Paused);
    assert!(!state.is_following());
    state.resume();
    assert!(state.is_following());
}

#[test]
fn log_options_map_to_kube_params() {
    let opts = LogOptions {
        container: Some("sidecar".to_string()),
        follow: true,
        tail_lines: Some(100),
        timestamps: true,
    };
    let params = opts.to_params();
    assert_eq!(params.container.as_deref(), Some("sidecar"));
    assert!(params.follow);
    assert_eq!(params.tail_lines, Some(100));
    assert!(params.timestamps);
}
