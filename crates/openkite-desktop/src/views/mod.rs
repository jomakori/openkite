//! Dioxus view components wired to the router.
//!
//! Two modules remain native RSX: `terminal` (xterm.js host — the crate has
//! no replacement) and `pod_detail` (5-tab inspector — the OKT-136 umbrella
//! is building its console-side counterpart). The standalone log viewer is
//! now `openkite_ui::components::logs::LogsView`, streamed into the shared
//! buffer by the desktop host (see `router::Logs`).
pub mod pod_detail;
pub mod terminal;
