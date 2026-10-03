//! Owned pod contract: the model the shared UI renders and the pure helpers
//! the inspector tabs run against it. The host fills it from a kube `Pod`
//! through `openkite_host::gateway::pod_object`; no `k8s-openapi` type ever
//! reaches into `openkite-ui`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A single container the inspector's Containers tab renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub name: String,
    pub image: String,
    pub ready: bool,
    pub restarts: i32,
    pub state: String,
}

/// The Overview tab's high-level status row set.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PodSummary {
    pub phase: String,
    pub node: String,
    pub pod_ip: String,
    pub qos: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}

/// The shared pod contract: name/namespace, summary fields, container rows,
/// labels/annotations, and a pre-rendered YAML blob for the YAML tab. The host
/// computes `yaml` (it owns the YAML serializer).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodObject {
    pub name: String,
    pub namespace: Option<String>,
    pub summary: PodSummary,
    pub containers: Vec<ContainerInfo>,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub yaml: String,
}

impl PodObject {
    /// Container names from the spec, in declaration order. Empty when the
    /// pod manifest lists no containers.
    pub fn container_names(&self) -> Vec<String> {
        self.containers.iter().map(|c| c.name.clone()).collect()
    }
}

/// Map a state sub-object to a human label (Running / Waiting: <reason> /
/// Terminated: <reason> / Pending / Unknown).
pub fn state_label(
    running: bool,
    waiting_reason: Option<&str>,
    terminated_reason: Option<&str>,
    exit_code: Option<i32>,
) -> String {
    if running {
        return "Running".to_string();
    }
    if let Some(reason) = waiting_reason {
        return format!("Waiting: {reason}");
    }
    if let Some(reason) = terminated_reason {
        return format!("Terminated: {reason}");
    }
    if let Some(code) = exit_code {
        return format!("Terminated (exit {code})");
    }
    "Unknown".to_string()
}

/// Build a `PodSummary` from scalar fields.
pub fn pod_summary(
    phase: Option<&str>,
    node: Option<&str>,
    pod_ip: Option<&str>,
    qos: Option<&str>,
    reason: Option<String>,
    message: Option<String>,
) -> PodSummary {
    PodSummary {
        phase: phase.unwrap_or("Unknown").to_string(),
        node: node.unwrap_or("").to_string(),
        pod_ip: pod_ip.unwrap_or("").to_string(),
        qos: qos.unwrap_or("").to_string(),
        reason,
        message,
    }
}

/// Sort events newest-first by their `last_timestamp`. Events without a
/// timestamp sink to the bottom (None sorts before Some).
pub fn sort_events_by_timestamp(events: &mut [(Option<String>, String)]) {
    events.sort_by(|a, b| b.0.cmp(&a.0));
}

/// The first non-empty container name from a list, or `None` if every entry
/// is empty or the list is empty. Reused by both `LogsView` and the
/// inspector's `LogsTab`.
pub fn pick_default_container(containers: &[String]) -> Option<String> {
    containers.iter().find(|c| !c.is_empty()).cloned()
}

/// One log record split the way the reference's `.log-line` is: the clock, the
/// severity tag, an optional HTTP method and the message body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogLine {
    /// Clock the line carries: a kube RFC 3339 stamp reduced to `HH:MM:SS.mmm`,
    /// or the `HH:MM:SS` a writer printed itself. Empty when absent.
    pub time: String,
    /// Severity word, upper-cased (`INFO`, `WARN`, `ERROR`, …). Empty when the
    /// line names no level.
    pub level: String,
    /// HTTP method (`GET`, `POST`, …). Empty when the line carries none.
    pub method: String,
    /// Everything after the recognised prefixes, verbatim.
    pub message: String,
}

impl LogLine {
    /// The severity's `.log-level` modifier: `warn` / `error`, empty for the
    /// info default the reference paints with `--log-info`.
    pub fn level_class(&self) -> &'static str {
        level_class(&self.level)
    }

    /// Whether the message body takes the severity's colour too — the
    /// reference marks an `ERROR` line's `.log-msg` with `.error` as well.
    pub fn message_is_error(&self) -> bool {
        self.level_class() == "error"
    }
}

