//! Dioxus view components wired to the router.
//!
//! OKT-127 keep-native — these three modules remain native RSX on purpose.
//! See `crates/openkite-desktop/src/router.rs` (module doc + `console_route`)
//! for the full rationale. In short:
//!
//! - `openkite-ui` exports no log viewer, terminal exec view, or pod-detail
//!   inspector. The crate-rendered console (`openkite-web::App`) currently
//!   paints only the capabilities summary.
//! - Retiring these would delete a working user surface, not consolidate
//!   one. Each stays until a successor ticket ports its surface to the
//!   console crate.
pub mod logs;
pub mod pod_detail;
pub mod terminal;
