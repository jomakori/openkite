//! Liquid Frost Glass design-system CSS contract test.
//!
//! Two layers, both reading shipped bytes through `include_str!` so they run
//! from a fresh CI clone:
//!
//! 1. **Reference-derived contract (G1, OKT-193).** The expected class, token
//!    and state sets are derived from the vendored mockup under
//!    `docs/reference/opendesign-console/` — none of them is hand-listed here.
//!    The mockup bytes are pinned by SHA-256 in a machine-readable block in
//!    `docs/design-surfaces.md`, so re-grabbing the mockup fails the contract
//!    until that block is re-derived and reviewed; the reference selectors the
//!    app deliberately does not ship are listed in the same block with a
//!    reason, so an omission is a documented decision and never silent drift.
//! 2. **RSX-vs-stylesheet contract (OKT-153).** Every class the crate renders
//!    exists in `assets/main.css`, and every base+state pair it renders has a
//!    compound rule — a surface that renders `.nav-item` but never
//!    `.nav-item.active` is a failed contract.
//!
//! Catches a deleted class rule (`.panel`), a token renamed on one side only,
//! an accidental re-declaration of an opaline-mapped variable, and a reference
//! that changed without anyone re-deriving it.
//!
//! This test does NOT import the `design` module (per the foundation-first
//! rule that foundation tests must not depend on sibling un-merged modules).

/// The shipped stylesheet, embedded at compile time.
const STYLESHEET: &str = openkite_ui::MAIN_CSS;

/* ---------------------------------------------------------------------------
Reference-derived contract (G1, OKT-193).

Everything this contract asserts about the design comes from the vendored
mockup bytes. The only hand-written part is the *scope*: the pinned hashes and
the handful of reference selectors the app deliberately does not ship live in a
`design-contract` block in `docs/design-surfaces.md`, which is also where a
reviewer looks. Changing the mockup therefore fails twice — once on the hash,
once on whatever class/token/state set moved — and both failures name the fix.
--------------------------------------------------------------------------- */

/// The vendored mockup stylesheet: the source of the class, token and state
/// expectations.
const REF_STYLESHEET: &str =
    include_str!("../../../docs/reference/opendesign-console/openkite-console.css");

/// The vendored mockup markup, pinned too, so a re-grab that only rewrites the
/// HTML still fails the contract.
const REF_MARKUP: &str =
    include_str!("../../../docs/reference/opendesign-console/openkite-console.html");

/// The human-owned half of the contract: `ref <file> sha256 <hex>` pins plus
/// `ref-only <selector> <reason>` lines for reference selectors the app does
/// not ship.
const DESIGN_CONTRACT_DOC: &str = include_str!("../../../docs/design-surfaces.md");

/// Class names that turn a base class into a state hook. A compound selector
/// whose last class is one of these is graded as a state (`.log-panel.open`);
/// a compound whose last class is not (`.pill.success`) is a variant and is
/// graded as an ordinary class.
const STATE_MODIFIERS: &[&str] = &[
    "active",
    "open",
    "show",
    "is-open",
    "is-active",
    "selected",
    "swiped",
    "paused",
    "collapsed",
    "hidden",
    "expanded",
];

/// Attribute selectors that carry state. `[data-label]` and `[tabindex]` are
/// layout/focus plumbing and are deliberately absent.
const STATE_ATTRIBUTES: &[&str] = &[
    "aria-current",
    "aria-selected",
    "aria-expanded",
    "aria-pressed",
    "aria-checked",
    "data-state",
    "data-disabled",
];

/// Tokens the *app* adds on top of the reference's `:root`. The reference is
/// the source of truth for every other custom property.
const APP_ONLY_TOKENS: &[&str] = &["--r-pill", "--on-strong"];

/// The parsed `design-contract` block.
#[derive(Debug)]
struct DesignContract {
    /// `(file name, pinned sha256)`.
    hashes: Vec<(String, String)>,
    /// `(selector, why the app does not ship it)`.
    ref_only: Vec<(String, String)>,
}

impl DesignContract {
    fn pinned_hash(&self, file: &str) -> Option<&str> {
        self.hashes
            .iter()
            .find(|(name, _)| name == file)
            .map(|(_, hash)| hash.as_str())
    }

    fn out_of_scope(&self, selector: &str) -> bool {
        self.ref_only.iter().any(|(listed, _)| listed == selector)
    }
}