/// Words [`parse_log_line`] accepts as a severity tag.
const LEVEL_WORDS: [&str; 8] = [
    "info", "warn", "warning", "error", "err", "debug", "trace", "fatal",
];

/// Methods the reference's `.log-method` column tags. Matched case-sensitively:
/// a message that merely starts with `Delete` is prose, not an access log.
const HTTP_METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

/// Split one raw stream line into the reference's four columns.
///
/// The host streams with `timestamps: true`, so a kube line opens with an RFC
/// 3339 stamp; a writer that prints its own clock (`10:42:07.114 INFO …`) is
/// understood too. Anything unrecognised stays in `message` verbatim.
pub fn parse_log_line(raw: &str) -> LogLine {
    let (time, rest) = split_leading_timestamp(raw);
    let mut cursor = rest.trim_start();

    let mut level = String::new();
    if let Some((token, after)) = take_token(cursor) {
        if let Some(word) = level_word(token) {
            level = word;
            cursor = after;
        }
    }

    let mut method = String::new();
    if let Some((token, after)) = take_token(cursor) {
        if HTTP_METHODS.contains(&token) {
            method = token.to_string();
            cursor = after;
        }
    }

    LogLine {
        time,
        level,
        method,
        message: cursor.trim_start().to_string(),
    }
}

/// Map a severity (a whole line, or the bare word from [`LogLine`]) to the
/// `.log-level` modifier. Empty when no level is recognised — the info default
/// carries no modifier.
pub fn level_class(line: &str) -> &'static str {
    match level_word(line.split_ascii_whitespace().next().unwrap_or("")).as_deref() {
        Some("WARN") => "warn",
        Some("ERROR") | Some("FATAL") => "error",
        _ => "",
    }
}

/// The leading timestamp of a line, as `(clock, remainder)`.
fn split_leading_timestamp(raw: &str) -> (String, &str) {
    let text = raw.trim_start();
    let Some((token, rest)) = take_token(text) else {
        return (String::new(), text);
    };
    if let Some(clock) = rfc3339_clock(token) {
        return (clock, rest);
    }
    if is_clock(token) {
        return (token.to_string(), rest);
    }
    (String::new(), text)
}

/// Split the leading whitespace-delimited token off `text`.
fn take_token(text: &str) -> Option<(&str, &str)> {
    let text = text.trim_start();
    if text.is_empty() {
        return None;
    }
    match text.find(char::is_whitespace) {
        Some(at) => Some((&text[..at], &text[at..])),
        None => Some((text, "")),
    }
}

/// `2026-10-03T10:42:07.114Z` (or `…+00:00`) reduced to `10:42:07.114`.
fn rfc3339_clock(token: &str) -> Option<String> {
    let (date, rest) = token.split_once('T')?;
    let valid_date = date.len() == 10
        && date.chars().enumerate().all(|(index, c)| {
            if index == 4 || index == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        });
    if !valid_date {
        return None;
    }
    let rest = rest.trim_end_matches('Z');
    let time = rest.split(['+', '-']).next().unwrap_or(rest);
    if !is_clock(time) {
        return None;
    }
    Some(time.to_string())
}

/// `HH:MM:SS`, with an optional `.fff` fraction.
fn is_clock(token: &str) -> bool {
    let (hms, fraction) = match token.split_once('.') {
        Some((hms, fraction)) => (hms, Some(fraction)),
        None => (token, None),
    };
    let fields: Vec<&str> = hms.split(':').collect();
    if fields.len() != 3
        || !fields
            .iter()
            .all(|field| field.len() == 2 && field.chars().all(|c| c.is_ascii_digit()))
    {
        return false;
    }
    match fraction {
        Some(fraction) => !fraction.is_empty() && fraction.chars().all(|c| c.is_ascii_digit()),
        None => true,
    }
}

