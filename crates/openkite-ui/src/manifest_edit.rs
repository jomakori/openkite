//! Pure model for the YAML editor: linting, per-line highlighting, the field-level diff
//! a dry run produces, and the apply/conflict state the panel renders.
//!
//! Nothing here talks to a cluster. [`ManifestEdit::dry_ran`] and
//! [`ManifestEdit::applied`] take the object the apiserver returned, and
//! [`ManifestEdit::conflicted`] takes the two versions a 409 carries, so every branch is
//! reachable from a test.

use serde_json::Value;

/// The field manager every apply this console makes is attributed to.
pub const FIELD_MANAGER: &str = "openkite";

/// Fields the apiserver owns. They never belong in an apply payload, so the diff skips
/// them instead of reporting churn the user cannot remove.
const SERVER_FIELDS: &[&str] = &[
    "status",
    "metadata.creationTimestamp",
    "metadata.generation",
    "metadata.managedFields",
    "metadata.resourceVersion",
    "metadata.uid",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warn,
}

impl Severity {
    /// Class the gutter, the issue row and the highlight line share.
    pub fn class(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warn => "warn",
        }
    }
}

/// One problem found before the object leaves the editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// 1-based line the problem sits on; 0 when the YAML has no location for it.
    pub line: usize,
    pub severity: Severity,
    pub message: String,
}

impl Diagnostic {
    pub fn error(line: usize, message: impl Into<String>) -> Self {
        Diagnostic {
            line,
            severity: Severity::Error,
            message: message.into(),
        }
    }

    pub fn warn(line: usize, message: impl Into<String>) -> Self {
        Diagnostic {
            line,
            severity: Severity::Warn,
            message: message.into(),
        }
    }
}

/// The parse result plus everything wrong with the document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lint {
    /// The parsed object, present only when the YAML itself parsed.
    pub doc: Option<Value>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Lint {
    pub fn errors(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|issue| issue.severity == Severity::Error)
            .count()
    }

    pub fn warnings(&self) -> usize {
        self.diagnostics.len() - self.errors()
    }

    /// Whether the document may be sent: it parsed and has no error.
    pub fn clean(&self) -> bool {
        self.doc.is_some() && self.errors() == 0
    }

    /// The worst severity present, for the status pill.
    pub fn worst(&self) -> Option<Severity> {
        if self.errors() > 0 {
            Some(Severity::Error)
        } else if self.warnings() > 0 {
            Some(Severity::Warn)
        } else {
            None
        }
    }
}

/// Parse and check a document. Warnings never block the apply; errors do.
pub fn lint(text: &str) -> Lint {
    if text.trim().is_empty() {
        return Lint {
            doc: None,
            diagnostics: vec![Diagnostic::error(0, "The document is empty.")],
        };
    }

    let doc: Value = match serde_yaml_ng::from_str(text) {
        Ok(doc) => doc,
        Err(error) => {
            // A scanner error can come back without a location; marking the first line
            // still puts the marker somewhere the reader will look.
            let line = error
                .location()
                .map(|location| location.line())
                .unwrap_or(1);
            return Lint {
                doc: None,
                diagnostics: vec![Diagnostic::error(line, error.to_string())],
            };
        }
    };

    let mut diagnostics = Vec::new();
    let Some(object) = doc.as_object() else {
        return Lint {
            doc: None,
            diagnostics: vec![Diagnostic::error(1, "The document must be a mapping.")],
        };
    };

    for field in ["apiVersion", "kind"] {
        if !is_filled(object.get(field)) {
            diagnostics.push(Diagnostic::error(1, format!("`{field}` is missing.")));
        }
    }

    let name = object
        .get("metadata")
        .and_then(|meta| meta.get("name"))
        .and_then(Value::as_str);
    if name.is_none_or(str::is_empty) {
        diagnostics.push(Diagnostic::error(1, "`metadata.name` is missing."));
    }

    if object.contains_key("status") {
        diagnostics.push(Diagnostic::warn(
            top_level_line(text, "status"),
            "`status` is server-managed and is dropped on apply.",
        ));
    }
    for field in ["resourceVersion", "uid"] {
        if object
            .get("metadata")
            .is_some_and(|meta| meta.get(field).is_some())
        {
            diagnostics.push(Diagnostic::warn(
                1,
                format!("`metadata.{field}` comes from the server."),
            ));
        }
    }

    for line in duplicate_keys(text) {
        diagnostics.push(Diagnostic::warn(
            line,
            "This key appears twice in the same mapping; the last one wins.",
        ));
    }

    Lint {
        doc: Some(doc),
        diagnostics,
    }
}

