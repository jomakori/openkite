//! OS window menu bar visibility (OKT-99).
//!
//! dioxus-desktop installs a muda-built platform default menu bar when the
//! menu state is left unset (`MenuBuilderState::Unset`). To make that bar
//! toggleable at runtime at all, the host owns it: [`build_menu`] mirrors the
//! default (Window + Edit, plus the debug-only Help), [`install`] keeps a
//! handle, and `Config::with_menu` hands the same menu to dioxus-desktop.
//! When the persisted setting is `hide`, the host passes `with_menu(None)` so
//! no bar is installed at startup; a later palette toggle re-initialises it.
//!
//! macOS is deliberately excluded from the runtime toggle: its menu bar is a
//! single global system bar that cannot be hidden, and the default menu
//! carries the cut/copy/paste accelerators. [`hideable`] is false there, so
//! the palette never offers a switch that would silently no-op.

use crate::config::OpenKiteConfig;

#[cfg(all(
    feature = "desktop",
    not(any(target_os = "ios", target_os = "android"))
))]
use dioxus::desktop::muda::{Menu, MenuItem, PredefinedMenuItem, Submenu};
#[cfg(all(
    feature = "desktop",
    not(any(target_os = "ios", target_os = "android"))
))]
use std::cell::RefCell;

/// Whether this build can hide/show the OS menu bar at runtime.
///
/// False without the `desktop` renderer, on macOS (a single global system
/// menu bar), and on iOS/Android (no menu bar concept).
pub const fn hideable() -> bool {
    cfg!(all(
        feature = "desktop",
        any(target_os = "linux", target_os = "windows")
    ))
}

#[cfg(all(
    feature = "desktop",
    not(any(target_os = "ios", target_os = "android"))
))]
thread_local! {
    /// The live menu handle. `muda::Menu` is `Rc`-backed (`!Send`), so it is
    /// kept on the desktop event-loop thread that created and drives it.
    static LIVE_MENU: RefCell<Option<Menu>> = const { RefCell::new(None) };
}

/// Build the app menu bar: the same Window/Edit structure (and debug Help)
/// dioxus-desktop installs by default, so replacing it with our own handle is
/// behaviour-preserving.
#[cfg(all(
    feature = "desktop",
    not(any(target_os = "ios", target_os = "android"))
))]
pub fn build_menu() -> Menu {
    let menu = Menu::new();

    let window_menu = Submenu::new("Window", true);
    window_menu
        .append_items(&[
            &PredefinedMenuItem::fullscreen(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::maximize(None),
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::close_window(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ])
        .expect("building the Window submenu");

    let edit_menu = Submenu::new("Edit", true);
    edit_menu
        .append_items(&[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::select_all(None),
        ])
        .expect("building the Edit submenu");

    menu.append_items(&[&window_menu, &edit_menu])
        .expect("appending top-level menus");

    #[cfg(debug_assertions)]
    {
        let help_menu = Submenu::new("Help", true);
        help_menu
            .append_items(&[
                &MenuItem::with_id(
                    "dioxus-toggle-dev-tools",
                    "Toggle Developer Tools",
                    true,
                    None,
                ),
                &MenuItem::with_id(
                    "dioxus-float-top",
                    "Float on Top (dev mode only)",
                    true,
                    None,
                ),
            ])
            .expect("building the Help submenu");
        menu.append_items(&[&help_menu])
            .expect("appending the Help menu");
    }

    menu
}

/// Retain a clone of the live menu so a palette toggle can reach it later.
#[cfg(all(
    feature = "desktop",
    not(any(target_os = "ios", target_os = "android"))
))]
pub fn install(menu: Menu) {
    LIVE_MENU.with(|slot| *slot.borrow_mut() = Some(menu));
}

#[cfg(all(feature = "desktop", any(target_os = "linux", target_os = "windows")))]
fn live() -> Option<Menu> {
    LIVE_MENU.with(|slot| slot.borrow().clone())
}

/// Flip the persisted menu-bar setting and apply it to the live window.
///
/// The preference is saved even when the platform call fails, so the next
/// launch starts from the state the user chose.
pub fn toggle() {
    if !hideable() {
        tracing::warn!("menu bar cannot be toggled on this platform");
        return;
    }
    let mut config = OpenKiteConfig::load();
    let next = config.menu_bar.toggled();
    config.menu_bar = next;
    if let Err(error) = config.save() {
        tracing::warn!(%error, "failed to persist the menu bar setting");
    }
    apply(next.is_visible());
}

/// Apply a visibility change to the live window (no-op off Linux/Windows).
fn apply(visible: bool) {
    #[cfg(all(feature = "desktop", any(target_os = "linux", target_os = "windows")))]
    {
        let Some(menu) = live() else {
            tracing::warn!("menu bar handle missing; nothing to toggle");
            return;
        };
        let window = dioxus::desktop::window().window.clone();
        set_visible(&menu, &window, visible);
    }
    #[cfg(not(all(feature = "desktop", any(target_os = "linux", target_os = "windows"))))]
    {
        let _ = visible;
    }
}

#[cfg(all(feature = "desktop", target_os = "linux"))]
fn set_visible(menu: &Menu, window: &dioxus::desktop::tao::window::Window, visible: bool) {
    use dioxus::desktop::tao::platform::unix::WindowExtUnix;
    let gtk_window = window.gtk_window();
    let container = window.default_vbox();
    if visible {
        match menu.init_for_gtk_window(gtk_window, container) {
            Ok(()) | Err(dioxus::desktop::muda::Error::AlreadyInitialized) => {}
            Err(error) => tracing::warn!(%error, "failed to install the menu bar"),
        }
        if let Err(error) = menu.show_for_gtk_window(gtk_window) {
            tracing::warn!(%error, "failed to show the menu bar");
        }
    } else if let Err(error) = menu.hide_for_gtk_window(gtk_window) {
        tracing::warn!(%error, "failed to hide the menu bar");
    }
}

#[cfg(all(feature = "desktop", target_os = "windows"))]
fn set_visible(menu: &Menu, window: &dioxus::desktop::tao::window::Window, visible: bool) {
    use dioxus::desktop::tao::platform::windows::WindowExtWindows;
    let hwnd = window.hwnd();
    // SAFETY: `hwnd` is the live tao window owned by the desktop context, so
    // it is valid for the duration of this call.
    unsafe {
        if visible {
            match menu.init_for_hwnd(hwnd) {
                Ok(()) | Err(dioxus::desktop::muda::Error::AlreadyInitialized) => {}
                Err(error) => tracing::warn!(%error, "failed to install the menu bar"),
            }
            if let Err(error) = menu.show_for_hwnd(hwnd) {
                tracing::warn!(%error, "failed to show the menu bar");
            }
        } else if let Err(error) = menu.hide_for_hwnd(hwnd) {
            tracing::warn!(%error, "failed to hide the menu bar");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hideable_matches_the_configured_platform() {
        let expected = cfg!(all(
            feature = "desktop",
            any(target_os = "linux", target_os = "windows")
        ));
        assert_eq!(hideable(), expected);
    }

    #[test]
    fn toggle_is_a_noop_when_not_hideable() {
        if !hideable() {
            // Must not panic (no window context): returns before touching one.
            toggle();
        }
    }
}
