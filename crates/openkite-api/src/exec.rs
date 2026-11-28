//! Pod exec wire contract: the commands a shell sends and the events it gets back.
//!
//! These types carry no cluster or socket state — they are what a shell (desktop, web, or a
//! future plugin host) exchanges with whatever bridges it to an `openkite_engine` exec session.
use serde::{Deserialize, Serialize};

/// Which output channel a chunk of exec bytes arrived on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellChannel {
    Stdout,
    Stderr,
}

/// Opens an exec session against one container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecOpen {
    pub namespace: String,
    pub pod: String,
    pub container: Option<String>,
    pub command: Vec<String>,
    pub tty: bool,
    pub cols: u16,
    pub rows: u16,
}

impl ExecOpen {
    /// A tty-less session running `command` in `pod`'s sole container.
    pub fn new(namespace: impl Into<String>, pod: impl Into<String>, command: Vec<String>) -> Self {
        Self {
            namespace: namespace.into(),
            pod: pod.into(),
            container: None,
            command,
            tty: false,
            cols: 80,
            rows: 24,
        }
    }

    /// Allocates a tty sized to `cols`x`rows`, which also makes the session resizable.
    #[must_use]
    pub fn with_tty(mut self, cols: u16, rows: u16) -> Self {
        self.tty = true;
        self.cols = cols;
        self.rows = rows;
        self
    }
}

/// A terminal size update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecResize {
    pub cols: u16,
    pub rows: u16,
}

/// A command a connected shell sends toward the pod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecCommand {
    Open(ExecOpen),
    Stdin(Vec<u8>),
    Resize(ExecResize),
    /// The shell is leaving; the bridge should tear the session down on its side too.
    Detach,
}

/// How a session ended, mirroring `openkite_engine::driver::ExecExit` without pulling kube
/// into this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecExitCode {
    Code(i32),
    Unknown,
}

/// An event a connected shell receives from the bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecEvent {
    Output {
        channel: ShellChannel,
        sequence: u64,
        bytes: Vec<u8>,
    },
    /// Frames the bridge's ring buffer evicted under backpressure before they reached here.
    Dropped {
        channel: ShellChannel,
        count: u64,
    },
    Exit(ExecExitCode),
    Error(String),
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(value: T)
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(&value).expect("serialize");
        let back: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(value, back);
    }

    #[test]
    fn exec_open_defaults_to_no_tty_and_an_80x24_size() {
        let open = ExecOpen::new("ns", "pod", vec!["sh".to_owned()]);
        assert!(!open.tty);
        assert_eq!((open.cols, open.rows), (80, 24));
    }

    #[test]
    fn with_tty_sets_the_requested_size() {
        let open = ExecOpen::new("ns", "pod", vec!["sh".to_owned()]).with_tty(133, 41);
        assert!(open.tty);
        assert_eq!((open.cols, open.rows), (133, 41));
    }

    #[test]
    fn every_command_variant_round_trips() {
        round_trip(ExecCommand::Open(ExecOpen::new(
            "ns",
            "pod",
            vec!["sh".to_owned()],
        )));
        round_trip(ExecCommand::Stdin(b"ls\n".to_vec()));
        round_trip(ExecCommand::Resize(ExecResize {
            cols: 100,
            rows: 30,
        }));
        round_trip(ExecCommand::Detach);
    }

    #[test]
    fn every_event_variant_round_trips() {
        round_trip(ExecEvent::Output {
            channel: ShellChannel::Stdout,
            sequence: 3,
            bytes: b"ok\n".to_vec(),
        });
        round_trip(ExecEvent::Dropped {
            channel: ShellChannel::Stderr,
            count: 2,
        });
        round_trip(ExecEvent::Exit(ExecExitCode::Code(7)));
        round_trip(ExecEvent::Exit(ExecExitCode::Unknown));
        round_trip(ExecEvent::Error("pod not found".to_owned()));
        round_trip(ExecEvent::Closed);
    }
}
