//! The YAML editor panel: gutter, highlighted overlay behind a textarea, the issue list,
//! the dry-run diff and the conflict views.
//!
//! Every colour, spacing and status rule comes from the console prototype's `yaml-ed`,
//! `yaml-gutter`, `yaml-hl`, `yaml-status`, `yaml-issues` and `yaml-btns` classes; the
//! diff table is the only new block (`.yaml-diff`).

use dioxus::prelude::*;

/// Keeps the highlight overlay and the gutter pinned to the textarea's scroll offset.
const SCROLL_SYNC: &str = r"(function () {
  const textarea = document.querySelector('.yaml-edit');
  if (!textarea || textarea.dataset.synced) return;
  textarea.dataset.synced = '1';
  const overlay = textarea.parentElement.querySelector('.yaml-hl');
  const gutter = document.querySelector('.yaml-ed .yaml-gutter');
  const follow = function () {
    if (overlay) {
      overlay.style.marginLeft = -textarea.scrollLeft + 'px';
      overlay.style.marginTop = -textarea.scrollTop + 'px';
    }
    if (gutter) gutter.style.marginTop = -textarea.scrollTop + 'px';
  };
  textarea.addEventListener('scroll', follow);
  follow();
})();";

use crate::manifest_edit::{highlight, FieldChange, ManifestEdit, Phase, Severity};

#[derive(Props, Clone, PartialEq)]
pub struct ManifestEditorProps {
    /// Object identity for the header, e.g. `Pod / api-7f9c`.
    pub title: String,
    pub edit: ManifestEdit,
    pub on_input: EventHandler<String>,
    pub on_dry_run: EventHandler<()>,
    pub on_apply: EventHandler<()>,
    pub on_reset: EventHandler<()>,
    pub on_close: EventHandler<()>,
}

fn icon(name: &'static str) -> Element {
    let href = format!("#i-{name}");
    rsx! {
        svg { class: "icon", use { href: "{href}" } }
    }
}

/// Line numbers down the left edge, tinted on the lines a diagnostic names.
fn gutter(text: &str, marks: &[Option<Severity>]) -> Element {
    let lines = text.lines().count().max(1);
    rsx! {
        div { class: "yaml-gutter", aria_hidden: "true",
            for line in 1..=lines {
                {
                    let mark = marks.get(line - 1).copied().flatten();
                    let class = match mark {
                        Some(severity) => format!("gl {}", severity.class()),
                        None => "gl".to_owned(),
                    };
                    rsx! { span { class: "{class}", "{line}" } }
                }
            }
        }
    }
}