/// Lines whose key repeats inside the same mapping. The YAML parser keeps the last
/// value and says nothing, so the editor reports it instead.
fn duplicate_keys(text: &str) -> Vec<usize> {
    let mut scopes: Vec<(usize, Vec<String>)> = Vec::new();
    let mut repeats = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        let indent = line.len() - trimmed.len();
        let Some((key, rest)) = trimmed.split_once(':') else {
            continue;
        };
        if key.is_empty() || key.contains(' ') || rest.starts_with("//") {
            continue;
        }
        while scopes.last().is_some_and(|(width, _)| *width > indent) {
            scopes.pop();
        }
        match scopes.last_mut() {
            Some((width, keys)) if *width == indent => {
                if keys.iter().any(|seen| seen == key) {
                    repeats.push(index + 1);
                } else {
                    keys.push(key.to_string());
                }
            }
            _ => scopes.push((indent, vec![key.to_string()])),
        }
    }
    repeats
}

fn is_filled(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty())
}

/// Line a top-level key starts on, so its warning points somewhere useful.
fn top_level_line(text: &str, key: &str) -> usize {
    text.lines()
        .position(|line| line.trim_start().starts_with(&format!("{key}:")))
        .map(|index| index + 1)
        .unwrap_or(1)
}

/// One field that differs between the live object and the edited one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldChange {
    /// Dotted path, array entries as `name[0]`.
    pub path: String,
    /// Rendered live value; `None` when the field is being added.
    pub before: Option<String>,
    /// Rendered edited value; `None` when the field is being removed.
    pub after: Option<String>,
}

impl FieldChange {
    /// `added`, `removed` or `changed`, for the diff row's marker.
    pub fn kind(&self) -> &'static str {
        match (&self.before, &self.after) {
            (None, Some(_)) => "added",
            (Some(_), None) => "removed",
            _ => "changed",
        }
    }
}

/// Field-level diff between two objects, ordered by path.
pub fn diff(live: &Value, edited: &Value) -> Vec<FieldChange> {
    let mut changes = Vec::new();
    walk(live, edited, "", &mut changes);
    changes.sort_by(|left, right| left.path.cmp(&right.path));
    changes
}

fn walk(live: &Value, edited: &Value, path: &str, changes: &mut Vec<FieldChange>) {
    if SERVER_FIELDS.contains(&path) {
        return;
    }

    match (live, edited) {
        (Value::Object(live), Value::Object(edited)) => {
            for (key, value) in edited {
                let child = child_path(path, key);
                match live.get(key) {
                    Some(before) => walk(before, value, &child, changes),
                    None => push_added(&child, value, changes),
                }
            }
            for key in live.keys() {
                if !edited.contains_key(key) {
                    let child = child_path(path, key);
                    if !SERVER_FIELDS.contains(&child.as_str()) {
                        push_change(&child, Some(live[key].clone()), None, changes);
                    }
                }
            }
        }
        (Value::Array(live), Value::Array(edited)) => {
            for (index, value) in edited.iter().enumerate() {
                let child = format!("{path}[{index}]");
                match live.get(index) {
                    Some(before) => walk(before, value, &child, changes),
                    None => push_added(&child, value, changes),
                }
            }
            for (index, value) in live.iter().enumerate().skip(edited.len()) {
                push_change(
                    &format!("{path}[{index}]"),
                    Some(value.clone()),
                    None,
                    changes,
                );
            }
        }
        _ if live != edited => {
            push_change(path, Some(live.clone()), Some(edited.clone()), changes);
        }
        _ => {}
    }
}

