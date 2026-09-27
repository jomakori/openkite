//! Headless component-mount harness (coverage roadmap B5).
//!
//! Mounts a zero-prop `fn() -> Element` root in a throwaway VirtualDom and
//! rebuilds it for real — hooks/signals run, no desktop webview. Same
//! `VirtualDom::new` shape as the `with_runtime` helper in tests/palette.rs,
//! but the root component actually renders via `rebuild_in_place` (the
//! documented headless render path; mutations are discarded).
//!
//! `setup` runs first inside the vdom runtime, mirroring how `lib::run`
//! publishes global signals before the UI launches. That lets a test seed
//! CRUD_TARGET / SELECTED_SECRET / SWITCHER_* / context state before the
//! first render.
//!
//! The mounted tree is snapshotted with `dioxus_ssr::Renderer` so tests can
//! assert on the emitted text/DOM, not just "didn't panic".

use dioxus::prelude::*;

/// Mount `app` headless and return its rendered HTML.
///
/// Panics if any component in the tree panics (that IS the no-panic
/// assertion). `setup` publishes global-signal state inside the vdom's
/// runtime before the rebuild runs.
pub fn mount_html(app: fn() -> Element, setup: impl FnOnce()) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.in_runtime(setup);
    vdom.rebuild_in_place();
    dioxus_ssr::Renderer::new().render(&vdom)
}
