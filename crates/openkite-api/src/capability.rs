//! Host capability descriptor (OKT-126): the console renders only the surfaces
//! the host reports, instead of rendering one that then fails.

use serde::{Deserialize, Serialize};

/// Where the gateway is answered from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayKind {
    /// The desktop host, talking to the cluster in-process over kube-rs.
    InProcess,
    /// The browser host, whose gateway is answered server-side.
    ServerSide,
}

/// What one host can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Which side of the process boundary answers the gateway.
    pub gateway: GatewayKind,
    /// Whether the host evaluates JS plugin bundles and serves the bridge.
    pub plugins: bool,
    /// Whether the host can host an embedded terminal.
    pub terminal: bool,
    /// Whether the host implements `ApiRequest::Exec`.
    pub exec: bool,
}

impl Capabilities {
    /// The desktop host: kube-rs in-process, plugin bridge and terminal
    /// present, `exec` still a bridge placeholder.
    pub fn in_process() -> Self {
        Self {
            gateway: GatewayKind::InProcess,
            plugins: true,
            terminal: true,
            exec: false,
        }
    }

    /// The browser host: the gateway is answered server-side and there is no
    /// JS plugin host or embedded terminal in the browser.
    pub fn server_side() -> Self {
        Self {
            gateway: GatewayKind::ServerSide,
            plugins: false,
            terminal: false,
            exec: false,
        }
    }
}