fn child_path(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

fn push_added(path: &str, value: &Value, changes: &mut Vec<FieldChange>) {
    push_change(path, None, Some(value.clone()), changes);
}

fn push_change(
    path: &str,
    before: Option<Value>,
    after: Option<Value>,
    changes: &mut Vec<FieldChange>,
) {
    if SERVER_FIELDS.contains(&path) {
        return;
    }
    changes.push(FieldChange {
        path: path.to_owned(),
        before: before.as_ref().map(render),
        after: after.as_ref().map(render),
    });
}

/// Compact rendering of a value for the diff cells.
fn render(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "null".to_owned(),
        other => other.to_string(),
    }
}

/// Where the panel is in the dry-run/apply cycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// Typing; nothing has been sent.
    #[default]
    Editing,
    /// A dry run came back with the field-level diff.
    DryRun,
    /// The request is in flight.
    Pending,
    /// The apply succeeded.
    Applied,
    /// The apiserver refused with a 409 and both versions are held.
    Conflict,
}

/// Two versions of the object: what the console sent, and what the server holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Conflict {
    pub message: String,
    pub submitted: String,
    pub live: String,
}

/// The editor's state: text, lint, diff and the outcome of the last request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ManifestEdit {
    /// The manifest as fetched, kept so Reset and the diff have a baseline.
    pub live: String,
    /// The text in the editor.
    pub text: String,
    pub lint: Lint,
    pub phase: Phase,
    pub changes: Vec<FieldChange>,
    pub conflict: Option<Conflict>,
    /// Human-readable result of the last request, shown in the status pill.
    pub notice: Option<String>,
}

impl ManifestEdit {
    /// Open the editor on a manifest.
    pub fn start(live: &str) -> Self {
        ManifestEdit {
            live: live.to_owned(),
            text: live.to_owned(),
            lint: lint(live),
            ..ManifestEdit::default()
        }
    }

    /// Replace the editor text and re-lint. Any previous dry run is stale now.
    pub fn edit(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.lint = lint(&self.text);
        self.phase = Phase::Editing;
        self.changes.clear();
        self.conflict = None;
        self.notice = None;
    }

    /// Go back to the fetched manifest.
    pub fn reset(&mut self) {
        self.edit(self.live.clone());
        self.phase = Phase::Editing;
    }

    /// Whether the text differs from the fetched manifest.
    pub fn dirty(&self) -> bool {
        self.text != self.live
    }

    /// A dry run needs a clean parse and something to send.
    pub fn can_dry_run(&self) -> bool {
        self.lint.clean() && self.dirty()
    }

    /// Apply needs a clean parse and a dry run of the current text, so the diff is
    /// never skipped.
    pub fn can_apply(&self) -> bool {
        self.can_dry_run() && matches!(self.phase, Phase::DryRun)
    }

    /// Mark the request in flight.
    pub fn pending(&mut self) {
        self.phase = Phase::Pending;
        self.notice = Some("Sending…".to_owned());
    }

    /// Record the object a dry run returned and the diff against the live one.
    pub fn dry_ran(&mut self, live: &Value, returned: &Value) {
        self.changes = diff(live, returned);
        self.phase = Phase::DryRun;
        self.notice = Some(self.dry_run_notice());
        self.conflict = None;
    }

    /// Record a successful apply.
    pub fn applied(&mut self, returned: &Value, live: &Value) {
        self.changes = diff(live, returned);
        self.phase = Phase::Applied;
        self.notice = Some(if self.changes.is_empty() {
            "Applied. No fields changed.".to_owned()
        } else {
            format!("Applied {} field change(s).", self.changes.len())
        });
        self.conflict = None;
    }

    /// Record a 409: keep both versions so the panel can show them side by side.
    pub fn conflicted(&mut self, message: impl Into<String>, submitted: &str, live: &str) {
        self.phase = Phase::Conflict;
        self.notice = Some("Conflict. The object changed on the server.".to_owned());
        self.conflict = Some(Conflict {
            message: message.into(),
            submitted: submitted.to_owned(),
            live: live.to_owned(),
        });
    }

