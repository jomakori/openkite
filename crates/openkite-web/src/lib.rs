//! OpenKite web host: one axum process that renders the console root, answers
//! the console's kube bridge over HTTP, and serves the bundle the native half
//! of the crate is built from.
//!
//! The console's data path is already HTTP. `web/src/bridge.ts` POSTs the
//! `{id, plugin, request}` envelope to `/openkite` and falls back to its bundled
//! fixtures only when that request cannot be made at all — a static host answers
//! 404, which is why the browser build renders fixture data today. This crate is
//! the missing half: the same [`Bridge`](openkite_host::bridge::Bridge) dispatch
//! the desktop webview reaches through its wry asset handler, mounted on axum
//! instead, with the console rendered from [`ssr`] rather than served as a
//! prebuilt file.
//!
//! Two targets share the crate. The native modules — the axum host (`host`,
//! `routes`), the reflectors' runtime (`headless`) and the console's
//! context/settings ops (`spike`) — need kube and axum. The render path
//! ([`app`], [`ssr`], [`client`]) and the wasm client binary behind the
//! `hydrate` feature have to compile for `wasm32-unknown-unknown`. Keeping the
//! two apart is what makes a crate-rendered, hydrating console possible at all.

pub mod app;
pub mod client;
pub mod ssr;

#[cfg(not(target_arch = "wasm32"))]
mod host;

#[cfg(not(target_arch = "wasm32"))]
pub use host::{bind_and_serve, connect, in_cluster, serve, DEFAULT_ADDR, DEFAULT_WEB_ROOT};

#[cfg(not(target_arch = "wasm32"))]
pub mod headless;
#[cfg(not(target_arch = "wasm32"))]
pub mod routes;
#[cfg(not(target_arch = "wasm32"))]
pub mod spike;
