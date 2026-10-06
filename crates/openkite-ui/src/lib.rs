//! The OpenKite console, once: the components, design tokens and shell chrome
//! every host renders.
//!
//! Nothing host-shaped lives here. No `wry`, no `axum`, no `web-sys`, no
//! `js_sys`, no tokio — a host-only dependency in this crate's graph is what
//! would make the wasm32 build impossible, so `wasm-ui` in lint-test.yml
//! type-checks the crate for `wasm32-unknown-unknown` on every PR.
//!
//! Data reaches the console two ways: through the globals in [`runtime`] (the
//! host publishes them at boot and on connect) and through [`runtime::gateway`]
//! for anything that must be fetched or mutated on demand.

#[cfg(not(target_arch = "wasm32"))]
pub mod assets;
pub mod components;
pub mod design;
pub mod dock;
pub mod fuzzy;
pub mod plugin_api;
pub mod runtime;
pub mod secrets;
pub mod shell;
pub mod theme;
pub mod theme_catalog;
pub mod theme_opaline;
pub mod touch;

/// The shell stylesheet, so a host renders the console with the design the
/// tokens in [`design`] describe.
#[cfg(not(target_arch = "wasm32"))]
pub const MAIN_CSS: &str = include_str!("../assets/main.css");
