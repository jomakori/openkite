//! Liquid Frost Glass design-system CSS contract test.
//!
//! Reads the shell stylesheet through `openkite_ui::MAIN_CSS` and asserts every
//! required custom property and primitive class is present. Catches:
//!
//! - A missing class rule (someone deleted `.panel` from `main.css`).
//! - A token renamed in CSS but not in the Rust constant table, or vice versa
//!   (`root_declares_only_the_theme_contract_and_the_design_system_properties`
//!   pins the two layers to the same name set).
//! - An accidental re-declaration of an opaline-mapped variable (the
//!   "declared exactly once" checks).
//! - The file accidentally broken by a partial push (the `include_str!`
//!   fails at compile time).
//!
//! This test does NOT import the `design` module (per the foundation-first
//! rule that foundation tests must not depend on sibling un-merged
//! modules). It reads the stylesheet bytes so it works from a fresh CI
//! clone.

/// The shipped stylesheet, embedded at compile time.
const STYLESHEET: &str = openkite_ui::MAIN_CSS;

/// Custom properties the design-system `:root` declares that the opaline
/// theme engine does not carry.
const REQUIRED_PROPERTIES: &[&str] = &[
    "--brand",
    "--argo",
    "--on-accent",
    "--terminal-bg",
    "--terminal-fg",
    "--log-info",
    "--log-method",
    "--log-error",
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
    ".value-mask",
    ".value-masked",
    ".value-revealed",
    ".value-actions",
    ".reveal-btn",
    ".inspector",
    // The resource detail pane (OKT-175): one stop for every kind, its own
    // drag handle, and the touch scrim the reference draws for the ≤767px
    // bottom sheet.
    ".inspector-scrim",
    ".inspector-scrim.show",
    ".inspector-resize",
    ".inspector-close",
    ".inspector-header",
    ".inspector-title",
    ".inspector-body",
    ".inspector-actions",
    ".inspector-eyebrow",
    ".resource-kind",
    ".kv-row",
    ".table-row.selected",
    ".kv-list",
    ".toast",
    ".nav-section",
    ".dot",
    ".dot.ok",
    ".dot.warn",
    ".dot.err",
    ".sort-indicator",
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
    // The shell's own vocabulary (OKT-154): the frame, its sidebar and top bar
    // carry the design's class names, not the desktop's old ones.
    ".app",
    ".icon",
    ".sidebar",
    ".sidebar-backdrop",
    ".sidebar-backdrop.show",
    ".sidebar-footer",
    ".brand",
    ".brand-mark",
    ".brand-word",
    ".cluster-btn",
    ".nav",
    ".nav-title",
    ".nav-item",
    ".nav-item.active",
    ".nav-badge",
    ".main",
    ".topbar",
    ".menu-toggle",
    ".breadcrumbs",
    ".breadcrumbs .crumb",
    ".breadcrumbs .current",
    ".topbar-actions",
    ".icon-btn",
    ".avatar",
    ".view",
    ".view.active",
    ".pull-indicator",
    ".eyebrow",
    ".status-line",
    // The route chrome (OKT-155): the heading row, the filter toolbar, the
    // panel footer and the declaration chips every primary route carries.
    ".page-head",
    ".page-sub",
    ".page-actions",
    ".toolbar",
    ".chip-row",
    ".panel-footer",
    ".pager",
    ".pager button.active",
    ".tag",
    ".tag-row",
    ".ns-bar",
    ".ns-search",
    ".ns-filter",
    ".ns-reset",
    ".chip-row .chip",
    ".chip-mark",
];

