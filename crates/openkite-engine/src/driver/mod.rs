//! Kube client drivers: exec/attach, logs, port-forward, and exec credential plugins.
pub mod bridge;
pub mod error;
mod exec;

pub use bridge::ShellBridge;
pub use exec::{ExecExit, PodExecRequest, PodExecSession};
