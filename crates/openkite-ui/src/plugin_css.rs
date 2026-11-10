//! Plugin CSS scoping — the single implementation for desktop and web.
//!
//! Every selector a plugin ships is rewritten under that plugin's own section
//! root, so it can restyle its own surface and nothing else on the page.
//!
//! Foreign-CSS channels are closed rather than patched: `@import`, `@charset`
//! and `@namespace` reach rules this pass never saw; `@font-face`,
//! `@keyframes` and `@property` declare names that are global by spec, so
//! nesting them cannot isolate them; and a selector whose first compound is
//! `html`/`body` targets the host document that no attribute scope can reach.
//! Each of those is refused with a reason, and the caller disables that
//! plugin's stylesheet instead of claiming an isolation it cannot deliver.
//!
//! Conditional blocks (`@media`, `@supports`, `@container`) recurse, so a rule
//! cannot escape by hiding inside one. `:root` maps onto the plugin root, and
//! selector lists are split on top-level commas so `:not(.a, .b)` survives.
//!
//! A sheet that could close the `<style>` element it rides in (`</style`,
//! `<!--`) is refused whole, and a sheet larger than [`MAX_STYLE_BYTES`] is
//! refused before it is parsed — it rides the registration payload.

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

/// Scope a plugin's stylesheet to its own section.
///
/// `Err` carries the reason the sheet cannot be scoped; the caller disables
/// the plugin with that reason instead of shipping unscoped CSS.
pub fn scope_css(plugin: &str, css: &str) -> Result<String, String> {
    if css.trim().is_empty() {
        return Err(format!("plugin '{plugin}' declared an empty stylesheet"));
    }
    if css.len() > MAX_STYLE_BYTES {
        return Err(format!(
            "plugin '{plugin}' stylesheet is {} bytes, over the {} byte limit",
            css.len(),
            MAX_STYLE_BYTES
        ));
    }
    let lowered = css.to_ascii_lowercase();
    if lowered.contains("</style") || lowered.contains("<!--") {
        return Err(format!(
            "plugin '{plugin}' stylesheet could close the element it rides in"
        ));
    }
    let root = scope_root(plugin);
    scope_rules(&strip_css_comments(css), &root, plugin)
}

/// Scoped sheet for a plugin, or `None` when it cannot be scoped.
pub fn scoped_style(plugin: &str, css: &str) -> Option<String> {
    scope_css(plugin, css).ok()
}