/// Parse the fenced `design-contract` block out of `docs/design-surfaces.md`.
fn design_contract() -> DesignContract {
    let block = DESIGN_CONTRACT_DOC
        .split_once("```design-contract")
        .expect(
            "docs/design-surfaces.md must carry a ```design-contract block pinning the vendored \
             mockup hashes",
        )
        .1
        .split_once("```")
        .expect("the design-contract block must be closed by ```")
        .0;

    let mut hashes = Vec::new();
    let mut ref_only = Vec::new();
    for line in block.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("ref ") {
            let mut fields = rest.split_whitespace();
            let file = fields
                .next()
                .expect("`ref <file> sha256 <hex>`")
                .to_string();
            let algorithm = fields.next().expect("`ref <file> sha256 <hex>`");
            let hash = fields
                .next()
                .expect("`ref <file> sha256 <hex>`")
                .to_string();
            assert_eq!(
                algorithm, "sha256",
                "the contract pins sha256, got `{algorithm}`"
            );
            assert_eq!(
                hash.len(),
                64,
                "a sha256 digest is 64 hex chars, got `{hash}`"
            );
            hashes.push((file, hash));
        } else if let Some(rest) = line.strip_prefix("ref-only ") {
            let (selector, reason) = rest
                .split_once(' ')
                .expect("`ref-only <selector> <reason>` — every omission carries a reason");
            assert!(
                !reason.trim().is_empty(),
                "`ref-only {selector}` needs a reason; `n/a` is not one"
            );
            ref_only.push((selector.to_string(), reason.trim().to_string()));
        } else {
            panic!("unparsed design-contract line: `{line}`");
        }
    }
    assert!(!hashes.is_empty(), "the design-contract block pins no file");
    DesignContract { hashes, ref_only }
}

/// SHA-256 of the vendored bytes, hex encoded.
fn sha256_hex(bytes: &str) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

/// One whitespace-separated piece of a selector: the classes it targets and
/// the attributes it requires. `.nav-item[data-disabled]` is one piece with
/// classes `[nav-item]` and attrs `[data-disabled]`.
#[derive(Clone, Debug, Default)]
struct SelectorPiece {
    classes: Vec<String>,
    attrs: Vec<String>,
}

impl SelectorPiece {
    /// The form the `ref-only` list uses: `.log-panel.open`,
    /// `.nav-item[data-disabled]`.
    fn label(&self) -> String {
        let mut out = String::new();
        for class in &self.classes {
            out.push('.');
            out.push_str(class);
        }
        for attr in &self.attrs {
            out.push('[');
            out.push_str(attr);
            out.push(']');
        }
        out
    }
}

/// Every rule head in a stylesheet, at any nesting depth, with at-rule heads
/// (`@media …`, `@supports …`) dropped.
fn stylesheet_rule_heads(css: &str) -> Vec<String> {
    let bytes = css.as_bytes();
    let mut heads = Vec::new();
    let mut head_start = 0usize;
    for (i, &byte) in bytes.iter().enumerate() {
        match byte {
            b'{' => {
                let raw = strip_css_comments(&css[head_start..i]);
                let head = raw.trim();
                if !head.is_empty() && !head.starts_with('@') {
                    heads.push(head.to_string());
                }
                head_start = i + 1;
            }
            b'}' => head_start = i + 1,
            _ => {}
        }
    }
    heads
}

/// Every comma-separated selector in a stylesheet, split into pieces.
fn stylesheet_selectors(css: &str) -> Vec<Vec<SelectorPiece>> {
    let mut selectors = Vec::new();
    for head in stylesheet_rule_heads(css) {
        for selector in head.split(',') {
            let pieces: Vec<SelectorPiece> = selector
                .split_whitespace()
                .map(parse_selector_piece)
                .filter(|piece| !piece.classes.is_empty() || !piece.attrs.is_empty())
                .collect();
            if !pieces.is_empty() {
                selectors.push(pieces);
            }
        }
    }
    selectors
}

/// Parse one selector piece. Pseudo-classes and pseudo-elements end the piece
/// (`:hover`, `::before`, `:not(.x)`): their contents are not part of the class
/// contract.
fn parse_selector_piece(raw: &str) -> SelectorPiece {
    let bytes = raw.as_bytes();
    let mut piece = SelectorPiece::default();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'.' => {
                let start = i + 1;
                let mut end = start;
                while end < bytes.len() && is_css_ident_byte(bytes[end]) {
                    end += 1;
                }
                if end > start {
                    piece.classes.push(raw[start..end].to_string());
                }
                i = end;
            }
            b'[' => {
                let start = i + 1;
                let mut end = start;
                while end < bytes.len() && bytes[end] != b']' && bytes[end] != b'=' {
                    end += 1;
                }
                if end > start {
                    piece.attrs.push(raw[start..end].trim().to_string());
                }
                while end < bytes.len() && bytes[end] != b']' {
                    end += 1;
                }
                i = end + 1;
            }
            b':' => break,
            _ => i += 1,
        }
    }
    let mut seen = std::collections::HashSet::new();
    piece.classes.retain(|class| seen.insert(class.clone()));
    piece
}