/// Normalise a severity token (`INFO`, `[warn]`, `error:`) to its display form.
fn level_word(token: &str) -> Option<String> {
    let trimmed =
        token.trim_matches(|c: char| matches!(c, '[' | ']' | '(' | ')' | ':' | ',' | ';'));
    let lower = trimmed.to_ascii_lowercase();
    if !LEVEL_WORDS.contains(&lower.as_str()) {
        return None;
    }
    Some(match lower.as_str() {
        "warn" | "warning" => "WARN".to_string(),
        "err" | "error" => "ERROR".to_string(),
        _ => trimmed.to_ascii_uppercase(),
    })
}

/// A bounded line buffer the log viewer drains the stream into. The cap keeps
/// the rendered DOM small; the public API stays minimal so the viewer swap is
/// mechanical.
pub const MAX_LINES: usize = 10_000;

/// A fixed-capacity buffer of log lines that drops the oldest when full.
#[derive(Debug, Clone)]
pub struct LineBuffer {
    lines: Vec<String>,
    cap: usize,
}

impl Default for LineBuffer {
    fn default() -> Self {
        Self::with_cap(MAX_LINES)
    }
}

impl LineBuffer {
    pub fn with_cap(cap: usize) -> Self {
        Self {
            lines: Vec::new(),
            cap,
        }
    }

    /// Append a single line, evicting the overflow batch when at cap.
    pub fn push(&mut self, line: String) {
        self.lines.push(line);
        if self.lines.len() > self.cap {
            let overflow = self.lines.len() - self.cap;
            self.lines.drain(..overflow);
        }
    }

