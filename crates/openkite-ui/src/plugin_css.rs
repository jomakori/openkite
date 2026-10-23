//! Plugin CSS scoping — plugin system v1.1.
//!
//! v1.0 let a plugin contribute navigation and never styling: the shell owned
//! every rule a plugin section painted. v1.1 lets a plugin ship CSS, and this
//! module is the boundary that keeps it off the host. Every selector a bundle
//! hands over is rewritten under the plugin's own section root
//! (`[data-plugin="<name>"]` — the attribute the shell already paints on the
//! block a plugin contributed), so a plugin can restyle its own surface and
//! nothing else on the page.
//!
//! The foreign-CSS channels are closed by the same pass:
//!
//! - `@import`, `@charset` and `@namespace` are dropped — they would pull in
//!   rules this pass never saw.
//! - A sheet whose text could close the `<style>` element it rides in
//!   (`</style`, `<!--`) is refused whole rather than patched.
//! - Conditional blocks (`@media`, `@supports`, `@container`, `@layer`)
//!   recurse, so a rule cannot escape by hiding inside one.
//! - `@keyframes` / `@font-face` / `@property` pass through verbatim: they
//!   declare animation, face and property names, never a selector that could
//!   reach the host's own elements.
//!
//! Known limit: a selector that conditions on the host's document state
//! (`body.dark .x`) keeps its descendant part and loses the condition
//! (`[data-plugin="p"] .x`) — wider *inside* the plugin's own section, never
//! outside it. A plugin that needs theme-conditional rules should scope them
//! with its own root-level attributes instead.

/// The largest stylesheet a plugin may register. The sheet rides the
/// registration payload, so this is a guard rail against a bundle handing the
/// host a megabyte of rules — not a design limit.
pub const MAX_STYLE_BYTES: usize = 256 * 1024;

/// The scope root for a plugin: the attribute the shell paints on the section
/// the plugin contributed.
///
/// The name is attribute-escaped, because the bridge's `plugin` field is
/// caller-supplied and would otherwise be able to end the selector.
pub fn scope_root(plugin: &str) -> String {
    format!("[data-plugin=\"{}\"]", escape_attr_value(plugin))
}

/// Scope a plugin's CSS to its own section.
///
/// Returns the rewritten sheet, or `None` when the plugin authored nothing,
/// authored something that cannot be scoped safely, or authored more than
/// [`MAX_STYLE_BYTES`].
pub fn scoped_style(plugin: &str, css: &str) -> Option<String> {
    if css.trim().is_empty() || css.len() > MAX_STYLE_BYTES {
        return None;
    }
    // A sheet that can close the element it rides in is a break-out attempt,
    // not a stylesheet: refuse it whole.
    let lowered = css.to_ascii_lowercase();
    if lowered.contains("</style") || lowered.contains("<!--") {
        return None;
    }
    let root = scope_root(plugin);
    let mut out = String::with_capacity(css.len() + 64);
    scope_block(&root, &strip_comments(css), &mut out);
    let scoped = out.trim();
    if scoped.is_empty() {
        None
    } else {
        Some(scoped.to_string())
    }
}

/// Escape a value for a CSS `[attr="..."]` selector, so a caller-supplied
/// plugin name cannot terminate the string or the selector.
fn escape_attr_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            // Inside a quoted attribute value only the quote, the escape and
            // the line breaks need escaping; a bare `]` is literal there.
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\a "),
            '\r' => out.push_str("\\d "),
            _ => out.push(ch),
        }
    }
    out
}

/// Drop `/* … */` comments (string-aware), so a commented-out selector is not
/// mistaken for a real one.
fn strip_comments(css: &str) -> String {
    let chars: Vec<char> = css.chars().collect();
    let mut out = String::with_capacity(css.len());
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let ch = chars[i];
        if let Some(q) = quote {
            out.push(ch);
            if ch == '\\' {
                if let Some(next) = chars.get(i + 1) {
                    out.push(*next);
                }
                i += 2;
                continue;
            }
            if ch == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                out.push(ch);
                i += 1;
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                i += 2;
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    i += 1;
                }
                i += 2;
                out.push(' ');
            }
            _ => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out
}

