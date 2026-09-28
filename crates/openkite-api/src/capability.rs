//! Host capability descriptor (OKT-126): the console renders only the surfaces
//! the host reports, instead of rendering one that then fails.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayKind {
    InProcess,
    ServerSide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub gateway: GatewayKind,
    pub plugins: bool,
    pub terminal: bool,
    pub exec: bool,
    pub native_menu_bar: bool,
    pub title_bar_override: bool,
    pub file_dialogs: bool,
}

impl Capabilities {
    pub fn in_process() -> Self {
        Self {
            gateway: GatewayKind::InProcess,
            plugins: true,
            terminal: true,
            exec: false,
            native_menu_bar: true,
            title_bar_override: true,
            file_dialogs: true,
        }
    }

    pub fn server_side() -> Self {
        Self {
            gateway: GatewayKind::ServerSide,
            plugins: false,
            terminal: false,
            exec: false,
            native_menu_bar: false,
            title_bar_override: false,
            file_dialogs: false,
        }
    }

    pub fn minimal() -> Self {
        Self {
            gateway: GatewayKind::ServerSide,
            plugins: false,
            terminal: false,
            exec: false,
            native_menu_bar: false,
            title_bar_override: false,
            file_dialogs: false,
        }
    }

    pub fn supports_terminal(&self) -> bool {
        self.terminal
    }

    pub fn supports_exec(&self) -> bool {
        self.exec
    }

    pub fn supports_plugins(&self) -> bool {
        self.plugins
    }

    pub fn supports_native_menu_bar(&self) -> bool {
        self.native_menu_bar
    }

    pub fn supports_title_bar_override(&self) -> bool {
        self.title_bar_override
    }

    pub fn supports_file_dialogs(&self) -> bool {
        self.file_dialogs
    }
}
