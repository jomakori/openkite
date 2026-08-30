//! Typed view of the design-system primitive strings declared in
//! `assets/main.css`. Use these constants in Rust code (Dioxus `style:`
//! attributes, inline `background:` rules) so a CSS-side change is
//! reflected in Rust by updating the constant — and so the contract test
//! can pin both layers to the same value.
//!
//! The values mirror the `:root` and primitive rules in `assets/main.css`
//! exactly. If you change one, change the other and update
//! `tests/design.rs` to match.

/// Backdrop blur for the branded "frost" surfaces (theme cards, status bar).
pub const BLUR_FROST: &str = "40px";

/// Backdrop blur for content panels sitting over the frosted topbar.
pub const BLUR_PANEL: &str = "18px";

/// Backdrop blur for the topbar and sidebar chrome.
pub const BLUR_TOPBAR: &str = "20px";

/// Small radius — buttons, chips, search fields, log-line meta.
pub const R_SM: &str = "6px";

/// Medium radius — panels, table-wrap, cards, inspector, toast.
pub const R_MD: &str = "8px";

/// Pill radius — chips, pills, namespace toggles.
pub const R_PILL: &str = "999px";

/// Resting elevation — single panel shadow.
pub const SHADOW_REST: &str = "0 1px 3px rgba(0,0,0,0.04), 0 4px 12px rgba(0,0,0,0.06)";

/// Hover elevation — buttons, draggable cards.
pub const SHADOW_HOVER: &str = "0 4px 16px rgba(0,0,0,0.08), 0 8px 24px rgba(0,0,0,0.06)";

/// Terminal elevation — the opaque log-panel surface.
pub const SHADOW_TERMINAL: &str = "0 8px 32px rgba(0,0,0,0.12)";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_constants_are_px_values() {
        assert_eq!(BLUR_FROST, "40px");
        assert_eq!(BLUR_PANEL, "18px");
        assert_eq!(BLUR_TOPBAR, "20px");
    }

    #[test]
    fn radii_match_brand_spec() {
        assert_eq!(R_SM, "6px");
        assert_eq!(R_MD, "8px");
        assert_eq!(R_PILL, "999px");
    }

    #[test]
    fn shadow_strings_are_non_empty() {
        assert!(SHADOW_REST.starts_with("0 "));
        assert!(SHADOW_HOVER.starts_with("0 "));
        assert!(SHADOW_TERMINAL.starts_with("0 "));
    }
}