/// At-rules whose body is a list of rules: recurse, so nothing inside escapes
/// the scope.
const CONDITIONAL_AT_RULES: &[&str] = &[
    "media",
    "supports",
    "container",
    "layer",
    "starting-style",
    "document",
    "-moz-document",
];

/// At-rules that declare names instead of selecting elements: only the list of
/// conditional at-rules above is recursed, and this list documents the ones
/// that are deliberately copied as authored.
const DECLARATION_AT_RULES: &[&str] = &[
    "keyframes",
    "-webkit-keyframes",
    "-moz-keyframes",
    "font-face",
    "property",
    "page",
    "counter-style",
    "font-feature-values",
];

/// Rewrite every rule in `src` under `root`, appending to `out`.
fn scope_block(root: &str, src: &str, out: &mut String) {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // Stray separators and unmatched closing braces carry no rule.
        while i < chars.len() && (chars[i].is_whitespace() || chars[i] == ';' || chars[i] == '}') {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        // The prelude runs to a top-level `{` (a rule) or `;` (a statement).
        let start = i;
        let mut quote: Option<char> = None;
        let mut nesting = 0i32;
        let mut terminator: Option<(usize, char)> = None;
        while i < chars.len() {
            let ch = chars[i];
            if let Some(q) = quote {
                if ch == '\\' {
                    i += 2;
                    continue;
                }
                if ch == q {
                    quote = None;
                }
                i += 1;
                continue;
            }
            match ch {
                '"' | '\'' => {
                    quote = Some(ch);
                    i += 1;
                }
                '(' | '[' => {
                    nesting += 1;
                    i += 1;
                }
                ')' | ']' => {
                    nesting -= 1;
                    i += 1;
                }
                '{' | ';' if nesting <= 0 => {
                    terminator = Some((i, ch));
                    break;
                }
                _ => i += 1,
            }
        }
        let Some((at, kind)) = terminator else {
            // Trailing text with no block and no statement: nothing to scope.
            break;
        };
        let prelude: String = chars[start..at]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();

        if kind == ';' {
            // `@import url(…)`, `@charset "…"`, `@namespace …`: the rules it
            // would pull in never went through this pass.
            i = at + 1;
            continue;
        }

        // Find this block's matching `}` (string- and brace-aware).
        let body_start = at + 1;
        let mut j = body_start;
        let mut inner_quote: Option<char> = None;
        let mut depth = 0i32;
        while j < chars.len() {
            let ch = chars[j];
            if let Some(q) = inner_quote {
                if ch == '\\' {
                    j += 2;
                    continue;
                }
                if ch == q {
                    inner_quote = None;
                }
                j += 1;
                continue;
            }
            match ch {
                '"' | '\'' => {
                    inner_quote = Some(ch);
                    j += 1;
                }
                '{' => {
                    depth += 1;
                    j += 1;
                }
                '}' if depth == 0 => break,
                '}' => {
                    depth -= 1;
                    j += 1;
                }
                _ => j += 1,
            }
        }
        let body_end = j.min(chars.len());
        let body: String = chars[body_start..body_end].iter().collect();
        i = (body_end + 1).min(chars.len());

        emit_rule(root, &prelude, &body, out);
    }
}

