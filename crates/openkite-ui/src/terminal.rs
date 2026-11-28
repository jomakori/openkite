//! Cluster shell state: one pod exec session rendered as a terminal pane.
//!
//! This mirrors `logs.rs`'s split between state and render: nothing here touches a socket.
//! A caller feeds it `ExecEvent`s already read off the bridge and drains the `ExecCommand`s
//! this emits back out onto the wire.

use openkite_api::exec::{
    ExecCommand, ExecEvent, ExecExitCode, ExecOpen, ExecResize, ShellChannel,
};

/// Lifecycle of a shell pane's session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalStatus {
    /// Open request sent, no output yet.
    Connecting,
    /// The bridge is streaming.
    Open,
    /// The remote command ended; `Code(0)` is success.
    Exited(ExecExitCode),
    /// The bridge reported an error; the pane shows it and stops accepting input.
    Errored(String),
    /// The shell detached and the bridge tore the session down.
    Closed,
}

/// One rendered chunk of terminal output, tagged by channel so stderr can be styled apart
/// from stdout without re-parsing bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalChunk {
    pub channel: ShellChannel,
    pub text: String,
}

/// One pod exec pane: what it is attached to, its live buffer, and whether it is still open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSession {
    pub namespace: String,
    pub pod: String,
    pub container: Option<String>,
    pub cols: u16,
    pub rows: u16,
    pub status: TerminalStatus,
    pub chunks: Vec<TerminalChunk>,
    pub dropped: u64,
}

impl TerminalSession {
    /// A pane about to open a command in `pod`, sized to `cols`x`rows`.
    pub fn new(namespace: impl Into<String>, pod: impl Into<String>, cols: u16, rows: u16) -> Self {
        Self {
            namespace: namespace.into(),
            pod: pod.into(),
            container: None,
            cols,
            rows,
            status: TerminalStatus::Connecting,
            chunks: Vec::new(),
            dropped: 0,
        }
    }

    /// Targets a container other than the pod's sole one.
    #[must_use]
    pub fn with_container(mut self, container: impl Into<String>) -> Self {
        self.container = Some(container.into());
        self
    }

    /// The open request this pane needs sent to start its session.
    pub fn open_request(&self, command: Vec<String>) -> ExecOpen {
        let mut open = ExecOpen::new(self.namespace.clone(), self.pod.clone(), command)
            .with_tty(self.cols, self.rows);
        open.container = self.container.clone();
        open
    }

    /// Whether the pane will still accept typed input.
    pub fn is_live(&self) -> bool {
        matches!(
            self.status,
            TerminalStatus::Connecting | TerminalStatus::Open
        )
    }

    /// Applies one bridge event, returning whether the pane's rendered state changed.
    pub fn apply_event(&mut self, event: ExecEvent) -> bool {
        match event {
            ExecEvent::Output { channel, bytes, .. } => {
                self.status = TerminalStatus::Open;
                let text = String::from_utf8_lossy(&bytes).into_owned();
                match self.chunks.last_mut() {
                    Some(chunk) if chunk.channel == channel => chunk.text.push_str(&text),
                    _ => self.chunks.push(TerminalChunk { channel, text }),
                }
                true
            }
            ExecEvent::Dropped { count, .. } => {
                self.dropped += count;
                true
            }
            ExecEvent::Exit(code) => {
                self.status = TerminalStatus::Exited(code);
                true
            }
            ExecEvent::Error(message) => {
                self.status = TerminalStatus::Errored(message);
                true
            }
            ExecEvent::Closed => {
                self.status = TerminalStatus::Closed;
                true
            }
        }
    }

    /// Turns one line of typed input into a `Stdin` command. `None` once the pane is no
    /// longer live, so a stale keystroke after exit/detach cannot be sent.
    pub fn send_line(&self, line: &str) -> Option<ExecCommand> {
        if !self.is_live() {
            return None;
        }
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\n');
        Some(ExecCommand::Stdin(bytes))
    }

    /// A resize command for a new size, updating the pane's own record of it.
    pub fn resize(&mut self, cols: u16, rows: u16) -> ExecCommand {
        self.cols = cols;
        self.rows = rows;
        ExecCommand::Resize(ExecResize { cols, rows })
    }

    /// Detaches the pane: marks it closed locally and returns the wire command to send.
    pub fn detach(&mut self) -> ExecCommand {
        self.status = TerminalStatus::Closed;
        ExecCommand::Detach
    }