    /// Record a failure that is not a conflict.
    pub fn failed(&mut self, message: impl Into<String>) {
        self.phase = Phase::Editing;
        self.notice = Some(message.into());
    }

    fn dry_run_notice(&self) -> String {
        if self.changes.is_empty() {
            "Dry run: no changes.".to_owned()
        } else {
            format!("Dry run: {} field change(s).", self.changes.len())
        }
    }

    /// Status pill: `(class, text)`. `ok` and `warn` carry the two clean outcomes,
    /// `bad` every refusal.
    pub fn status(&self) -> (&'static str, String) {
        // The status line summarises; the issue list below carries the parser's own
        // wording, so it is not repeated here.
        if matches!(self.phase, Phase::Conflict) {
            return ("bad", self.problem_text());
        }
        if let Some(issue) = self
            .lint
            .diagnostics
            .iter()
            .find(|issue| issue.severity == Severity::Error)
        {
            return ("bad", format!("Parse failed at line {}.", issue.line));
        }
        if self.lint.warnings() > 0 && self.phase == Phase::Editing {
            let count = self.lint.warnings();
            return ("warn", format!("{count} warning(s) — see the list below."));
        }
        match &self.notice {
            Some(notice) => ("ok", notice.clone()),
            // Nothing has been sent yet; that is idle, not a warning.
            None => ("idle", "Not sent yet.".to_owned()),
        }
    }

    fn problem_text(&self) -> String {
        if let Some(conflict) = &self.conflict {
            return conflict.message.clone();
        }
        match self.lint.diagnostics.first() {
            Some(issue) => issue.message.clone(),
            None => "Not sent yet.".to_owned(),
        }
    }

    /// Per-line severity for the gutter and the highlight overlay, 1-based by index.
    pub fn marks(&self) -> Vec<Option<Severity>> {
        let mut marks = vec![None; self.text.lines().count().max(1)];
        for issue in &self.lint.diagnostics {
            if issue.line == 0 {
                continue;
            }
            // A parser location can point past the final line, so clamp to the last one
            // instead of dropping the marker.
            let index = (issue.line - 1).min(marks.len() - 1);
            let slot = &mut marks[index];
            let worse = match (*slot, issue.severity) {
                (Some(Severity::Error), _) => Some(Severity::Error),
                _ => Some(issue.severity),
            };
            *slot = worse;
        }
        marks
    }
}

/// One coloured run of text inside a highlighted line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    /// One of `y-k`, `y-s`, `y-q`, `y-n`, `y-b`, `y-z`, `y-a`, `y-c`, `y-d`, or empty.
    pub class: &'static str,
    pub text: String,
}

/// Highlight one document, line by line, for the overlay behind the textarea.
pub fn highlight(text: &str) -> Vec<Vec<Segment>> {
    text.lines().map(highlight_line).collect()
}

fn highlight_line(line: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let indent = line.len() - line.trim_start().len();
    if indent > 0 {
        segments.push(segment("", &line[..indent]));
    }
    let body = &line[indent..];

    if body.is_empty() {
        return segments;
    }
    if body.starts_with('#') {
        segments.push(segment("y-c", body));
        return segments;
    }
    if body == "---" || body == "..." {
        segments.push(segment("y-d", body));
        return segments;
    }

    let mut rest = body;
    if let Some(item) = rest.strip_prefix("- ") {
        segments.push(segment("y-d", "- "));
        rest = item;
    }

    match split_key(rest) {
        Some((key, after_colon)) => {
            let key_class = if key.starts_with('"') || key.starts_with('\'') {
                "y-q"
            } else {
                "y-k"
            };
            segments.push(segment(key_class, key));
            segments.push(segment("y-d", ":"));
            push_value(&mut segments, after_colon);
        }
        None => push_value(&mut segments, rest),
    }
    segments
}

