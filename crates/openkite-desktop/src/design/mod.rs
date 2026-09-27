//! Design system: the typed Rust view of the `assets/main.css` contract.
//!
//! Theme tokens (opaline-mapped) live in [`crate::theme`] — they supply the
//! color palette. This module supplies the surface treatment built on top:
//! the frost blur radii, panel radii, and shadow strings that Dioxus
//! `style:` attributes and other Rust-side code can reference by name
//! instead of scattering raw `40px` / `0 1px 3px ...` literals.
//!
//! Pair with the primitive CSS classes in `assets/main.css` (`.panel`,
//! `.btn`, `.pill`, `.log-panel`, `.inspector`, `.toast`, …). Interactive
//! `#[component]` wrappers for each primitive live in the consuming view
//! ticket — this module is pure constants.

#![allow(dead_code)]

pub mod tokens;

pub use tokens::{
    BLUR_FROST, BLUR_PANEL, BLUR_TOPBAR, R_MD, R_PILL, R_SM, SHADOW_HOVER, SHADOW_REST,
    SHADOW_TERMINAL,
};
