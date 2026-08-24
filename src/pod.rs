//! Pod detail: container info, status summary, and event ordering.

use k8s_openapi::api::core::v1::{ContainerState, Event, Pod};

/// A single container's rendered detail row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerInfo {
    /// Container name (from the pod spec).
    pub name: String,
    /// Image reference; empty when the spec omits it.
    pub image: String,
    /// Whether the container is ready (from status).
    pub ready: bool,
    /// Cumulative restart count.
    pub restarts: i32,
    /// Human-readable state, e.g. `Running` or `Waiting: CrashLoopBackOff`.
    pub state: String,
}

/// The pod's high-level status summary (Overview tab).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PodSummary {
    pub phase: String,
    pub node: String,
    pub pod_ip: String,
    pub qos: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}

/// Human-readable container state, derived from its state sub-object.
fn state_label(state: Option<&ContainerState>) -> String {
    let Some(state) = state else {
        return "Pending".to_string();
    };
    if state.running.is_some() {
        return "Running".to_string();
    }
    if let Some(waiting) = &state.waiting {
        return match &waiting.reason {
            Some(reason) => format!("Waiting: {reason}"),
            None => "Waiting".to_string(),
        };
    }
    if let Some(terminated) = &state.terminated {
        return match &terminated.reason {
            Some(reason) => format!("Terminated: {reason}"),
            None => format!("Terminated (exit {})", terminated.exit_code),
        };
    }
    "Unknown".to_string()
}

/// Map a pod's spec containers and their statuses into detail rows.
///
/// Containers without a matching status report as `Pending` with zero
/// restarts.
pub fn container_infos(pod: &Pod) -> Vec<ContainerInfo> {
    let containers = pod
        .spec
        .as_ref()
        .map(|s| s.containers.as_slice())
        .unwrap_or_default();
    let statuses = pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_deref())
        .unwrap_or_default();

    containers
        .iter()
        .map(|c| {
            let status = statuses.iter().find(|s| s.name == c.name);
            ContainerInfo {
                name: c.name.clone(),
                image: c.image.clone().unwrap_or_default(),
                ready: status.map(|s| s.ready).unwrap_or(false),
                restarts: status.map(|s| s.restart_count).unwrap_or(0),
                state: state_label(status.and_then(|s| s.state.as_ref())),
            }
        })
        .collect()
}

/// Summarize a pod's status for the Overview tab.
pub fn pod_summary(pod: &Pod) -> PodSummary {
    let status = pod.status.as_ref();
    PodSummary {
        phase: status
            .and_then(|s| s.phase.clone())
            .unwrap_or_else(|| "Unknown".to_string()),
        node: pod
            .spec
            .as_ref()
            .and_then(|s| s.node_name.clone())
            .unwrap_or_default(),
        pod_ip: status.and_then(|s| s.pod_ip.clone()).unwrap_or_default(),
        qos: status.and_then(|s| s.qos_class.clone()).unwrap_or_default(),
        reason: status.and_then(|s| s.reason.clone()),
        message: status.and_then(|s| s.message.clone()),
    }
}

/// Sort events newest-first by `last_timestamp`; events without a timestamp
/// sink to the bottom.
pub fn sort_events(events: &mut [Event]) {
    events.sort_by_key(|e| std::cmp::Reverse(e.last_timestamp.clone()));
}
