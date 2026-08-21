//! `openkite-plugin-sdk` — the contract between OpenKite core and plugins.
//!
//! This crate is a **stable public API**: breaking changes require a major
//! version bump. Plugins depend on it and never reach into core internals;
//! core never assumes a specific plugin exists.
//!
//! See `docs/plugin-architecture.md` for the plugin strategy (static-first).

pub mod context;
pub mod meta;
pub mod plugin;
pub mod route;
pub mod sidebar;

pub use context::{PluginContext, PluginUiHandle, ThemeReadHandle};
pub use meta::{PluginIcon, PluginMeta};
pub use plugin::OpenKitePlugin;
pub use route::PluginRoute;
pub use sidebar::{SidebarEntry, SidebarSection};