fn is_css_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

/// Every class the reference stylesheet defines a rule for.
fn reference_classes() -> std::collections::BTreeSet<String> {
    stylesheet_selectors(REF_STYLESHEET)
        .into_iter()
        .flatten()
        .flat_map(|piece| piece.classes)
        .collect()
}

/// Every custom property the reference's `:root` declares.
fn reference_tokens() -> std::collections::BTreeSet<String> {
    declared_root_properties(REF_STYLESHEET)
        .into_iter()
        .collect()
}

/// The reference's state selectors: `.base.modifier` compounds plus
/// `.base[state-attr]` pieces.
fn reference_states() -> Vec<SelectorPiece> {
    let mut states: Vec<SelectorPiece> = Vec::new();
    for piece in stylesheet_selectors(REF_STYLESHEET).into_iter().flatten() {
        let modifier = piece.classes.len() >= 2
            && piece
                .classes
                .last()
                .is_some_and(|last| STATE_MODIFIERS.contains(&last.as_str()));
        let state_attr = !piece.classes.is_empty()
            && piece
                .attrs
                .iter()
                .any(|attr| STATE_ATTRIBUTES.contains(&attr.as_str()));
        if modifier || state_attr {
            states.push(piece);
        }
    }
    states.sort_by_key(SelectorPiece::label);
    states.dedup_by_key(|piece| piece.label());
    states
}

/// Every class `assets/main.css` defines a rule for.
fn main_css_classes() -> std::collections::BTreeSet<String> {
    stylesheet_selectors(STYLESHEET)
        .into_iter()
        .flatten()
        .flat_map(|piece| piece.classes)
        .collect()
}

/// Whether some rule in `css` targets a compound that requires every class and
/// attribute in `state`.
fn stylesheet_covers_state(css: &str, state: &SelectorPiece) -> bool {
    stylesheet_selectors(css)
        .into_iter()
        .flatten()
        .any(|piece| {
            state
                .classes
                .iter()
                .all(|class| piece.classes.contains(class))
                && state.attrs.iter().all(|attr| piece.attrs.contains(attr))
        })
}

/// The `:root` block's custom properties, in declaration order.
fn declared_root_properties(css: &str) -> Vec<String> {
    let stripped = strip_css_comments(css);
    let Some(root_start) = stripped.find(":root") else {
        return Vec::new();
    };
    let Some(open) = stripped[root_start..].find('{') else {
        return Vec::new();
    };
    let body_start = root_start + open + 1;
    let Some(close) = stripped[body_start..].find('}') else {
        return Vec::new();
    };
    stripped[body_start..body_start + close]
        .split(';')
        .filter_map(|declaration| {
            let (name, _) = declaration.split_once(':')?;
            let name = name.trim();
            name.starts_with("--").then(|| name.to_string())
        })
        .collect()
}

/// The expected `:root` property set: opaline's theme contract, the design
/// tokens the reference declares, and the app's own additions.
fn expected_root_properties() -> Vec<String> {
    let mut expected: Vec<String> = openkite_ui::theme::CSS_VARS
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    expected.extend(reference_tokens());
    expected.extend(APP_ONLY_TOKENS.iter().map(|name| (*name).to_string()));
    expected.sort();
    expected.dedup();
    expected
}