    /// Drop every retained line.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Whether the buffer has no retained lines.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Current retained lines (caller is free to iterate or clone).
    pub fn lines(&self) -> &[String] {
        &self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pod_summary_defaults_phase_to_unknown() {
        let s = pod_summary(None, None, None, None, None, None);
        assert_eq!(s.phase, "Unknown");
        assert_eq!(s.node, "");
        assert_eq!(s.qos, "");
    }

    #[test]
    fn pod_summary_keeps_optional_reason_and_message() {
        let s = pod_summary(
            Some("Running"),
            Some("n1"),
            Some("10.0.0.7"),
            Some("Guaranteed"),
            Some("Evicted".into()),
            Some("low mem".into()),
        );
        assert_eq!(s.phase, "Running");
        assert_eq!(s.node, "n1");
        assert_eq!(s.pod_ip, "10.0.0.7");
        assert_eq!(s.qos, "Guaranteed");
        assert_eq!(s.reason.as_deref(), Some("Evicted"));
        assert_eq!(s.message.as_deref(), Some("low mem"));
    }

    #[test]
    fn state_label_running_when_running_flag_set() {
        assert_eq!(state_label(true, None, None, None), "Running");
    }

    #[test]
    fn state_label_waiting_with_reason() {
        assert_eq!(
            state_label(false, Some("CrashLoopBackOff"), None, None),
            "Waiting: CrashLoopBackOff"
        );
    }

    #[test]
    fn state_label_terminated_with_reason_or_exit_code() {
        assert_eq!(
            state_label(false, None, Some("OOMKilled"), None),
            "Terminated: OOMKilled"
        );
        assert_eq!(
            state_label(false, None, None, Some(137)),
            "Terminated (exit 137)"
        );
    }

    #[test]
    fn state_label_falls_back_to_unknown() {
        assert_eq!(state_label(false, None, None, None), "Unknown");
    }

    #[test]
    fn pick_default_container_returns_first_non_empty() {
        assert_eq!(
            pick_default_container(&["a".into(), "b".into()]),
            Some("a".into())
        );
        assert_eq!(
            pick_default_container(&["".into(), "a".into()]),
            Some("a".into())
        );
        assert_eq!(pick_default_container(&[]), None);
        assert_eq!(pick_default_container(&["".into()]), None);
    }

    #[test]
    fn level_class_recognises_warn_and_error() {
        assert_eq!(level_class("WARN foo"), "warn");
        assert_eq!(level_class("ERROR bar"), "error");
        assert_eq!(level_class("ERR err"), "error");
        assert_eq!(level_class("FATAL boom"), "error");
        assert_eq!(level_class("INFO baz"), "");
        assert_eq!(level_class(""), "");
        assert_eq!(level_class("   "), "");
    }

    #[test]
    fn parse_log_line_splits_a_kube_line_into_the_reference_columns() {
        let line = parse_log_line("2026-10-03T10:42:07.114Z INFO GET /api/v1/orders 200 18.4ms");
        assert_eq!(line.time, "10:42:07.114");
        assert_eq!(line.level, "INFO");
        assert_eq!(line.method, "GET");
        assert_eq!(line.message, "/api/v1/orders 200 18.4ms");
        assert_eq!(line.level_class(), "");
    }

    #[test]
    fn parse_log_line_reads_a_writer_printed_clock_and_severity() {
        let line = parse_log_line("10:42:09.204 ERROR GET /internal/queue depth exceeded");
        assert_eq!(line.time, "10:42:09.204");
        assert_eq!(line.level, "ERROR");
        assert_eq!(line.method, "GET");
        assert_eq!(line.message, "/internal/queue depth exceeded");
        assert_eq!(line.level_class(), "error");
        assert!(line.message_is_error());
    }

    #[test]
    fn parse_log_line_keeps_unrecognised_text_in_the_message() {
        let line = parse_log_line("plain message with no prefixes");
        assert_eq!(line.time, "");
        assert_eq!(line.level, "");
        assert_eq!(line.method, "");
        assert_eq!(line.message, "plain message with no prefixes");
        assert!(!line.message_is_error());
    }

    #[test]
    fn parse_log_line_leaves_prose_that_looks_like_a_method_alone() {
        let line = parse_log_line("2026-10-03T10:42:07Z Delete the stale endpoint");
        assert_eq!(line.time, "10:42:07");
        assert_eq!(line.level, "");
        assert_eq!(line.method, "");
        assert_eq!(line.message, "Delete the stale endpoint");
    }

    #[test]
    fn parse_log_line_tolerates_brackets_and_lowercase_levels() {
        let line = parse_log_line("[warn] upstream 10.0.0.1 slow");
        assert_eq!(line.level, "WARN");
        assert_eq!(line.method, "");
        assert_eq!(line.message, "upstream 10.0.0.1 slow");
        assert_eq!(line.level_class(), "warn");
    }

    #[test]
    fn parse_log_line_accepts_an_empty_input() {
        assert_eq!(parse_log_line(""), LogLine::default());
        assert_eq!(parse_log_line("   "), LogLine::default());
    }

    #[test]
    fn line_buffer_caps_and_clears() {
        let mut buf = LineBuffer::with_cap(2);
        assert!(buf.is_empty());
        buf.push("a".into());
        buf.push("b".into());
        buf.push("c".into());
        assert_eq!(buf.lines(), &["b".to_string(), "c".to_string()]);
        buf.clear();
        assert!(buf.is_empty());
    }

    #[test]
    fn container_names_returns_in_order() {
        let pod = PodObject {
            containers: vec![
                ContainerInfo {
                    name: "x".into(),
                    image: "img".into(),
                    ready: true,
                    restarts: 0,
                    state: "Running".into(),
                },
                ContainerInfo {
                    name: "y".into(),
                    image: "img".into(),
                    ready: false,
                    restarts: 1,
                    state: "Pending".into(),
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            pod.container_names(),
            vec!["x".to_string(), "y".to_string()]
        );
    }

    #[test]
    fn sort_events_newest_timestamp_first() {
        let mut events = vec![
            (Some("2025-01-01T00:00:00Z".into()), "old".into()),
            (Some("2025-01-03T00:00:00Z".into()), "newest".into()),
            (Some("2025-01-02T00:00:00Z".into()), "mid".into()),
            (None, "untimestamped".into()),
        ];
        sort_events_by_timestamp(&mut events);
        let labels: Vec<&str> = events.iter().map(|(_, msg)| msg.as_str()).collect();
        assert_eq!(labels, vec!["newest", "mid", "old", "untimestamped"]);
    }
}