/// The highlighted copy the transparent textarea sits on.
fn highlighted(text: &str, marks: &[Option<Severity>]) -> Element {
    rsx! {
        div { class: "yaml-hl", aria_hidden: "true",
            for (index, segments) in highlight(text).into_iter().enumerate() {
                {
                    let mark = marks.get(index).copied().flatten();
                    let class = match mark {
                        Some(severity) => format!("yl {}", severity.class()),
                        None => "yl".to_owned(),
                    };
                    rsx! {
                        span { class: "{class}",
                            for segment in segments {
                                span { class: "{segment.class}", "{segment.text}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The issue list under the editor; clicking a row is left to the shell.
fn issues(edit: &ManifestEdit) -> Element {
    if edit.lint.diagnostics.is_empty() {
        return rsx! {};
    }
    rsx! {
        ul { class: "yaml-issues",
            for issue in edit.lint.diagnostics.iter() {
                li { class: "{issue.severity.class()}",
                    span { class: "yl-ln", "L{issue.line}" }
                    span { "{issue.message}" }
                }
            }
        }
    }
}

/// One diff row: marker, path, live value, edited value.
fn diff_row(change: &FieldChange) -> Element {
    let before = change.before.clone().unwrap_or_else(|| "—".to_owned());
    let after = change.after.clone().unwrap_or_else(|| "—".to_owned());
    rsx! {
        div { class: "diff-row {change.kind()}",
            span { class: "diff-mark", "{mark(change)}" }
            span { class: "diff-path mono", "{change.path}" }
            span { class: "diff-before mono", "{before}" }
            span { class: "diff-after mono", "{after}" }
        }
    }
}

fn mark(change: &FieldChange) -> &'static str {
    match change.kind() {
        "added" => "+",
        "removed" => "−",
        _ => "~",
    }
}

fn diff_table(changes: &[FieldChange]) -> Element {
    rsx! {
        div { class: "yaml-diff", role: "table", aria_label: "Field-level diff",
            div { class: "diff-head", role: "row",
                span { class: "diff-mark", "" }
                span { class: "diff-path", "Field" }
                span { class: "diff-before", "Live" }
                span { class: "diff-after", "Edited" }
            }
            for change in changes {
                {diff_row(change)}
            }
        }
    }
}

/// The two versions a 409 returns, newest response first.
fn conflict_view(edit: &ManifestEdit) -> Element {
    let Some(conflict) = edit.conflict.as_ref() else {
        return rsx! {};
    };
    rsx! {
        div { class: "yaml-conflict",
            div { class: "conflict-pair",
                div { class: "conflict-side",
                    h4 { "Yours" }
                    div { class: "conflict-body",
                        {gutter(&conflict.submitted, &[])}
                        pre { class: "yaml", "data-yaml": "submitted", "{conflict.submitted}" }
                    }
                }
                div { class: "conflict-side",
                    h4 { "On the server" }
                    div { class: "conflict-body",
                        {gutter(&conflict.live, &[])}
                        pre { class: "yaml", "data-yaml": "live", "{conflict.live}" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn ManifestEditor(props: ManifestEditorProps) -> Element {
    let edit = props.edit.clone();
    let (status_class, status_text) = edit.status();
    let marks = edit.marks();
    let dirty = edit.dirty();
    let can_dry_run = edit.can_dry_run();
    let can_apply = edit.can_apply();
    let pending = edit.phase == Phase::Pending;
    let applied = edit.phase == Phase::Applied;
    let changes = edit.changes.clone();

    rsx! {
        section {
            class: if edit.conflict.is_some() { "yaml-editor conflict" } else { "yaml-editor" },
            "data-surface": "yaml-editor",
            header { class: "inspector-header",
                h2 { class: "insp-title", "Edit manifest" }
                span { class: "mono muted", "{props.title}" }
                button {
                    r#type: "button",
                    class: "icon-btn",
                    aria_label: "Close editor",
                    onclick: move |_| props.on_close.call(()),
                    {icon("close")}
                }
            }
            div { class: "yaml-bar",
                span { class: "mono muted-note", "fieldManager=openkite" }
                div { class: "yaml-btns",
                    button {
                        r#type: "button",
                        class: "btn",
                        disabled: !can_dry_run || pending,
                        onclick: move |_| props.on_dry_run.call(()),
                        "Dry run"
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-primary",
                        id: "yamlApply",
                        disabled: !can_apply || pending,
                        onclick: move |_| props.on_apply.call(()),
                        if applied { "Applied" } else { "Apply" }
                    }
                    button {
                        r#type: "button",
                        class: "btn ghost",
                        disabled: !dirty || pending,
                        onclick: move |_| props.on_reset.call(()),
                        "Reset"
                    }
                }
            }
            div { class: "yaml-ed",
                {gutter(&edit.text, &marks)}
                div { class: "yaml-stack",
                    {highlighted(&edit.text, &marks)}
                    textarea {
                        class: "yaml-edit",
                        "data-editor": "manifest",
                        spellcheck: "false",
                        aria_label: "Manifest YAML",
                        value: "{edit.text}",
                        oninput: move |event| props.on_input.call(event.value()),
                        // The highlight layer sits behind a transparent textarea, so it has
                        // to follow the textarea's scroll or the colours slide off the text.
                        onmounted: move |_| {
                            spawn(async move {
                                let _ = dioxus::document::eval(SCROLL_SYNC).await;
                            });
                        },
                    }
                }
            }
            div { class: "yaml-status {status_class}", role: "status", "{status_text}" }
            {issues(&edit)}
            if !changes.is_empty() {
                {diff_table(&changes)}
            }
            if edit.phase == Phase::Conflict {
                {conflict_view(&edit)}
            }
        }
    }
}