#[test]
fn the_vendored_reference_matches_its_pinned_hashes() {
    let contract = design_contract();
    let vendored = [
        ("openkite-console.css", REF_STYLESHEET),
        ("openkite-console.html", REF_MARKUP),
    ];

    for (name, _) in &contract.hashes {
        assert!(
            vendored
                .iter()
                .any(|(vendored_name, _)| vendored_name == name),
            "the design-contract block pins `{name}`, which is not one of the vendored \
             contract files"
        );
    }

    let mut mismatches = Vec::new();
    for (name, bytes) in vendored {
        let actual = sha256_hex(bytes);
        match contract.pinned_hash(name) {
            Some(pinned) if pinned == actual => {}
            Some(pinned) => mismatches.push(format!("{name}: pinned {pinned}, vendored {actual}")),
            None => mismatches.push(format!("{name}: no `ref` pin in docs/design-surfaces.md")),
        }
    }

    let repin = vendored
        .iter()
        .map(|(name, bytes)| format!("ref {name} sha256 {}", sha256_hex(bytes)))
        .collect::<Vec<_>>()
        .join("\n  ");
    assert!(
        mismatches.is_empty(),
        "the vendored mockup no longer matches its pinned hash. Re-derive the contract — do \
         not just edit the hash:\n  {}\n\nRe-derived pins for docs/design-surfaces.md:\n  \
         {repin}\n\nThen re-check the `ref-only` lines and the class/token/state failures below, \
         and say in the PR what moved in the mockup.",
        mismatches.join("\n  ")
    );
}