    /// Plain concatenated text across every chunk, in arrival order -- what the pane renders.
    pub fn text(&self) -> String {
        self.chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_tags_the_session_tty_sized_to_the_pane() {
        let session = TerminalSession::new("ns", "pod", 120, 32);
        let open = session.open_request(vec!["sh".to_owned()]);
        assert!(open.tty);
        assert_eq!((open.cols, open.rows), (120, 32));
    }

    #[test]
    fn output_events_move_the_pane_into_open_and_append_text() {
        let mut session = TerminalSession::new("ns", "pod", 80, 24);
        assert_eq!(session.status, TerminalStatus::Connecting);
        session.apply_event(ExecEvent::Output {
            channel: ShellChannel::Stdout,
            sequence: 0,
            bytes: b"hi".to_vec(),
        });
        assert_eq!(session.status, TerminalStatus::Open);
        assert_eq!(session.text(), "hi");
    }

    #[test]
    fn same_channel_output_coalesces_into_one_chunk() {
        let mut session = TerminalSession::new("ns", "pod", 80, 24);
        session.apply_event(ExecEvent::Output {
            channel: ShellChannel::Stdout,
            sequence: 0,
            bytes: b"a".to_vec(),
        });
        session.apply_event(ExecEvent::Output {
            channel: ShellChannel::Stdout,
            sequence: 1,
            bytes: b"b".to_vec(),
        });
        assert_eq!(session.chunks.len(), 1);
        assert_eq!(session.text(), "ab");
    }

    #[test]
    fn channel_switch_starts_a_new_chunk() {
        let mut session = TerminalSession::new("ns", "pod", 80, 24);
        session.apply_event(ExecEvent::Output {
            channel: ShellChannel::Stdout,
            sequence: 0,
            bytes: b"out".to_vec(),
        });
        session.apply_event(ExecEvent::Output {
            channel: ShellChannel::Stderr,
            sequence: 1,
            bytes: b"err".to_vec(),
        });
        assert_eq!(session.chunks.len(), 2);
        assert_eq!(session.chunks[1].channel, ShellChannel::Stderr);
    }

    #[test]
    fn dropped_events_accumulate_without_touching_text() {
        let mut session = TerminalSession::new("ns", "pod", 80, 24);
        session.apply_event(ExecEvent::Dropped {
            channel: ShellChannel::Stdout,
            count: 3,
        });
        session.apply_event(ExecEvent::Dropped {
            channel: ShellChannel::Stdout,
            count: 2,
        });
        assert_eq!(session.dropped, 5);
        assert_eq!(session.text(), "");
    }

    #[test]
    fn exit_error_and_closed_stop_the_pane_accepting_input() {
        let mut exited = TerminalSession::new("ns", "pod", 80, 24);
        exited.apply_event(ExecEvent::Exit(ExecExitCode::Code(0)));
        assert!(!exited.is_live());

        let mut errored = TerminalSession::new("ns", "pod", 80, 24);
        errored.apply_event(ExecEvent::Error("boom".to_owned()));
        assert!(!errored.is_live());
        assert_eq!(errored.status, TerminalStatus::Errored("boom".to_owned()));

        let mut closed = TerminalSession::new("ns", "pod", 80, 24);
        closed.apply_event(ExecEvent::Closed);
        assert!(!closed.is_live());
    }

    #[test]
    fn send_line_appends_a_newline_and_wraps_stdin() {
        let session = TerminalSession::new("ns", "pod", 80, 24);
        let command = session.send_line("ls").expect("live pane sends");
        assert_eq!(command, ExecCommand::Stdin(b"ls\n".to_vec()));
    }

    #[test]
    fn send_line_is_a_noop_once_the_pane_is_no_longer_live() {
        let mut session = TerminalSession::new("ns", "pod", 80, 24);
        session.apply_event(ExecEvent::Closed);
        assert!(session.send_line("ls").is_none());
    }

    #[test]
    fn resize_updates_the_panes_own_size_and_emits_the_command() {
        let mut session = TerminalSession::new("ns", "pod", 80, 24);
        let command = session.resize(133, 41);
        assert_eq!(
            command,
            ExecCommand::Resize(ExecResize {
                cols: 133,
                rows: 41
            })
        );
        assert_eq!((session.cols, session.rows), (133, 41));
    }

    #[test]
    fn detach_closes_the_pane_locally_and_emits_the_command() {
        let mut session = TerminalSession::new("ns", "pod", 80, 24);
        let command = session.detach();
        assert_eq!(command, ExecCommand::Detach);
        assert_eq!(session.status, TerminalStatus::Closed);
    }
}