/// Split `key: value` into the key and everything after the colon, including its spacing.
fn split_key(text: &str) -> Option<(&str, &str)> {
    let mut in_single = false;
    let mut in_double = false;
    for (index, ch) in text.char_indices() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            ':' if !in_single && !in_double => {
                let after = &text[index + 1..];
                if after.is_empty() || after.starts_with(' ') {
                    return Some((&text[..index], after));
                }
            }
            _ => {}
        }
    }
    None
}

/// Push a value run: a comment tail, a quoted string, a scalar, or spaced-out plain text.
fn push_value(segments: &mut Vec<Segment>, text: &str) {
    if text.is_empty() {
        return;
    }
    let (value, comment) = split_comment(text);
    if value.is_empty() {
        segments.push(segment("", value));
    } else if value.trim_start().starts_with('"') || value.trim_start().starts_with('\'') {
        segments.push(segment("y-q", value));
    } else if let Some(rest) = value
        .trim_start()
        .strip_prefix('&')
        .or_else(|| value.trim_start().strip_prefix('*'))
    {
        // The anchor sigil and its name carry the class; the pad and any tail stay plain.
        let trimmed = value.trim_start();
        let lead = value.len() - trimmed.len();
        let name_end = rest.find(' ').map_or(rest.len(), |index| index + 1);
        let anchor_end = lead + name_end;
        segments.push(segment("y-a", &value[..anchor_end]));
        if anchor_end < value.len() {
            segments.push(segment("", &value[anchor_end..]));
        }
    } else if matches!(value.trim(), "true" | "false" | "yes" | "no" | "on" | "off") {
        segments.push(segment("y-b", value));
    } else if matches!(value.trim(), "null" | "~") {
        segments.push(segment("y-z", value));
    } else if value.trim().parse::<f64>().is_ok() {
        segments.push(segment("y-n", value));
    } else if matches!(value.trim(), "|" | ">" | "|-" | ">-" | "|+" | ">+") {
        segments.push(segment("y-d", value));
    } else {
        segments.push(segment("", value));
    }
    if !comment.is_empty() {
        segments.push(segment("y-c", comment));
    }
}

/// Split a trailing ` # comment` off a value. A `#` with no space before it stays in the value.
fn split_comment(text: &str) -> (&str, &str) {
    let mut in_single = false;
    let mut in_double = false;
    for (index, ch) in text.char_indices() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '#' if !in_single && !in_double && index > 0 && text[..index].ends_with(' ') => {
                return (&text[..index], &text[index..]);
            }
            _ => {}
        }
    }
    (text, "")
}