#[test]
fn every_reference_class_is_covered_by_main_css() {
    let contract = design_contract();
    let main_classes = main_css_classes();
    let mut missing = Vec::new();
    for class in reference_classes() {
        if contract.out_of_scope(&format!(".{class}")) {
            continue;
        }
        if !main_classes.contains(&class) {
            missing.push(class);
        }
    }
    assert!(
        missing.is_empty(),
        "the reference mockup styles these classes and assets/main.css does not:\n  {}\n\n\
         Either ship the rule (the usual fix) or add a `ref-only <selector> <reason>` line to the \
         design-contract block.",
        missing
            .iter()
            .map(|class| format!(".{class}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn every_reference_token_is_declared_by_main_css() {
    let declared: std::collections::BTreeSet<String> =
        declared_root_properties(STYLESHEET).into_iter().collect();
    let missing = reference_tokens()
        .difference(&declared)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "the reference `:root` declares these custom properties and assets/main.css does not: \
         {missing:?}"
    );
}

#[test]
fn every_reference_state_selector_is_covered_or_declared_out_of_scope() {
    let contract = design_contract();
    let mut missing = Vec::new();
    for state in reference_states() {
        let label = state.label();
        if contract.out_of_scope(&label) {
            continue;
        }
        if !stylesheet_covers_state(STYLESHEET, &state) {
            missing.push(label);
        }
    }
    assert!(
        missing.is_empty(),
        "the reference draws these states and assets/main.css has no rule for them:\n  {}\n\n\
         Ship the compound rule, or record why the app expresses the state another way with a \
         `ref-only <selector> <reason>` line.",
        missing.join("\n  ")
    );
}

#[test]
fn the_contract_derives_a_plausible_set_from_the_reference() {
    let classes = reference_classes();
    assert!(
        classes.len() >= 100,
        "class derivation returned {} names — the parser is broken, not the mockup",
        classes.len()
    );
    for known in ["panel", "nav-item", "log-panel", "resource-table"] {
        assert!(
            classes.contains(known),
            "class derivation missed `.{known}`, which the mockup styles"
        );
    }

    let tokens = reference_tokens();
    assert!(
        tokens.len() >= 25,
        "token derivation returned {} names — the parser is broken, not the mockup",
        tokens.len()
    );
    for known in ["--brand", "--shadow-terminal"] {
        assert!(tokens.contains(known), "token derivation missed `{known}`");
    }

    let states = reference_states();
    assert!(
        states.len() >= 8,
        "state derivation returned {} selectors — the parser is broken, not the mockup",
        states.len()
    );
    assert!(
        states
            .iter()
            .any(|state| state.label() == ".nav-item.active"),
        "state derivation missed `.nav-item.active`"
    );
}

#[test]
fn every_out_of_scope_selector_still_exists_in_the_reference() {
    let contract = design_contract();
    let mut known: std::collections::BTreeSet<String> = reference_classes()
        .iter()
        .map(|class| format!(".{class}"))
        .collect();
    known.extend(reference_states().iter().map(SelectorPiece::label));

    let stale: Vec<&str> = contract
        .ref_only
        .iter()
        .map(|(selector, _)| selector.as_str())
        .filter(|selector| !known.contains(*selector))
        .collect();
    assert!(
        stale.is_empty(),
        "these `ref-only` entries no longer match anything in the vendored mockup, so the scope \
         list is stale: {stale:?} — re-derive the block when the reference changes"
    );
}

#[test]
fn every_contract_property_is_declared_exactly_once() {
    for name in expected_root_properties() {
        let count = STYLESHEET.matches(&format!("{name}:")).count();
        assert_eq!(
            count, 1,
            "`{name}` is declared {count} times in assets/main.css (expected exactly 1)"
        );
    }
}

/// Every base+state pair the crate renders needs a compound rule. A surface
/// that renders the base class but never the state class is exactly the drift
/// this catches: the base rule keeps the element on screen, so nothing looks
/// broken until someone toggles the state.
#[test]
fn every_rendered_base_and_state_pair_has_a_rule() {
    let mut pairs: std::collections::BTreeMap<(String, String), &'static str> =
        std::collections::BTreeMap::new();
    for &(source_path, source) in RSX_SOURCES {
        let mut record = |value: &str| {
            let tokens: Vec<String> = tokenize_class_value(value).collect();
            let Some(base) = tokens.first() else {
                return;
            };
            for modifier in &tokens[1..] {
                if STATE_MODIFIERS.contains(&modifier.as_str()) {
                    pairs
                        .entry((base.clone(), modifier.clone()))
                        .or_insert(source_path);
                }
            }
        };
        for_each_class_literal(source, &mut record);
        for_each_class_ternary(source, |if_true, if_false| {
            record(if_true);
            record(if_false);
        });
    }

    assert!(
        !pairs.is_empty(),
        "the rsx walker found no base+state pair — the walker is broken, not the crate"
    );

    let missing: Vec<String> = pairs
        .iter()
        .filter(|((base, modifier), _)| {
            !stylesheet_covers_state(
                STYLESHEET,
                &SelectorPiece {
                    classes: vec![base.clone(), modifier.clone()],
                    attrs: Vec::new(),
                },
            )
        })
        .map(|((base, modifier), source)| {
            format!("  - `.{base}.{modifier}` (rendered from {source})")
        })
        .collect();
    assert!(
        missing.is_empty(),
        "the crate renders these base+state pairs but assets/main.css has no compound rule for \
         them:\n{}",
        missing.join("\n")
    );
}

#[test]
fn root_declares_only_the_theme_contract_and_the_design_system_properties() {
    let mut declared = declared_root_properties(STYLESHEET);
    declared.sort();
    declared.dedup();

    assert_eq!(
        declared,
        expected_root_properties(),
        "assets/main.css :root and the contract's token tables disagree — a name exists in one \
         layer but not the other"
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
const DOCK_RSX: &str = include_str!("../src/components/dock.rs");
const APP_CARD_RSX: &str = include_str!("../src/components/app_card.rs");
const POD_DETAIL_RSX: &str = include_str!("../src/components/pod_detail.rs");
const OVERVIEW_RSX: &str = include_str!("../src/components/overview.rs");

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
    ("crates/openkite-ui/src/components/dock.rs", DOCK_RSX),
    (
        "crates/openkite-ui/src/components/app_card.rs",
        APP_CARD_RSX,
    ),
    (
        "crates/openkite-ui/src/components/pod_detail.rs",
        POD_DETAIL_RSX,
    ),
    (
        "crates/openkite-ui/src/components/overview.rs",
        OVERVIEW_RSX,
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

/// Split a stylesheet into bare class tokens and compound class sets.
///
/// Uses [`stylesheet_rule_heads`], so rules nested inside `@media`/
/// `@supports` are collected too and at-rule heads are dropped.
fn parse_stylesheet_selectors(css: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut bare: Vec<String> = Vec::new();
    let mut compound: Vec<Vec<String>> = Vec::new();
    for selector in stylesheet_selectors(css) {
        let mut classes: Vec<String> = Vec::new();
        for piece in selector {
            for class in piece.classes {
                if !classes.contains(&class) {
                    classes.push(class);
                }
            }
        }
        match classes.len() {
            0 => {}
            1 => bare.push(classes.into_iter().next().expect("non-empty")),
            _ => compound.push(classes),
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
        row.contains("scroll-snap-type: x mandatory"),
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
        chip.contains("scroll-snap-stop: always"),
        "a flick cannot carry past a chip's snap point: {chip}"
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

/// The ≤767px half of the shell (OKT-166): the bar is a below-768px
/// affordance and the view scrolls clear of it.
#[test]
fn the_bottom_bar_lives_below_the_768px_breakpoint() {
    assert!(
        STYLESHEET.contains(".main { padding-bottom: 74px; }"),
        "the view must clear the fixed bar"
    );
    assert!(
        STYLESHEET.contains(".bottom-nav { display: block; }"),
        "the bar is shown below 768px"
    );
    assert!(
        STYLESHEET.contains("@media (min-width: 768px) {\n  .bottom-nav { display: none; }\n}"),
        "and reset from 768px up"
    );
}
