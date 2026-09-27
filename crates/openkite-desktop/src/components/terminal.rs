//! Vendored xterm.js surface: cache-buster id and the JS snippet builders
//! the terminal view evals to bootstrap and control the bundle.
//!
//! The bundle lives in `assets/vendored/xterm/` (see its `SOURCE.txt` for
//! the rebuild command) and exposes three functions on `window.openkite`:
//! `_term_mount(selector)`, `_term_reset(selector)`, `_term_writeln(selector,
//! text)`. The builders here are pure string functions so the selector
//! plumbing is unit-testable without a Dioxus runtime.

/// Cache-buster id for the vendored xterm bundle. Bump the `vN` suffix
/// whenever the bundle in `assets/vendored/xterm/` is rebuilt (the
/// `SOURCE.txt` in that directory records the rebuild command). The
/// bootstrap effect reads this id to guard the
/// `window.__openkite_xterm_loaded` flag and the `<html>` class.
pub fn xterm_host_path() -> &'static str {
    "xterm-bundle-v1"
}

/// The JS that injects the vendored CSS + JS bundle once per webview load.
///
/// CSS is guarded by an `<html>` class (so a re-render never appends a
/// second stylesheet); the bundle eval is guarded by a window flag. Both
/// guards are keyed off the cache-buster id, so bumping
/// [`xterm_host_path`] forces the new bundle to load.
pub fn bootstrap_js(cache_id: &str) -> String {
    let css = include_str!("../../assets/vendored/xterm/xterm.css");
    let js = include_str!("../../assets/vendored/xterm/xterm.js");
    let css_json = serde_json::to_string(css).unwrap_or_else(|_| "\"\"".into());
    let css_marker = format!("openkite-xterm-css-{cache_id}");
    format!(
        r#"if (!document.documentElement.classList.contains('{css_marker}')) {{
    document.documentElement.classList.add('{css_marker}');
    var s = document.createElement('style');
    s.textContent = {css_json};
    document.head.appendChild(s);
}}
if (!window.__openkite_xterm_loaded) {{
    window.__openkite_xterm_loaded = '{cache_id}';
    {js}
}}"#,
        css_marker = css_marker,
        css_json = css_json,
        cache_id = cache_id,
        js = js,
    )
}

/// The JS that mounts the xterm instance into the host div and forwards
/// keystrokes into the `window.__openkite_term_input` poll global.
///
/// The mount is async with respect to the bundle eval, so the snippet
/// retries up to 64 frames (16ms each) until `_term_mount` exists and the
/// host div is in the DOM. The `__openkite_term_id` stash keeps re-runs
/// from stacking a second Terminal.
pub fn mount_js(selector: &str) -> String {
    format!(
        r#"(function() {{
    var sel = {selector:?};
    function tryMount(retries) {{
        if (window.openkite && typeof window.openkite._term_mount === 'function') {{
            var root = document.querySelector(sel);
            if (root) {{
                if (!root.__openkite_term_id) {{
                    root.__openkite_term_id = window.openkite._term_mount(sel);
                    var t = root.__openkite_term_id;
                    if (t && t.onData) {{
                        t.onData(function(data) {{
                            window.__openkite_term_input = (window.__openkite_term_input || "") + data;
                        }});
                    }}
                }}
            }}
            return;
        }}
        if (retries <= 0) return;
        setTimeout(function() {{ tryMount(retries - 1); }}, 16);
    }}
    tryMount(64);
}})();"#,
        selector = selector,
    )
}

/// The JS that disposes the xterm instance and clears the host div.
pub fn reset_js(selector: &str) -> String {
    format!(
        r#"(function() {{
    var root = document.querySelector({selector:?});
    if (!root) return;
    var t = root.__openkite_term_id;
    if (t && typeof t.dispose === 'function') {{ t.dispose(); }}
    root.innerHTML = "";
    root.__openkite_term_id = null;
}})();"#,
        selector = selector,
    )
}

/// The JS that writes one line into the terminal (used for the
/// bridge-pending hint and error rendering).
pub fn writeln_js(selector: &str, text: &str) -> String {
    let text_json = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into());
    format!(
        "window.openkite._term_writeln({selector:?}, {text_json});",
        selector = selector,
        text_json = text_json,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xterm_host_path_is_stable() {
        assert_eq!(xterm_host_path(), "xterm-bundle-v1");
    }

    #[test]
    fn mount_js_uses_the_given_selector() {
        let js = mount_js("[data-term-host=\"xterm-bundle-v1-1\"]");
        assert!(js.contains("_term_mount"));
        assert!(js.contains("tryMount(64)"));
        assert!(js.contains("__openkite_term_input"));
        // The selector is interpolated as `var sel = "…"` (Rust `:?` debug
        // quoting escapes the inner quotes), then used via `querySelector(sel)`.
        assert!(js.contains(r#"var sel = "[data-term-host=\"xterm-bundle-v1-1\"]";"#));
        assert!(js.contains("document.querySelector(sel)"));
    }

    #[test]
    fn reset_js_disposes_and_clears() {
        let js = reset_js("[data-term-host=\"x\"]");
        assert!(js.contains("dispose"));
        assert!(js.contains("__openkite_term_id = null"));
        assert!(js.contains("[data-term-host=\\\"x\\\"]"));
    }

    #[test]
    fn writeln_js_escapes_the_text_payload() {
        let js = writeln_js("[data-term-host=\"x\"]", "exec bridge pending");
        assert!(js.contains("_term_writeln"));
        assert!(js.contains("\"exec bridge pending\""));
    }

    #[test]
    fn bootstrap_js_injects_bundle_once_guarded() {
        let js = bootstrap_js("xterm-bundle-v1");
        assert!(js.contains("openkite-xterm-css-xterm-bundle-v1"));
        assert!(js.contains("__openkite_xterm_loaded"));
        assert!(js.contains("window.__openkite_xterm_loaded = 'xterm-bundle-v1';"));
    }
}
