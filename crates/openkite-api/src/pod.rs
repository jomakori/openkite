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

/// Whether the `.log-paused` hint should be rendered: true when follow is
/// off OR the user has scrolled up.
pub fn should_show_paused_hint(following: bool, at_bottom: bool) -> bool {
    !following || !at_bottom
}

/// Map a log line's first whitespace-delimited token to a CSS class. Empty
/// when no recognised level is found (the viewer wraps the line as info).
pub fn level_class(line: &str) -> &'static str {
    let head = line.split_ascii_whitespace().next().unwrap_or("");
    match head {
        "WARN" | "warn" => "warn",
        "ERROR" | "ERR" | "error" | "err" => "error",
        _ => "",
    }
}

/// A bounded line buffer the log viewer drains the stream into. The cap keeps
/// the rendered DOM small; the public API stays minimal so the viewer swap is
/// mechanical.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LineBuffer {
    lines: Vec<String>,
    cap: usize,
}

impl LineBuffer {
    pub fn with_cap(cap: usize) -> Self {
        Self {
            lines: Vec::new(),
            cap,
        }
    }

    /// Append a single line; drop the oldest when at cap.
    pub fn push(&mut self, line: String) {
        if self.lines.len() >= self.cap {
            self.lines.remove(0);
        }
        self.lines.push(line);
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
    fn should_show_paused_hint_logic() {
        assert!(should_show_paused_hint(false, true));
        assert!(should_show_paused_hint(true, false));
        assert!(!should_show_paused_hint(true, true));
        assert!(should_show_paused_hint(false, false));
    }

    #[test]
    fn level_class_recognises_warn_and_error() {
        assert_eq!(level_class("WARN foo"), "warn");
        assert_eq!(level_class("ERROR bar"), "error");
        assert_eq!(level_class("ERR err"), "error");
        assert_eq!(level_class("INFO baz"), "");
        assert_eq!(level_class(""), "");
        assert_eq!(level_class("   "), "");
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
