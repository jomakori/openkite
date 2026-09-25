//! OS window decoration theme override (OKT-100).
//!
//! tao models the native window decoration theme (title bar and OS chrome) as
//! `Option<Theme>`: `None` follows the system, `Some(Theme::Light | Dark)`
//! forces it. `WindowBuilder::with_theme` applies the choice at window
//! creation; `Window::set_theme` changes it live with no restart. tao's
//! [`Theme`] has no "system" variant, so the persisted setting is the
//! tri-state [`TitleBarTheme`] and [`TitleBarTheme::System`] maps to `None`.
//!
//! Platform matrix (tao's `Window::set_theme` docs): per-window on Windows,
//! app-wide on Linux and macOS.
//!
//! This is strictly the OS decorations. The in-page app theme
//! ([`crate::theme`], opaline tokens + the CSS variable contract) is
//! independent and is never touched from here.

use crate::config::{OpenKiteConfig, TitleBarTheme};

/// Whether this build can override the OS decoration theme.
///
/// True for desktop builds on Linux, Windows, and macOS; false without the
/// `desktop` renderer and on iOS/Android (tao's `set_theme` is a no-op there).
pub const fn overridable() -> bool {
    cfg!(all(
        feature = "desktop",
        not(any(target_os = "ios", target_os = "android"))
    ))
}

/// Persist an explicit `titleBarTheme` choice and apply it to the live window.
///
/// Saved even when the platform call fails, so the next launch starts from the
/// state the user chose.
pub fn set(theme: TitleBarTheme) {
    let mut config = OpenKiteConfig::load();
    config.title_bar_theme = theme;
    if let Err(error) = config.save() {
        tracing::warn!(%error, "failed to persist the title bar theme setting");
    }
    apply(theme);
}

/// Build dioxus-desktop's window with the same defaults it installs itself
/// (title from the Dioxus CLI config, debug-mode always-on-top) plus the
/// requested decoration theme. Startup only.
#[cfg(all(
    feature = "desktop",
    not(any(target_os = "ios", target_os = "android"))
))]
pub fn window_builder(theme: TitleBarTheme) -> dioxus::desktop::WindowBuilder {
    let window = dioxus::desktop::WindowBuilder::new()
        .with_title(dioxus::cli_config::app_title().unwrap_or_else(|| "Dioxus App".to_string()));
    #[cfg(debug_assertions)]
    let window = window.with_always_on_top(dioxus::cli_config::always_on_top().unwrap_or(true));
    window.with_theme(to_tao(theme))
}

/// Apply a decoration theme to the live window and log the effective theme the
/// OS reports back.
#[cfg(all(
    feature = "desktop",
    not(any(target_os = "ios", target_os = "android"))
))]
pub fn apply(theme: TitleBarTheme) {
    let window = dioxus::desktop::window().window.clone();
    window.set_theme(to_tao(theme));
    tracing::info!(
        requested = theme.as_str(),
        effective = effective_for(&window),
        "title bar theme applied"
    );
}

#[cfg(not(all(
    feature = "desktop",
    not(any(target_os = "ios", target_os = "android"))
)))]
pub fn apply(_theme: TitleBarTheme) {
    tracing::warn!("title bar theming needs the desktop renderer; ignoring");
}

/// The concrete decoration theme the OS currently reports, as a lowercase
/// label (`"light"` / `"dark"`). `None` when no window exists or off desktop.
#[cfg(all(
    feature = "desktop",
    not(any(target_os = "ios", target_os = "android"))
))]
pub fn effective_label() -> Option<&'static str> {
    // `try_consume_context` rather than `desktop::window()` so headless
    // renders (tests) read `None` instead of panicking.
    let context = dioxus::prelude::try_consume_context::<dioxus::desktop::DesktopContext>()?;
    Some(effective_for(&context.window))
}

#[cfg(not(all(
    feature = "desktop",
    not(any(target_os = "ios", target_os = "android"))
)))]
pub fn effective_label() -> Option<&'static str> {
    None
}

/// Map the tri-state setting onto tao's `Option<Theme>`.
#[cfg(all(
    feature = "desktop",
    not(any(target_os = "ios", target_os = "android"))
))]
fn to_tao(theme: TitleBarTheme) -> Option<dioxus::desktop::tao::window::Theme> {
    use dioxus::desktop::tao::window::Theme;
    match theme {
        TitleBarTheme::System => None,
        TitleBarTheme::Light => Some(Theme::Light),
        TitleBarTheme::Dark => Some(Theme::Dark),
    }
}

/// Read the effective theme off a live tao window. `Theme` is
/// `#[non_exhaustive]`, so unknown variants map to a generic label.
#[cfg(all(
    feature = "desktop",
    not(any(target_os = "ios", target_os = "android"))
))]
fn effective_for(window: &dioxus::desktop::tao::window::Window) -> &'static str {
    use dioxus::desktop::tao::window::Theme;
    match window.theme() {
        Theme::Light => "light",
        Theme::Dark => "dark",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overridable_matches_the_configured_platform() {
        let expected = cfg!(all(
            feature = "desktop",
            not(any(target_os = "ios", target_os = "android"))
        ));
        assert_eq!(overridable(), expected);
    }

    #[cfg(all(
        feature = "desktop",
        not(any(target_os = "ios", target_os = "android"))
    ))]
    #[test]
    fn to_tao_maps_system_to_none_and_forces_light_dark() {
        use dioxus::desktop::tao::window::Theme;
        assert_eq!(to_tao(TitleBarTheme::System), None);
        assert_eq!(to_tao(TitleBarTheme::Light), Some(Theme::Light));
        assert_eq!(to_tao(TitleBarTheme::Dark), Some(Theme::Dark));
    }
}
