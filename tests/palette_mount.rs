//! Headless mounts of the palette chrome (coverage roadmap B5).
//! tests/palette.rs pins the registry/filter/cursor/global-signal surface;
//! these run the `PaletteKeybind` + `CommandPalette` zero-state render
//! bodies (overlay closed, nothing mounted). Opening the panel needs the
//! `use_navigator()` router context, so the panel body stays covered by the
//! desktop E2E — mounting it headless without a router would panic.

mod support;

use dioxus::prelude::*;
use openkite::palette::{CommandPalette, PaletteKeybind};

fn keybind() -> Element {
    rsx! {
        PaletteKeybind {}
        div { "keybind-mounted" }
    }
}

fn closed_palette() -> Element {
    rsx! {
        CommandPalette {}
        div { "shell" }
    }
}

#[test]
fn palette_keybind_mounts_without_panicking() {
    let html = support::mount_html(keybind, || {});
    assert!(html.contains("keybind-mounted"), "got: {html}");
}

#[test]
fn command_palette_closed_renders_nothing() {
    let html = support::mount_html(closed_palette, || {});
    assert!(html.contains("shell"), "got: {html}");
    assert!(!html.contains("palette"), "got: {html}");
}
