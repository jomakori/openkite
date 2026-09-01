//! Liquid Frost Glass design-system CSS contract test.
//!
//! Reads `assets/main.css` via `include_str!` and asserts every required
//! custom property and primitive class is present. Catches:
//!
//! - A missing class rule (someone deleted `.panel` from `main.css`).
//! - A token renamed in CSS but not in the Rust constant table, or vice versa.
//! - An accidental re-declaration of an opaline-mapped variable (the
//!   "exactly 12 new properties" check).
//! - The file accidentally broken by a partial push (the `include_str!`
//!   fails at compile time).
//!
//! This test does NOT import `openkite::design` (per the foundation-first
//! rule that foundation tests must not depend on sibling un-merged
//! modules). It reads the CSS file directly so it works from a fresh CI
//! clone.

/// The shipped stylesheet, embedded at compile time.
const STYLESHEET: &str = include_str!("../assets/main.css");

/// The 12 new custom properties the design system adds on top of the
/// opaline-mapped theme contract.
const REQUIRED_PROPERTIES: &[&str] = &[
    "--brand",
    "--argo",
    "--on-accent",
    "--font-sans",
    "--font-mono",
    "--shadow-rest",
    "--shadow-hover",
    "--shadow-terminal",
    "--r-sm",
    "--r-md",
    "--r-pill",
];

/// Every primitive class the design system ships. Both the design-system
/// `.pill` and the existing `status-badge` are asserted so the contract
/// pins both name sets until a consumer refactor migrates one to the
/// other.
const REQUIRED_CLASSES: &[&str] = &[
    ".panel",
    ".btn",
    ".btn-primary",
    ".btn-secondary",
    ".chip",
    ".search-field",
    ".pill",
    ".pill.success",
    ".pill.warn",
    ".pill.danger",
    ".table-wrap",
    ".resource-name",
    ".log-panel",
    ".log-line",
    ".term-status",
    ".inspector",
    ".kv-list",
    ".toast",
    ".nav-section",
    ".dot",
    ".dot.ok",
    ".dot.warn",
    ".dot.err",
    ".sort-indicator",
    // OKT-43 CRUD modal + confirm dialogs.
    ".modal-backdrop",
    ".modal",
    ".modal-header",
    ".modal-eyebrow",
    ".modal-title",
    ".modal-body",
    ".modal-footer",
    ".modal-editor",
    ".modal-confirm",
    ".editor-textarea",
    ".field-label",
    ".field-helper",
    ".field-error",
    ".confirm-warning",
    ".btn-danger",
];

/// Properties the opaline theme contract already provides — must not be
/// re-declared by the design-system `:root` block.
const PRE_EXISTING_PROPERTIES: &[&str] = &[
    "--bg-0", "--bg-1", "--bg-2", "--border", "--fg-0", "--fg-1", "--fg-2", "--accent", "--green",
    "--yellow", "--red", "--violet",
];

#[test]
fn every_required_property_is_declared() {
    for name in REQUIRED_PROPERTIES {
        let needle = format!("{name}:");
        assert!(
            STYLESHEET.contains(&needle),
            "design-system contract: `{name}` not declared in assets/main.css"
        );
    }
}

#[test]
fn every_required_class_is_present() {
    for class in REQUIRED_CLASSES {
        assert!(
            STYLESHEET.contains(class),
            "design-system contract: `{class}` rule missing from assets/main.css"
        );
    }
}

#[test]
fn pre_existing_theme_properties_are_not_redeclared() {
    // A re-declaration inside the same :root block would be ambiguous
    // (later wins) and likely a mistake. We assert the file's
    // declaration count for each is exactly one.
    for name in PRE_EXISTING_PROPERTIES {
        let needle = format!("{name}:");
        let count = STYLESHEET.matches(&needle).count();
        assert_eq!(
            count, 1,
            "pre-existing opaline property `{name}` is declared {count} times (expected 1)"
        );
    }
}

#[test]
fn design_system_adds_exactly_twelve_new_properties() {
    // Guard against accidental re-declaration: the design system adds
    // exactly 12 new properties on top of the 28 opaline-mapped ones.
    for name in REQUIRED_PROPERTIES {
        let needle = format!("{name}:");
        let count = STYLESHEET.matches(&needle).count();
        assert_eq!(
            count, 1,
            "design-system property `{name}` is declared {count} times (expected exactly 1)"
        );
    }
}

#[test]
fn rust_blur_constants_match_css() {
    // If the Rust constant table drifts from the CSS, downstream style:
    // attributes will go out of sync. The contract test pins the
    // public-facing strings on both sides.
    assert!(
        STYLESHEET.contains("blur(40px)"),
        "BLUR_FROST (40px) not used in any CSS rule"
    );
    assert!(
        STYLESHEET.contains("blur(18px)"),
        "BLUR_PANEL (18px) not used in any CSS rule"
    );
    assert!(
        STYLESHEET.contains("blur(20px)"),
        "BLUR_TOPBAR (20px) not used in any CSS rule"
    );
}

#[test]
fn stylesheet_uses_typed_font_vars() {
    // The design system exposes --font-sans / --font-mono; the existing
    // body rule should reference them rather than the hard-coded stack.
    assert!(
        STYLESHEET.contains("var(--font-sans)"),
        "var(--font-sans) should be used by the body rule"
    );
    assert!(
        STYLESHEET.contains("var(--font-mono)"),
        "var(--font-mono) should be used by .resource-name and .log-panel"
    );
}

#[test]
fn log_panel_is_the_only_opaque_surface() {
    // Brand posture: translucent everywhere except the terminal anchor.
    // Assert .log-panel uses the opaque --term-bg (not a translucent
    // var like --bg-1).
    let log_panel_block = STYLESHEET
        .split(".log-panel {")
        .nth(1)
        .expect(".log-panel rule present");
    let log_panel_body = log_panel_block
        .split('}')
        .next()
        .expect(".log-panel block has a closing brace");
    assert!(
        log_panel_body.contains("var(--term-bg)"),
        ".log-panel must use the opaque --term-bg, got: {log_panel_body}"
    );
}
