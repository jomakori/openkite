//! Dioxus view components wired to the router.
//!
//! One module remains native RSX: `terminal` (xterm.js host — the crate has
//! no replacement). The standalone log viewer is
//! `openkite_ui::components::logs::LogsView`, streamed into the shared buffer
//! by the desktop host (see `router::Logs`), and the pod detail slide-over is
//! `openkite_ui::components::pod_detail::PodDetail` (see `router::AppShell`).

pub mod terminal;