/// Strip `/* ... */` comments (string-unaware: plugin CSS is not expected to
/// carry `*/` inside a string literal, and stripping comments first keeps
/// the brace scanner below simple).
fn strip_css_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut chars = css.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(c) = chars.next() {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Split `css` into top-level `(prelude, body)` rules, where `body` is the
/// text between a rule's `{` and matching `}`. Depth-aware over `()`/`[]`
/// and string quotes, so a prelude like `:not(.a, .b)` or a `url("{")` never
/// confuses the brace match. A top-level `;` before any `{` (e.g.
/// `@import "x";`) is rejected rather than silently dropped.
fn split_rules(css: &str) -> Result<Vec<(String, String)>, String> {
    let chars: Vec<char> = css.chars().collect();
    let mut rules = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let start = i;
        let mut depth = 0i32;
        let mut quote: Option<char> = None;
        while i < chars.len() {
            let c = chars[i];
            if let Some(q) = quote {
                if c == q && chars[i - 1] != '\\' {
                    quote = None;
                }
            } else {
                match c {
                    '"' | '\'' => quote = Some(c),
                    '(' | '[' => depth += 1,
                    ')' | ']' => depth -= 1,
                    '{' if depth == 0 => break,
                    ';' if depth == 0 => {
                        let stmt: String = chars[start..i].iter().collect();
                        return Err(format!(
                            "unsupported statement in plugin stylesheet: '{}'",
                            stmt.trim()
                        ));
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        if i >= chars.len() {
            let rest: String = chars[start..].iter().collect();
            if rest.trim().is_empty() {
                break;
            }
            return Err(format!("unterminated rule: '{}'", rest.trim()));
        }
        let prelude: String = chars[start..i].iter().collect();
        let body_start = i + 1;
        let mut body_depth = 1i32;
        let mut j = body_start;
        let mut quote: Option<char> = None;
        while j < chars.len() && body_depth > 0 {
            let c = chars[j];
            if let Some(q) = quote {
                if c == q && chars[j - 1] != '\\' {
                    quote = None;
                }
            } else {
                match c {
                    '"' | '\'' => quote = Some(c),
                    '{' => body_depth += 1,
                    '}' => body_depth -= 1,
                    _ => {}
                }
            }
            j += 1;
        }
        if body_depth != 0 {
            return Err(format!("unterminated block for '{}'", prelude.trim()));
        }
        let body: String = chars[body_start..j - 1].iter().collect();
        rules.push((prelude.trim().to_string(), body));
        i = j;
    }
    Ok(rules)
}

/// Scope every rule in `css`, recursing into the rule-container at-rules.
fn scope_rules(css: &str, root: &str, plugin: &str) -> Result<String, String> {
    let mut out = String::new();
    for (prelude, body) in split_rules(css)? {
        if let Some(rest) = prelude.strip_prefix('@') {
            let (keyword, params) = split_at_rule(rest);
            match keyword.to_ascii_lowercase().as_str() {
                "media" | "supports" | "container" => {
                    let inner = scope_rules(&body, root, plugin)?;
                    out.push_str(&format!("@{keyword} {params} {{\n{inner}}}\n"));
                }
                "font-face" => {
                    return Err(format!(
                        "@font-face is global and cannot be scoped to plugin '{plugin}'s subtree"
                    ));
                }
                "keyframes" | "-webkit-keyframes" | "-moz-keyframes" => {
                    return Err(format!(
                        "@{keyword} names are global and cannot be scoped to plugin '{plugin}'s subtree"
                    ));
                }
                "import" => {
                    return Err(format!(
                        "@import reaches outside plugin '{plugin}'s own files and is not supported"
                    ));
                }
                other => {
                    return Err(format!(
                        "unsupported at-rule '@{other}' in plugin '{plugin}'s stylesheet"
                    ));
                }
            }
        } else {
            let selector = scope_selector_list(&prelude, root, plugin)?;
            out.push_str(&format!("{selector} {{{body}}}\n"));
        }
    }
    Ok(out)
}

/// Split an at-rule prelude (after the `@`) into its keyword and the rest
/// (the part between the keyword and the `{`), e.g. `media (min-width: 1px)`
/// → `("media", "(min-width: 1px)")`.
fn split_at_rule(s: &str) -> (&str, &str) {
    match s.find(|c: char| c.is_whitespace() || c == '(') {
        Some(idx) => (&s[..idx], s[idx..].trim()),
        None => (s, ""),
    }
}

/// Scope a comma-separated selector list under `root`.
fn scope_selector_list(selector_list: &str, root: &str, plugin: &str) -> Result<String, String> {
    let mut scoped = Vec::new();
    for part in split_top_level_commas(selector_list) {
        let sel = part.trim();
        if sel.is_empty() {
            continue;
        }
        if sel == ":root" {
            scoped.push(root.to_string());
            continue;
        }
        let first = sel
            .split(|c: char| c.is_whitespace() || c == '>' || c == '+' || c == '~')
            .next()
            .unwrap_or("")
            .trim_start_matches('*');
        let targets_host = |tag: &str| {
            let lower = first.to_ascii_lowercase();
            let tag_lower = tag.to_ascii_lowercase();
            lower == tag_lower
                || lower.starts_with(&format!("{tag_lower}."))
                || lower.starts_with(&format!("{tag_lower}#"))
                || lower.starts_with(&format!("{tag_lower}:"))
        };
        if targets_host("html") || targets_host("body") {
            return Err(format!(
                "selector '{sel}' in plugin '{plugin}'s stylesheet targets the host document; no plugin scope can reach it"
            ));
        }
        scoped.push(format!("{root} {sel}"));
    }
    Ok(scoped.join(", "))
}

/// Split a selector list on top-level commas (depth-aware over `()`/`[]`, so
/// `:not(.a, .b)` is not mistaken for two selectors).
fn split_top_level_commas(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for c in s.chars() {
        match c {
            '(' | '[' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    parts.push(current);
    parts
}
