//! `openkite-plugin-sdk` — the contract between OpenKite core and plugins.
//!
//! This crate is a **stable public API**: breaking changes require a major
//! version bump. Plugins depend on it and never reach into core internals;
//! core never assumes a specific plugin exists.
//!
//! See `docs/plugin-architecture.md` for the plugin strategy (static-first).
//!
//! # Semver policy
//!
//! - **Patch** (`0.x.0`): bug fixes that don't change the public API.
//! - **Minor** (`0.0.x`): additive, non-breaking API (new methods, new types).
//! - **Major** (`x.0.0`): breaking change — removed/renamed items or changed
//!   signatures. All plugins must be recompiled against the new SDK.
//!
//! Plugins record the SDK version they were built against; the host checks it
//! at load time (dylib) or compile time (static).
//!
//! # Example
//!
//! Implement [`OpenKitePlugin`] to add a plugin:
//!
//! ```no_run
//! use openkite_plugin_sdk::anyhow::Result;
//! use openkite_plugin_sdk::{
//!     OpenKitePlugin, PluginContext, PluginIcon, PluginMeta, PluginRoute, SidebarSection,
//! };
//!
//! pub struct MyPlugin;
//!
//! impl OpenKitePlugin for MyPlugin {
//!     fn metadata(&self) -> PluginMeta {
//!         PluginMeta {
//!             name: "my-plugin".into(),
//!             display_name: "My Plugin".into(),
//!             version: "0.1.0".into(),
//!             author: "me".into(),
//!             icon: PluginIcon::BuiltIn("cube"),
//!             accent_color: None,
//!         }
//!     }
//!     fn on_cluster_connect(&mut self, _ctx: &PluginContext) -> Result<()> {
//!         Ok(())
//!     }
//!     fn on_cluster_disconnect(&mut self) {}
//!     fn sidebar_entries(&self) -> Vec<SidebarSection> {
//!         vec![]
//!     }
//!     fn routes(&self) -> Vec<PluginRoute> {
//!         vec![]
//!     }
//!     fn on_unload(&mut self) {}
//! }
//! ```

pub mod context;
pub mod meta;
pub mod plugin;
pub mod route;
pub mod sidebar;

pub use anyhow;
pub use context::{PluginContext, PluginUiHandle, ThemeReadHandle};
pub use meta::{PluginIcon, PluginMeta};
pub use plugin::OpenKitePlugin;
pub use route::PluginRoute;
pub use sidebar::{SidebarEntry, SidebarSection};
