//! CodeMirror 6 surface for the manifest editor and the YAML diagnostics
//! block. Mounts a vendored CodeMirror 6 IIFE bundle (see
//! `assets/vendored/codemirror/`) into a `.code-editor` host element and
//! pushes parse diagnostics from the P1 `crate::yaml::parse_yaml` surface
//! into a `.diagnostics` block below the editor.
//!
//! The component is split per the openkite-dev skill: the pure-logic
//! helpers (`compute_diagnostics`, `code_editor_path`) live at the top of
//! the file with no `dioxus` import so they unit-test without a runtime;
//! the `#[component]` bodies at the bottom pull in the dioxus glob.

use dioxus::prelude::*;

use crate::yaml::{parse_yaml, Diagnostic};

/// Parse `text` and return the syntax errors as a `Vec<Diagnostic>`.
///
/// `parse_yaml` reports the first error (or `Ok` on a clean document);
/// the `Vec` shape is forward-compatible with a future multi-error parser
/// and matches what the host needs to render the diagnostics block.
pub fn compute_diagnostics(text: &str) -> Vec<Diagnostic> {
    match parse_yaml(text) {
        Ok(_) => Vec::new(),
        Err(diag) => vec![diag],
    }
}

/// Cache-buster id for the vendored CodeMirror bundle. Bump the `vN`
/// suffix whenever the bundle in `assets/vendored/codemirror/` is
/// rebuilt (the `SOURCE.txt` in that directory records the rebuild
/// command). The bootstrap effect reads this id to guard the
/// `window.__openkite_cm_loaded` flag and the `<html>` class.
pub fn code_editor_path() -> &'static str {
    "cm-bundle-v1"
}

/// Precompute the `(line, column, message)` rows for the rsx! for-loop
/// (skill: precompute `Vec<T>` outside `rsx!`; the macro cannot parse a
/// bare expression as an element body).
fn diagnostics_rows(diagnostics: &[Diagnostic]) -> Vec<(u64, u64, String)> {
    diagnostics
        .iter()
        .map(|d| (d.line, d.column, d.message.clone()))
        .collect()
}