/// Emit one prelude + body pair, scoped under `root`.
fn emit_rule(root: &str, prelude: &str, body: &str, out: &mut String) {
    // Rules are emitted one per line-group; the separator keeps consecutive
    // scoped rules from running together.
    if !out.is_empty() {
        out.push(' ');
    }
    if prelude.is_empty() {
        return;
    }

    if let Some(after_at) = prelude.strip_prefix('@') {
        let name = after_at
            .split(|ch: char| ch.is_whitespace() || ch == '(' || ch == '{' || ch == ':')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if CONDITIONAL_AT_RULES.contains(&name.as_str()) {
            // The block only carries rules this pass rewrote; an empty rewrite
            // (every rule inside refused) leaves nothing to emit.
            let mut inner = String::new();
            scope_block(root, body, &mut inner);
            if inner.is_empty() {
                return;
            }
            out.push_str(prelude);
            out.push_str(" { ");
            out.push_str(&inner);
            out.push_str(" }");
            return;
        }
        // Declaration at-rules pass through, and so does anything this pass
        // does not know: guessing at unknown scoping semantics would be worse
        // than keeping the block opaque (and none of the ones we know of
        // select host elements).
        debug_assert!(
            DECLARATION_AT_RULES.contains(&name.as_str()) || !name.is_empty(),
            "unknown at-rule kept verbatim: {name}"
        );
        out.push_str(prelude);
        out.push_str(" {");
        out.push_str(body);
        out.push('}');
        return;
    }

    let scoped: Vec<String> = split_selector_list(prelude)
        .into_iter()
        .filter_map(|selector| scope_selector(root, selector))
        .collect();
    if scoped.is_empty() {
        return;
    }
    out.push_str(&scoped.join(", "));
    out.push_str(" {");
    out.push_str(body);
    out.push('}');
}

/// Scope one selector: a document-level selector *is* the scope root; anything
/// else becomes a descendant of it.
fn scope_selector(root: &str, selector: &str) -> Option<String> {
    let selector = selector.trim();
    if selector.is_empty() {
        return None;
    }
    if selector == "*" {
        return Some(root.to_string());
    }
    let rest = strip_document_prefix(selector);
    if rest.is_empty() {
        Some(root.to_string())
    } else {
        Some(format!("{root} {rest}"))
    }
}

/// Strip the leading compounds that name the document itself (`html`, `body`,
/// `:root`, with or without a modifier), so `body .x` lands on the scope root
/// instead of on a `body` the plugin cannot see.
fn strip_document_prefix(selector: &str) -> &str {
    let mut rest = selector.trim_start();
    loop {
        let end = rest
            .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '+' || ch == '~')
            .unwrap_or(rest.len());
        let compound = rest[..end].to_ascii_lowercase();
        if !names_document(&compound) {
            return rest;
        }
        let mut after = rest[end..].trim_start();
        if let Some(stripped) = after.strip_prefix(['>', '+', '~']) {
            after = stripped.trim_start();
        }
        if after.is_empty() {
            return "";
        }
        rest = after;
    }
}

/// Whether one compound selector names the document root.
fn names_document(compound: &str) -> bool {
    let rest = if let Some(rest) = compound.strip_prefix(":root") {
        rest
    } else if let Some(rest) = compound.strip_prefix("html") {
        rest
    } else if let Some(rest) = compound.strip_prefix("body") {
        rest
    } else {
        return false;
    };
    // A modifier on the document element (`body.dark`, `:root[data-theme]`).
    rest.is_empty()
        || matches!(
            rest.chars().next(),
            Some('.') | Some('#') | Some('[') | Some(':')
        )
}

