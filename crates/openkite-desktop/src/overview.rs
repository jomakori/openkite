//! The live overview dashboard, desktop host (OKT-203).
//!
//! Reads the host's own reflector-backed signals — the same watches the
//! workload pages already paint — and hands the shared [`OverviewSummary`] to
//! [`OverviewDashboard`]. The browser host fills that same model from the
//! cluster client behind its gateway, so both paint one design.
//!
//! State rule (matching the browser host's precedence):
//! no client yet → `loading` (the app is still connecting); a client that was
//! attached and is gone → `error`; a client with no watches stood up yet →
//! `loading`; watches standing but nothing in the cluster → `empty`; otherwise
//! `ready`.

use dioxus::prelude::*;
use openkite_host::state::live;
use openkite_ui::components::overview::{
    OverviewDashboard, OverviewSummary, OverviewTile, TileTone,
};

/// The `/` surface, filled from the desktop's live watches.
#[component]
pub fn LiveOverview() -> Element {
    let client = crate::runtime::client();
    let attached_ever = *live::LIVE_GEN.read() > 0;
    let context = crate::runtime::CONTEXT
        .read()
        .clone()
        .unwrap_or_else(|| "no cluster".to_string());

    // Reading each watch subscribes this render to it; the snapshot clone is
    // cheap because the rows are `Arc`s. `None` means the host has not stood
    // that watch up.
    let pods = live::pods_signal().map(|signal| signal.read().clone());
    let nodes = live::nodes_signal().map(|signal| signal.read().clone());
    let deployments = live::deployments_signal().map(|signal| signal.read().clone());
    let services = live::services_signal().map(|signal| signal.read().clone());

    let watched = pods.is_some() && nodes.is_some() && deployments.is_some() && services.is_some();
    let summary = OverviewSummary {
        context,
        gateway: "in-process".to_string(),
        connection: if client.is_some() {
            "Connected".to_string()
        } else {
            "Disconnected".to_string()
        },
        tiles: if watched {
            tiles(
                pods.as_deref().unwrap_or_default(),
                nodes.as_deref().unwrap_or_default(),
                deployments.as_deref().unwrap_or_default(),
                services.as_deref().unwrap_or_default(),
            )
        } else {
            Vec::new()
        },
    };

    let (state, error) = if client.is_none() && !attached_ever {
        // Nothing is attached yet: the app is still connecting, so this is a
        // pending read, not a failure.
        ("loading".to_string(), None)
    } else if client.is_none() {
        (
            "error".to_string(),
            Some("the cluster client is not connected".to_string()),
        )
    } else if !watched {
        ("loading".to_string(), None)
    } else {
        (summary.state(false, None).to_string(), None)
    };

    rsx! {
        OverviewDashboard {
            summary: summary.clone(),
            state: state.clone(),
            error: error.clone(),
        }
    }
}

/// The tiles: what the watches counted, and how each count reads.
fn tiles(
    pods: &[std::sync::Arc<k8s_openapi::api::core::v1::Pod>],
    nodes: &[std::sync::Arc<k8s_openapi::api::core::v1::Node>],
    deployments: &[std::sync::Arc<k8s_openapi::api::apps::v1::Deployment>],
    services: &[std::sync::Arc<k8s_openapi::api::core::v1::Service>],
) -> Vec<OverviewTile> {
    let total = pods.len();
    let mut running = 0;
    let mut pending = 0;
    let mut failing = 0;
    for pod in pods {
        let phase = pod
            .status
            .as_ref()
            .and_then(|status| status.phase.as_deref())
            .unwrap_or("Unknown");
        match phase {
            "Running" => running += 1,
            "Pending" => pending += 1,
            "Failed" => failing += 1,
            _ => {}
        }
    }
    let pod_tone = if total == 0 {
        TileTone::Neutral
    } else if failing > 0 {
        TileTone::Bad
    } else if pending > 0 {
        TileTone::Warn
    } else if running == total {
        TileTone::Ok
    } else {
        TileTone::Neutral
    };
    let pod_detail = if pending > 0 || failing > 0 {
        format!("{running} running · {pending} pending · {failing} failing")
    } else {
        format!("{running} running")
    };

    let node_total = nodes.len();
    let ready = nodes
        .iter()
        .filter(|node| {
            node.status
                .as_ref()
                .and_then(|status| status.conditions.as_ref())
                .map(|conditions| {
                    conditions
                        .iter()
                        .any(|condition| condition.type_ == "Ready" && condition.status == "True")
                })
                .unwrap_or(false)
        })
        .count();
    let node_tone = if node_total == 0 {
        TileTone::Neutral
    } else if ready == node_total {
        TileTone::Ok
    } else if ready == 0 {
        TileTone::Bad
    } else {
        TileTone::Warn
    };

    let deployment_total = deployments.len();
    let available = deployments
        .iter()
        .filter(|deployment| {
            let want = deployment
                .spec
                .as_ref()
                .and_then(|spec| spec.replicas)
                .unwrap_or(1);
            let have = deployment
                .status
                .as_ref()
                .and_then(|status| status.available_replicas)
                .unwrap_or(0);
            have >= want
        })
        .count();
    let deployment_tone = if deployment_total == 0 {
        TileTone::Neutral
    } else if available == deployment_total {
        TileTone::Ok
    } else {
        TileTone::Warn
    };

    vec![
        OverviewTile::new(
            "pods",
            "Pods",
            format!("{running}/{total}"),
            pod_detail,
            pod_tone,
        ),
        OverviewTile::new(
            "nodes",
            "Nodes",
            format!("{ready}/{node_total}"),
            "ready".to_string(),
            node_tone,
        ),
        OverviewTile::new(
            "deployments",
            "Deployments",
            format!("{available}/{deployment_total}"),
            "fully available".to_string(),
            deployment_tone,
        ),
        OverviewTile::new(
            "services",
            "Services",
            services.len().to_string(),
            "across all namespaces".to_string(),
            TileTone::Neutral,
        ),
    ]
}