fn segment(class: &'static str, text: &str) -> Segment {
    Segment {
        class,
        text: text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MANIFEST: &str =
        "apiVersion: v1\nkind: Pod\nmetadata:\n  name: api-7f9c\nspec:\n  replicas: 3\n";

    #[test]
    fn a_valid_manifest_lints_clean() {
        let result = lint(MANIFEST);
        assert!(result.clean(), "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.worst(), None);
    }

    #[test]
    fn a_broken_document_reports_the_line() {
        let result = lint("apiVersion: v1\nkind: Pod\n  name: stray\n");
        assert!(!result.clean());
        assert!(result.doc.is_none());
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert!(result.diagnostics[0].line > 0);
    }

    #[test]
    fn a_missing_kind_is_an_error() {
        let result = lint("apiVersion: v1\nmetadata:\n  name: api\n");
        assert!(result
            .diagnostics
            .iter()
            .any(|issue| issue.message.contains("`kind` is missing")));
        assert!(!result.clean());
    }

    #[test]
    fn a_missing_name_is_an_error() {
        let result = lint("apiVersion: v1\nkind: Pod\nmetadata: {}\n");
        assert!(result
            .diagnostics
            .iter()
            .any(|issue| issue.message.contains("metadata.name")));
    }

    #[test]
    fn status_is_a_warning_not_an_error() {
        let result = lint(&format!("{MANIFEST}status:\n  phase: Running\n"));
        assert!(result.clean());
        assert_eq!(result.warnings(), 1);
        assert_eq!(result.worst(), Some(Severity::Warn));
        assert!(result.diagnostics[0].line >= 7);
    }

    #[test]
    fn a_multi_document_file_is_refused() {
        let result = lint(&format!("{MANIFEST}---\n{MANIFEST}"));
        assert!(!result.clean());
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn an_empty_document_is_refused() {
        let result = lint("   \n");
        assert!(!result.clean());
        assert_eq!(result.diagnostics[0].message, "The document is empty.");
    }

    #[test]
    fn the_diff_reports_one_field_per_change() {
        let live = json!({"apiVersion":"v1","kind":"Pod","metadata":{"name":"api"},
            "spec":{"replicas":3,"image":"v1"}});
        let edited = json!({"apiVersion":"v1","kind":"Pod","metadata":{"name":"api"},
            "spec":{"replicas":5,"image":"v1"}});

        let changes = diff(&live, &edited);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "spec.replicas");
        assert_eq!(changes[0].before.as_deref(), Some("3"));
        assert_eq!(changes[0].after.as_deref(), Some("5"));
        assert_eq!(changes[0].kind(), "changed");
    }

    #[test]
    fn the_diff_marks_added_and_removed_fields() {
        let live = json!({"spec":{"image":"v1","replicas":3}});
        let edited = json!({"spec":{"image":"v1","paused":true}});

        let changes = diff(&live, &edited);
        let paths: Vec<&str> = changes.iter().map(|change| change.path.as_str()).collect();
        assert_eq!(paths, ["spec.paused", "spec.replicas"]);
        assert_eq!(changes[0].kind(), "added");
        assert_eq!(changes[1].kind(), "removed");
    }

    #[test]
    fn the_diff_walks_arrays_by_index() {
        let live = json!({"spec":{"containers":[{"image":"v1"},{"image":"v2"}]}});
        let edited = json!({"spec":{"containers":[{"image":"v1"},{"image":"v9"}]}});

        let changes = diff(&live, &edited);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "spec.containers[1].image");
    }

    #[test]
    fn the_diff_skips_server_owned_fields() {
        let live = json!({"metadata":{"name":"api","resourceVersion":"7"},
            "status":{"phase":"Running"},"spec":{"replicas":3}});
        let edited = json!({"metadata":{"name":"api","resourceVersion":"9"},
            "spec":{"replicas":5}});

        let changes = diff(&live, &edited);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "spec.replicas");
    }

    #[test]
    fn a_dry_run_carries_the_diff_and_a_notice() {
        let mut edit = ManifestEdit::start(MANIFEST);
        edit.edit(MANIFEST.replace("replicas: 3", "replicas: 5"));
        assert!(edit.can_dry_run());

        let live = lint(MANIFEST).doc.expect("live parses");
        let returned = lint(&edit.text).doc.expect("edited parses");
        edit.dry_ran(&live, &returned);

        assert_eq!(edit.phase, Phase::DryRun);
        assert_eq!(edit.changes.len(), 1);
        assert_eq!(edit.status().0, "ok");
        assert!(edit.status().1.contains("1 field change"));
    }

    #[test]
    fn an_unchanged_manifest_cannot_be_sent() {
        let edit = ManifestEdit::start(MANIFEST);
        assert!(!edit.dirty());
        assert!(!edit.can_dry_run());
        assert!(!edit.can_apply());
    }

    #[test]
    fn a_conflict_keeps_both_versions() {
        let mut edit = ManifestEdit::start(MANIFEST);
        edit.edit(MANIFEST.replace("replicas: 3", "replicas: 5"));
        edit.conflicted(
            "Apply failed: the object was modified by another manager.",
            &edit.text.clone(),
            MANIFEST,
        );

        assert_eq!(edit.phase, Phase::Conflict);
        assert_eq!(edit.status().0, "bad");
        let conflict = edit.conflict.expect("conflict is held");
        assert!(conflict.submitted.contains("replicas: 5"));
        assert!(conflict.live.contains("replicas: 3"));
        assert!(conflict.message.contains("another manager"));
    }

    #[test]
    fn reset_restores_the_fetched_manifest() {
        let mut edit = ManifestEdit::start(MANIFEST);
        edit.edit(MANIFEST.replace("replicas: 3", "replicas: 5"));
        edit.reset();
        assert_eq!(edit.text, MANIFEST);
        assert!(!edit.dirty());
        assert_eq!(edit.phase, Phase::Editing);
    }

    #[test]
    fn an_apply_reports_the_change_count() {
        let mut edit = ManifestEdit::start(MANIFEST);
        let live = lint(MANIFEST).doc.expect("live parses");
        let returned = json!({"spec":{"replicas":5}});
        let mut live_only_spec = live.clone();
        live_only_spec["spec"] = returned["spec"].clone();
        edit.applied(&live_only_spec, &live);

        assert_eq!(edit.phase, Phase::Applied);
        assert_eq!(edit.status().0, "ok");
        assert!(edit.status().1.contains("1 field change"));
    }

    #[test]
    fn the_gutter_marks_the_line_a_diagnostic_points_at() {
        let edit =
            ManifestEdit::start("apiVersion: v1\nkind: Pod\nmetadata:\n  name: api\nwrong: [\n");
        let marks = edit.marks();
        assert!(marks.iter().any(Option::is_some));
        assert!(marks.iter().flatten().all(|mark| *mark == Severity::Error));
        assert_eq!(marks.len(), edit.text.lines().count());
    }

    #[test]
    fn a_key_and_a_value_highlight_separately() {
        let lines = highlight("  name: api-7f9c\n");
        let classes: Vec<&str> = lines[0].iter().map(|segment| segment.class).collect();
        assert_eq!(classes, ["", "y-k", "y-d", ""]);
        assert_eq!(lines[0][1].text, "name");
    }

    #[test]
    fn a_quoted_value_and_a_trailing_comment_highlight() {
        let lines = highlight("image: \"v1.2\"  # pinned\n");
        let classes: Vec<&str> = lines[0].iter().map(|segment| segment.class).collect();
        assert_eq!(classes, ["y-k", "y-d", "y-q", "y-c"]);
        assert_eq!(lines[0][2].text.trim(), "\"v1.2\"");
        assert_eq!(lines[0][3].text, "# pinned");
    }

    #[test]
    fn numbers_booleans_and_null_highlight() {
        let lines = highlight("replicas: 3\npaused: true\nvalue: null\nanchor: &base\n");
        assert_eq!(lines[0][2].class, "y-n");
        assert_eq!(lines[1][2].class, "y-b");
        assert_eq!(lines[2][2].class, "y-z");
        assert_eq!(lines[3][2].class, "y-a");
    }

    #[test]
    fn a_list_item_keeps_its_dash_and_highlight() {
        let lines = highlight("  - name: sidecar\n");
        let classes: Vec<&str> = lines[0].iter().map(|segment| segment.class).collect();
        assert_eq!(classes, ["", "y-d", "y-k", "y-d", ""]);
    }

    #[test]
    fn a_comment_line_highlights_whole() {
        let lines = highlight("# the entrypoint\n");
        assert_eq!(lines[0].len(), 1);
        assert_eq!(lines[0][0].class, "y-c");
    }

    #[test]
    fn a_url_keeps_its_colon_in_the_value() {
        let lines = highlight("image: docker.io/library/nginx:1.27\n");
        let classes: Vec<&str> = lines[0].iter().map(|segment| segment.class).collect();
        assert_eq!(classes, ["y-k", "y-d", ""]);
        assert_eq!(lines[0][2].text.trim(), "docker.io/library/nginx:1.27");
    }

    #[test]
    fn the_segments_rebuild_every_source_line_exactly() {
        // The highlight overlay sits behind the textarea, so its text has to match the
        // buffer character for character or the lines drift.
        let text = "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: api-7f9c\n  annotations:\n    url: https://example.test:8443/x  # keep\n    anchor:  &base   \n    on: true\n";
        let lines = highlight(text);
        assert_eq!(lines.len(), text.lines().count());
        for (segments, line) in lines.iter().zip(text.lines()) {
            let rebuilt: String = segments
                .iter()
                .map(|segment| segment.text.as_str())
                .collect();
            assert_eq!(rebuilt, line);
        }
    }
}