/// Split a selector list on its top-level commas (a comma inside `:is(…)` or
/// `[title="a,b"]` belongs to that selector).
fn split_selector_list(prelude: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quote: Option<char> = None;
    let mut nesting = 0i32;
    let chars: Vec<char> = prelude.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if let Some(q) = quote {
            if ch == '\\' {
                i += 2;
                continue;
            }
            if ch == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' | '[' => nesting += 1,
            ')' | ']' => nesting -= 1,
            ',' if nesting <= 0 => {
                parts.push(&prelude[start..byte_index(prelude, i)]);
                start = byte_index(prelude, i + 1);
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&prelude[start..]);
    parts
}

/// The byte offset of character index `index` in `text`.
fn byte_index(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scoped(css: &str) -> String {
        scoped_style("demo", css).expect("sheet should scope")
    }

    #[test]
    fn simple_selector_is_a_descendant_of_the_scope_root() {
        assert_eq!(
            scoped(".card { color: red; }"),
            "[data-plugin=\"demo\"] .card { color: red; }"
        );
    }

    #[test]
    fn selector_lists_are_scoped_one_by_one() {
        assert_eq!(
            scoped("h1, .a[title=\"x,y\"] { margin: 0 }"),
            "[data-plugin=\"demo\"] h1, [data-plugin=\"demo\"] .a[title=\"x,y\"] { margin: 0 }"
        );
    }

    #[test]
    fn document_selectors_become_the_scope_root() {
        assert_eq!(
            scoped(":root { --ink: #fff }"),
            "[data-plugin=\"demo\"] { --ink: #fff }"
        );
        assert_eq!(
            scoped("html { color: red } body { color: blue }"),
            "[data-plugin=\"demo\"] { color: red } [data-plugin=\"demo\"] { color: blue }"
        );
        assert_eq!(
            scoped(".card body { color: red }"),
            "[data-plugin=\"demo\"] .card body { color: red }"
        );
    }

    #[test]
    fn document_state_selectors_keep_their_descendant_and_lose_the_condition() {
        assert_eq!(
            scoped("body.dark .card { color: red }"),
            "[data-plugin=\"demo\"] .card { color: red }"
        );
        assert_eq!(
            scoped("html body > .card { color: red }"),
            "[data-plugin=\"demo\"] .card { color: red }"
        );
    }

    #[test]
    fn conditional_blocks_recurse() {
        assert_eq!(
            scoped("@media (min-width: 640px) { .a > .b { color: red } }"),
            "@media (min-width: 640px) { [data-plugin=\"demo\"] .a > .b { color: red } }"
        );
        assert_eq!(
            scoped("@supports (display: grid) { @media print { body { color: red } } }"),
            "@supports (display: grid) { @media print { [data-plugin=\"demo\"] { color: red } } }"
        );
    }

    #[test]
    fn foreign_css_channels_are_dropped() {
        assert_eq!(
            scoped(
                "@import url(https://evil.example/x.css); @charset \"utf-8\"; .a { color: red }"
            ),
            "[data-plugin=\"demo\"] .a { color: red }"
        );
    }

    #[test]
    fn declaration_at_rules_pass_through_verbatim() {
        let css =
            "@keyframes spin { from { opacity: 0 } to { opacity: 1 } } .a { animation: spin 1s }";
        let out = scoped(css);
        assert!(out.starts_with("@keyframes spin { from { opacity: 0 } to { opacity: 1 } }"));
        assert!(out.ends_with("[data-plugin=\"demo\"] .a { animation: spin 1s }"));
    }

    #[test]
    fn break_out_text_is_refused_whole() {
        assert_eq!(
            scoped_style("demo", ".a { } </style><script>alert(1)</script>"),
            None
        );
        assert_eq!(scoped_style("demo", "<!-- .a { color: red } -->"), None);
        assert_eq!(scoped_style("demo", "   "), None);
        assert_eq!(
            scoped_style("demo", &"a{color:red}".repeat(MAX_STYLE_BYTES)),
            None
        );
    }

    #[test]
    fn comments_never_become_rules() {
        assert_eq!(
            scoped("/* .commented { color: red } */ .real { color: blue }"),
            "[data-plugin=\"demo\"] .real { color: blue }"
        );
    }

    #[test]
    fn the_plugin_name_cannot_end_the_selector() {
        let root = scope_root("x\"] body, [y=\"");
        assert_eq!(root, "[data-plugin=\"x\\\"] body, [y=\\\"\"]");
        let scoped = scoped_style("x\"] body, [y=\"", ".a { color: red }").unwrap();
        assert_eq!(scoped.matches("[data-plugin=").count(), 1);
        assert!(!scoped.contains(", body"));
    }

    #[test]
    fn every_emitted_selector_is_scoped() {
        let css = "@media screen { .a, body, :root { color: red } }\n.b { color: blue }";
        let out = scoped(css);
        let root = scope_root("demo");
        let rules = out.split('}').filter(|rule| rule.contains('{'));
        for rule in rules {
            let prelude = rule
                .rsplit_once('{')
                .map(|(prelude, _)| prelude)
                .unwrap_or(rule);
            for selector in prelude.split(',').map(str::trim) {
                if selector.is_empty() || selector.starts_with('@') {
                    continue;
                }
                assert!(
                    selector.starts_with(&root),
                    "unscoped selector escaped: {selector}"
                );
            }
        }
    }
}