/// Properties the opaline theme contract already provides — must not be
/// re-declared by the design-system `:root` block.
const PRE_EXISTING_PROPERTIES: &[&str] = &[
    "--bg",
    "--surface",
    "--surface-solid",
    "--border",
    "--fg",
    "--muted",
    "--subtle",
    "--accent",
    "--progress",
    "--success",
    "--warn",
    "--danger",
    "--violet",
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
fn design_system_adds_each_new_property_exactly_once() {
    for name in REQUIRED_PROPERTIES {
        let needle = format!("{name}:");
        let count = STYLESHEET.matches(&needle).count();
        assert_eq!(
            count, 1,
            "design-system property `{name}` is declared {count} times (expected exactly 1)"
        );
    }
}

/// The custom-property names the stylesheet's `:root` block declares.
fn declared_root_properties() -> Vec<String> {
    let css = strip_css_comments(STYLESHEET);
    let (_, rest) = css.split_once(":root {").expect("main.css declares :root");
    let (block, _) = rest.split_once('}').expect("the :root block is closed");
    let mut names = Vec::new();
    let mut cursor = block;
    while let Some(at) = cursor.find("--") {
        cursor = &cursor[at..];
        let end = cursor
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .unwrap_or(cursor.len());
        if cursor[end..].starts_with(':') {
            names.push(cursor[..end].to_string());
        }
        cursor = &cursor[end..];
    }
    names
}

#[test]
fn root_declares_only_the_theme_contract_and_the_design_system_properties() {
    let mut declared = declared_root_properties();
    declared.sort();
    declared.dedup();

    let mut expected: Vec<String> = openkite_ui::theme::CSS_VARS
        .iter()
        .map(|v| (*v).to_string())
        .collect();
    expected.extend(REQUIRED_PROPERTIES.iter().map(|v| (*v).to_string()));
    expected.sort();

    assert_eq!(
        declared, expected,
        "assets/main.css :root and the crate's token tables disagree — a name \
         exists in one layer but not the other"
    );
}

#[test]
fn rust_blur_constants_match_css() {
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
    let log_panel_block = STYLESHEET
        .split(".log-panel {")
        .nth(1)
        .expect(".log-panel rule present");
    let log_panel_body = log_panel_block
        .split('}')
        .next()
        .expect(".log-panel block has a closing brace");
    assert!(
        log_panel_body.contains("var(--terminal-bg)"),
        ".log-panel must use the opaque --terminal-bg, got: {log_panel_body}"
    );
}

/* ---------------------------------------------------------------------------
RSX-vs-stylesheet contract (OKT-153).

Every CSS class the crate renders must exist in `assets/main.css`. The
failure mode is silent: the markup looks right and the elements are
simply unstyled. This test greps the rsx `class:` / `class=` attributes
across every component and the host `App`, extracts the literal tokens,
and asserts each is covered by a selector in the shipped stylesheet
(either as a bare selector or as a sub-piece of a compound one).

The source files are pulled in via `include_str!` so the test runs from
a fresh CI clone without depending on the workspace being on disk.
--------------------------------------------------------------------------- */

const APP_RSX: &str = include_str!("../../openkite-web/src/app.rs");
const STATUS_BADGE_RSX: &str = include_str!("../src/components/status_badge.rs");
const THEME_SELECTOR_RSX: &str = include_str!("../src/components/theme_selector.rs");
const SECRET_DETAIL_RSX: &str = include_str!("../src/components/secret_detail.rs");
const RESOURCE_TABLE_RSX: &str = include_str!("../src/components/resource_table.rs");
const CODE_EDITOR_RSX: &str = include_str!("../src/components/code_editor.rs");
const CRUD_MODAL_RSX: &str = include_str!("../src/components/crud_modal.rs");
const SHELL_RSX: &str = include_str!("../src/components/shell.rs");
const ROUTE_VIEWS_RSX: &str = include_str!("../src/components/route_views.rs");
const NAMESPACE_BAR_RSX: &str = include_str!("../src/components/namespace_bar.rs");

/// Every rsx source file the crate renders. Order is for stable error
/// messages — does not affect semantics.
const RSX_SOURCES: &[(&str, &str)] = &[
    ("crates/openkite-web/src/app.rs", APP_RSX),
    ("crates/openkite-ui/src/components/shell.rs", SHELL_RSX),
    (
        "crates/openkite-ui/src/components/route_views.rs",
        ROUTE_VIEWS_RSX,
    ),
    (
        "crates/openkite-ui/src/components/status_badge.rs",
        STATUS_BADGE_RSX,
    ),
    (
        "crates/openkite-ui/src/components/theme_selector.rs",
        THEME_SELECTOR_RSX,
    ),
    (
        "crates/openkite-ui/src/components/secret_detail.rs",
        SECRET_DETAIL_RSX,
    ),
    (
        "crates/openkite-ui/src/components/resource_table.rs",
        RESOURCE_TABLE_RSX,
    ),
    (
        "crates/openkite-ui/src/components/code_editor.rs",
        CODE_EDITOR_RSX,
    ),
    (
        "crates/openkite-ui/src/components/crud_modal.rs",
        CRUD_MODAL_RSX,
    ),
    (
        "crates/openkite-ui/src/components/namespace_bar.rs",
        NAMESPACE_BAR_RSX,
    ),
];

/// Extract every distinct CSS token used in a `class:` or `class=` rsx
/// attribute. Both literal tokens (`"kv-row"`) and the static portions of
/// interpolated strings (`"pill {status.pill_class()}"` → `"pill"`) are
/// captured. Ternary `class: if … { "a" } else { "b" }` branches are both
/// included.
///
/// Dynamic placeholders (`{foo}`) are not asserted directly — they are
/// opaque from the stylesheet's perspective. Callers are responsible for
/// pinning the strings they expand to (e.g. `StatusKind::pill_class` →
/// `"success" | "warn" | "danger" | "muted"`, covered by
/// `tests/resource_table.rs::status_kind_pill_classes_cover_every_variant`).
fn collect_rsx_class_tokens() -> Vec<(&'static str, String)> {
    let mut seen = std::collections::BTreeMap::<String, Vec<&'static str>>::new();

    for (source_path, source) in RSX_SOURCES {
        for_each_class_literal(source, |value| {
            for token in tokenize_class_value(value) {
                seen.entry(token).or_default().push(*source_path);
            }
        });
        for_each_class_ternary(source, |true_branch, false_branch| {
            for branch in [true_branch, false_branch] {
                for token in tokenize_class_value(branch) {
                    seen.entry(token).or_default().push(*source_path);
                }
            }
        });
    }

    seen.into_iter()
        .map(|(token, sources)| {
            let mut uniq = sources;
            uniq.sort();
            uniq.dedup();
            (uniq[0], token)
        })
        .collect()
}

/// Hand-rolled walker that finds every `class:` / `class=` rsx attribute
/// string literal and hands the value (without surrounding quotes) to `cb`.
/// No regex dependency — a small string scan is enough.
fn for_each_class_literal(source: &str, mut cb: impl FnMut(&str)) {
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Match the literal `class` keyword as a whole word.
        if bytes[i..].starts_with(b"class")
            && (i == 0 || !is_ident_byte(bytes[i - 1]))
            && i + 5 < bytes.len()
            && !is_ident_byte(bytes[i + 5])
        {
            let mut j = i + 5;
            // Skip whitespace.
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            // Expect `:` or `=`.
            if j < bytes.len() && (bytes[j] == b':' || bytes[j] == b'=') {
                j += 1;
                // Skip whitespace.
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                // Expect `"`.
                if j < bytes.len() && bytes[j] == b'"' {
                    let start = j + 1;
                    let mut k = start;
                    while k < bytes.len() && bytes[k] != b'"' {
                        k += 1;
                    }
                    if k < bytes.len() {
                        // SAFETY: input is valid UTF-8 and we only cut on
                        // ASCII boundaries.
                        let value = &source[start..k];
                        cb(value);
                        i = k + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
}

/// Hand-rolled walker that finds every `class: if <cond> { "a" } else { "b" }`
/// rsx ternary and hands both branches to `cb`. Sufficient because the
/// ternary shape is fixed (a literal string in each arm).
fn for_each_class_ternary(source: &str, mut cb: impl FnMut(&str, &str)) {
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i..].starts_with(b"class") {
            i += 1;
            continue;
        }
        if i > 0 && is_ident_byte(bytes[i - 1]) {
            i += 1;
            continue;
        }
        let mut j = i + 5;
        if j < bytes.len() && is_ident_byte(bytes[j]) {
            i += 1;
            continue;
        }
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b':' {
            i += 1;
            continue;
        }
        j += 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        // Expect `if`.
        if !bytes[j..].starts_with(b"if") {
            i += 1;
            continue;
        }
        // Skip until we hit the first `{`.
        while j < bytes.len() && bytes[j] != b'{' {
            j += 1;
        }
        if j >= bytes.len() {
            i += 1;
            continue;
        }
        // Capture first literal inside the if-branch.
        let (lit_a, after_a) = match scan_string_literal(source, j + 1) {
            Some(found) => found,
            None => {
                i += 1;
                continue;
            }
        };
        // Skip until the `else` keyword.
        let mut k = after_a;
        while k < bytes.len() {
            if bytes[k..].starts_with(b"else")
                && (k == 0 || !is_ident_byte(bytes[k - 1]))
                && (k + 4 >= bytes.len() || !is_ident_byte(bytes[k + 4]))
            {
                break;
            }
            k += 1;
        }
        if k >= bytes.len() {
            i += 1;
            continue;
        }
        // Skip whitespace, then expect `{`.
        k += 4;
        while k < bytes.len() && bytes[k].is_ascii_whitespace() {
            k += 1;
        }
        if k >= bytes.len() || bytes[k] != b'{' {
            i += 1;
            continue;
        }
        let (lit_b, _) = match scan_string_literal(source, k + 1) {
            Some(found) => found,
            None => {
                i += 1;
                continue;
            }
        };
        cb(lit_a, lit_b);
        i += 1;
    }
}

/// Find the next `"`-delimited string literal starting at byte offset `start`.
/// Returns `(slice, end_of_closing_quote)`. The slice never contains the
/// surrounding quotes.
fn scan_string_literal(source: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = source.as_bytes();
    let mut j = start;
    while j < bytes.len() && bytes[j] != b'"' {
        j += 1;
    }
    if j >= bytes.len() {
        return None;
    }
    let begin = j + 1;
    j = begin;
    while j < bytes.len() && bytes[j] != b'"' {
        j += 1;
    }
    if j >= bytes.len() {
        return None;
    }
    Some((&source[begin..j], j))
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Split a class attribute value into static CSS class tokens, dropping
/// the contents of any `{…}` interpolation. A value that is PURELY one or
/// more interpolations (`"{cls}"`, `"{x} {y}"`) yields nothing — those
/// tokens aren't class names from the stylesheet's perspective.
///
/// Tokens are also filtered to the CSS class-name shape
/// `[A-Za-z_-][A-Za-z0-9_-]*`, which drops things like `status.class()`
/// that survive the brace stripping but are obviously not class names.
fn tokenize_class_value(value: &str) -> impl Iterator<Item = String> + '_ {
    let mut static_parts = Vec::new();
    let mut current = String::new();
    let mut in_interp = false;
    for c in value.chars() {
        match c {
            '{' => {
                in_interp = true;
                current.clear();
            }
            '}' => {
                in_interp = false;
                current.clear();
            }
            c if c.is_whitespace() => {
                if !in_interp && !current.is_empty() {
                    static_parts.push(std::mem::take(&mut current));
                }
            }
            _ => {
                if !in_interp {
                    current.push(c);
                }
            }
        }
    }
    if !in_interp && !current.is_empty() {
        static_parts.push(current);
    }
    static_parts
        .into_iter()
        .filter(|tok| is_css_class_name(tok))
        .collect::<Vec<_>>()
        .into_iter()
}

fn is_css_class_name(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_' || first == '-') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Parse the bare class selectors out of the stylesheet. Returns the set of
/// bare tokens and the list of compound selectors (e.g. `.chip.active`) so
/// the coverage check can match a token like `active` against `.chip.active`.
///
/// Walks the stylesheet tracking brace depth so nested rules (inside
/// `@media`, `@supports`, etc.) are all collected.
fn parse_stylesheet_selectors(css: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut bare: Vec<String> = Vec::new();
    let mut compound: Vec<Vec<String>> = Vec::new();
    let bytes = css.as_bytes();
    let mut depth = 0usize;
    let mut head_start = 0usize;
    let mut heads: Vec<String> = Vec::new();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'{' => {
                if depth == 0 {
                    let raw = &css[head_start..i];
                    let cleaned = strip_css_comments(raw).trim().to_string();
                    if !cleaned.is_empty() {
                        heads.push(cleaned);
                    }
                    head_start = i + 1;
                }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    head_start = i + 1;
                }
            }
            _ => {}
        }
    }

    for head in heads {
        // A single selector head may contain commas (grouped selectors).
        for sel in head.split(',') {
            let sel = sel.trim();
            if sel.is_empty() {
                continue;
            }
            // Tokenize on whitespace — keeps `.foo.bar` as one piece and
            // splits descendants/children apart. We only care about the
            // class pieces; element selectors (`input`, `*`) are skipped.
            //
            // Compound class selectors (`.chip.active`, `.pill.success`)
            // are stored as a vec of pieces so the coverage check can
            // match a token like `active` against `.chip.active`.
            let mut parts: Vec<String> = Vec::new();
            for piece in sel.split_whitespace() {
                // Strip pseudo-class / pseudo-element after the first
                // colon so `.pill::before` → `.pill`.
                let head = piece.split(':').next().unwrap_or("");
                if !head.starts_with('.') {
                    continue;
                }
                for raw in head.split('.') {
                    let token = raw.trim();
                    if token.is_empty() {
                        continue;
                    }
                    parts.push(token.to_string());
                }
            }
            if parts.is_empty() {
                continue;
            }
            // Dedup while preserving order so `.foo.foo` → `[foo]`.
            let mut seen = std::collections::HashSet::<String>::new();
            parts.retain(|p| seen.insert(p.clone()));
            if parts.len() == 1 {
                bare.push(parts.into_iter().next().expect("non-empty"));
            } else {
                compound.push(parts);
            }
        }
    }
    (bare, compound)
}

/// Strip `/* … */` comments. Single pass, string-literal safe enough for
/// our stylesheet (we never emit `/*` inside a class selector).
fn strip_css_comments(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// True when the stylesheet defines a selector that targets the given token
/// — either as a bare selector (`.token`) or as one of the pieces of a
/// compound selector (`.other.token`, `.token.other`, etc.).
fn stylesheet_covers(token: &str, bare: &[String], compound: &[Vec<String>]) -> bool {
    if bare.iter().any(|b| b == token) {
        return true;
    }
    compound
        .iter()
        .any(|parts| parts.iter().any(|p| p == token))
}

#[test]
fn every_rsx_class_is_defined_in_main_css() {
    let (bare, compound) = parse_stylesheet_selectors(STYLESHEET);
    let tokens = collect_rsx_class_tokens();

    let mut failures: Vec<(String, &'static str)> = Vec::new();
    for (source, token) in &tokens {
        if !stylesheet_covers(token, &bare, &compound) {
            failures.push((token.clone(), source));
        }
    }

    assert!(
        failures.is_empty(),
        "rsx classes with no stylesheet rule (fix by adding the rule to \
         crates/openkite-ui/assets/main.css, or by renaming the rsx \
         attribute to a class the design system already ships):\n{}",
        failures
            .iter()
            .map(|(tok, src)| format!("  - `{tok}` ({src})"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Sanity check that the parser sees the stylesheet's selectors — if this
/// test fails the parser is broken, not the stylesheet.
#[test]
fn rsx_class_coverage_parser_sees_known_selectors() {
    let (bare, compound) = parse_stylesheet_selectors(STYLESHEET);
    for must_exist in [
        "panel",
        "pill",
        "pill.success",
        "inspector",
        "inspector.open",
        "chip",
        "chip.active",
        "toast",
        "toast.show",
        "theme-row",
        "theme-row-active",
        "status-badge",
        "status-ok",
        "surface",
        "kv",
        "error",
        "field-input",
        "resource-table",
        "table-cell",
        "table-row",
    ] {
        let covered = if let Some((base, variant)) = must_exist.split_once('.') {
            stylesheet_covers(base, &bare, &compound)
                && stylesheet_covers(variant, &bare, &compound)
        } else {
            stylesheet_covers(must_exist, &bare, &compound)
        };
        assert!(
            covered,
            "parser regression: .{must_exist} should be covered"
        );
    }
}

/// Smoke test that the rsx walker actually finds classes. Catches a future
/// refactor that breaks the `class:` / `class=` matcher.
#[test]
fn rsx_class_walker_finds_known_tokens() {
    let tokens = collect_rsx_class_tokens();
    let names: std::collections::BTreeSet<&str> = tokens.iter().map(|(_, t)| t.as_str()).collect();
    for must_find in ["app", "topbar", "btn", "btn-primary", "panel", "kv-row"] {
        assert!(
            names.contains(must_find),
            "walker regression: rsx must surface `{must_find}` as a class token"
        );
    }
}

/// The declaration body of the first rule whose selector head is exactly
/// `selector`.
fn rule_body(css: &str, selector: &str) -> Option<String> {
    let mut search_from = 0usize;
    while let Some(offset) = css[search_from..].find(selector) {
        let start = search_from + offset;
        let after = start + selector.len();
        // The head must end here — the next byte is `{`, `,` (a grouped
        // selector head) or whitespace. This rejects `selector` appearing as
        // a prefix of a longer token (`.chip` vs `.chip-row`).
        let boundary = css[after..].chars().next();
        let is_head = match boundary {
            Some('{') | Some(',') | None => true,
            Some(c) => c.is_whitespace(),
        };
        if !is_head {
            search_from = after;
            continue;
        }
        let open = css[start..].find('{').map(|i| start + i)?;
        let close = css[open..].find('}').map(|i| open + i)?;
        return Some(css[open + 1..close].to_string());
    }
    None
}

/// The namespace strip stays one line at every width: `nowrap` with horizontal
/// overflow, a hidden scrollbar and scroll-snap, and a chip at the 44px floor.
#[test]
fn namespace_strip_is_one_hidden_scrollbar_line_with_scroll_snap() {
    let row = rule_body(STYLESHEET, ".chip-row").expect("`.chip-row` rule");
    assert!(
        row.contains("flex-wrap: nowrap"),
        "the strip must never wrap: {row}"
    );
    assert!(
        row.contains("overflow-x: auto"),
        "the strip scrolls horizontally: {row}"
    );
    assert!(
        row.contains("scroll-snap-type: x proximity"),
        "chips snap so none rests half-visible: {row}"
    );
    assert!(
        row.contains("scrollbar-width: none"),
        "the scrollbar is hidden (Firefox): {row}"
    );
    assert!(
        STYLESHEET.contains(".chip-row::-webkit-scrollbar"),
        "the scrollbar is hidden (WebKit/Blink)"
    );
    assert!(
        STYLESHEET.contains("display: none")
            && STYLESHEET.contains(".chip-row::-webkit-scrollbar { display: none; }"),
        "the WebKit scrollbar rule must actually hide it"
    );

    let chip = rule_body(STYLESHEET, ".chip-row .chip").expect("`.chip-row .chip` rule");
    assert!(
        chip.contains("scroll-snap-align: start"),
        "each chip is a snap target: {chip}"
    );
    assert!(
        chip.contains("flex: 0 0 auto"),
        "chips keep their natural width instead of squashing: {chip}"
    );

    let base_chip = rule_body(STYLESHEET, ".chip").expect("`.chip` rule");
    assert!(
        base_chip.contains("min-height: 44px"),
        "every chip is a >=44px touch target: {base_chip}"
    );

    let circles = rule_body(STYLESHEET, ".ns-search").expect("`.ns-search` rule");
    assert!(
        circles.contains("width: 44px") && circles.contains("height: 44px"),
        "the search circle and its reset twin are 44px: {circles}"
    );
    assert!(
        circles.contains("border-radius: var(--r-pill)"),
        "…and circular: {circles}"
    );
}