/// Host a CodeMirror 6 instance with the design-system surface.
///
/// The host contract is the read-only / writable mount; `on_change` is
/// an `Option` so the read-only `YamlTab` consumer does not have to
/// provide a handler. Diagnostics are pre-computed (the parent owns the
/// parse + debounce for the future OKT-43 editable mode); this
/// component just renders the `Vec<Diagnostic>` it is given.
///
/// Props:
/// - `text`: the YAML to display / edit.
/// - `read_only`: when `true`, the CodeMirror `EditorState` is created
///   in read-only mode and the host never fires `on_change`.
/// - `on_change`: optional edit callback. Today the read-only
///   `YamlTab` passes `None`; OKT-43 will be the first consumer that
///   wires this to a kube `Api::replace` call.
/// - `diagnostics`: the latest parse errors. The component renders a
///   `.diagnostics` block below the editor with one row per error
///   (`{line}:{column} — {message}`).
#[component]
pub fn CodeEditor(
    text: String,
    #[props(default)] read_only: bool,
    #[props(default)] on_change: Option<EventHandler<String>>,
    #[props(default)] diagnostics: Vec<Diagnostic>,
) -> Element {
    // Stable per-mount instance id. `use_hook` returns state by value
    // (skill: avoids the !Send RefCell trap of use_hook's return-type
    // contract). The id is fixed for the lifetime of this component
    // instance, so the data-cm-host attribute is stable across renders
    // and the JS mount effect's `document.querySelector` finds the
    // same div on every prop change.
    let instance_id = use_hook(|| {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    });
    let cache_id = code_editor_path();
    let data_attr = format!("{cache_id}-{instance_id}");
    // The mount effect below captures `data_attr` by move, but the
    // `rsx!` after the effect also references it. Clone for the
    // closure so the rsx! can keep the original (the effect runs on
    // text/read_only prop changes, so the closure-captured value is
    // the only one that mutates; the rsx! uses the outer one).
    let data_attr_for_effect = data_attr.clone();
    let text_json = serde_json::to_string(&text).unwrap_or_else(|_| "\"\"".into());
    let read_only_js = if read_only { "true" } else { "false" };
    // Precompute the first diagnostic as a `Copy` Option<&Diagnostic>
    // so the bootstrap effect can capture it by move without taking
    // ownership of the whole `Vec<Diagnostic>` prop (which is still
    // borrowed by `diagnostics_rows(&diagnostics)` after the effect).
    let first_diag: Option<&Diagnostic> = diagnostics.first();

    // Bootstrap effect: inject the vendored CSS once (guarded by an
    // <html> class), inject the vendored JS once (guarded by
    // `window.__openkite_cm_loaded`), then push the latest parse
    // diagnostic so the lint marker is rendered without a per-keystroke
    // re-parse on the JS side. Mount-once: not subscribed to `text` or
    // `read_only`; the mount effect below handles per-prop updates.
    use_effect(move || {
        let css = include_str!("../../assets/vendored/codemirror/editor.css");
        let js = include_str!("../../assets/vendored/codemirror/editor.js");
        let css_json = serde_json::to_string(css).unwrap_or_else(|_| "\"\"".into());
        let css_marker = format!("openkite-cm-css-{cache_id}");
        let inject_css = format!(
            r#"if (!document.documentElement.classList.contains('{css_marker}')) {{
                document.documentElement.classList.add('{css_marker}');
                var s = document.createElement('style');
                s.textContent = {css_json};
                document.head.appendChild(s);
            }}"#,
        );
        let _ = document::eval(&inject_css);
        let inject_js = format!(
            r#"if (!window.__openkite_cm_loaded) {{
                window.__openkite_cm_loaded = '{cache_id}';
                {js}
            }}"#,
        );
        let _ = document::eval(&inject_js);
        let diag = first_diag;
        let diag_json = match diag {
            Some(d) => serde_json::to_string(&serde_json::json!({
                "message": d.message,
                "line": d.line,
                "column": d.column,
            }))
            .unwrap_or_else(|_| "null".into()),
            None => "null".to_string(),
        };
        let _ = document::eval(&format!("window.__openkite_yaml_diag = {diag_json};"));
    });

    // Mount effect: re-runs on (text, read_only) prop changes. Calls the
    // bundle's `_cm_mount` on the first run for this host div, then
    // `_cm_set_text` on subsequent runs to keep the document in sync
    // without recreating the EditorState.
    use_effect(move || {
        // Suppress the unused-warning on `on_change`: the prop is part
        // of the public surface; consumers wire it in OKT-43.
        let _ = on_change;
        let selector = format!("[data-cm-host=\"{data_attr_for_effect}\"]");
        let mount = format!(
            r#"(function() {{
                var sel = {selector:?};
                function tryMount(retries) {{
                    if (window.openkite && typeof window.openkite._cm_mount === 'function') {{
                        var root = document.querySelector(sel);
                        if (root) {{
                            if (!root.__openkite_cm_id) {{
                                root.__openkite_cm_id = window.openkite._cm_mount(sel, {text_json}, {read_only_js});
                            }} else {{
                                window.openkite._cm_set_text(root.__openkite_cm_id, {text_json});
                            }}
                        }}
                        return;
                    }}
                    if (retries <= 0) return;
                    setTimeout(function() {{ tryMount(retries - 1); }}, 16);
                }}
                tryMount(64);
            }})();"#
        );
        let _ = document::eval(&mount);
    });

    // The host div carries `data-cm-host` so the mount effect's
    // `document.querySelector` finds it. The `.code-editor` class
    // supplies the design-system surface (border, padding, surface).
    let rows = diagnostics_rows(&diagnostics);
    rsx! {
        div { class: "code-editor", "data-cm-host": "{data_attr}",
            YamlDiagnostics { diagnostics: rows }
        }
    }
}

/// Render the parse-error block. One row per diagnostic, in the
/// `.diagnostics` / `.diagnostic-line` design-system surface. The
/// component takes the **owned** `(line, column, message)` rows so the
/// rsx! for-loop iterates by value (skill: precompute outside rsx!).
#[component]
pub fn YamlDiagnostics(diagnostics: Vec<(u64, u64, String)>) -> Element {
    rsx! {
        div { class: "diagnostics",
            for (line, column, message) in diagnostics.iter() {
                div { class: "diagnostic-line",
                    span { class: "line-col", "{line}:{column}" }
                    span { class: "message", " — {message}" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_diagnostics_returns_empty_for_valid_yaml() {
        let text = "apiVersion: v1\nkind: Pod\n";
        assert!(compute_diagnostics(text).is_empty());
    }

    #[test]
    fn compute_diagnostics_returns_one_for_bad_yaml() {
        let text = "apiVersion: v1\nitems: [1, 2, 3\n";
        let diags = compute_diagnostics(text);
        assert_eq!(diags.len(), 1);
        assert!(!diags[0].message.is_empty());
        assert!(
            diags[0].line >= 1,
            "line should be reported, got {}",
            diags[0].line
        );
    }

    #[test]
    fn compute_diagnostics_returns_empty_for_empty_text() {
        assert!(compute_diagnostics("").is_empty());
    }

    #[test]
    fn code_editor_path_returns_stable_id() {
        assert_eq!(code_editor_path(), "cm-bundle-v1");
    }
}
