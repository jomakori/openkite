//! The cluster overview dashboard at `/` (OKT-203).
//!
//! Both hosts paint this surface from the *same* model, so the two renderings
//! cannot drift: the desktop fills [`OverviewSummary`] from its live watches,
//! the browser host from the cluster client behind its gateway. The component
//! is props in, markup out — the host owns the data, the crate owns the design.
//!
//! Four states, like every other data surface (see `docs/design-surfaces.md`):
//! a pending read paints `loading`, a refusal paints `error`, a client that
//! returned nothing paints `empty`, and only real numbers paint `ready`. The
//! state a summary is in is derived in one place ([`OverviewSummary::state`])
//! so the hosts cannot disagree about it.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::status_badge::{StatusKind, StatusPill};

/// How a tile's number reads: fine, worth a look, wrong, or merely factual.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TileTone {
    /// A count with nothing to flag.
    #[default]
    Neutral,
    /// Every one of them is where it should be.
    Ok,
    /// Some of them are not, and that is visible rather than fatal.
    Warn,
    /// The read found them but they are failing.
    Bad,
}

impl TileTone {
    /// The `data-tone` value the tile carries.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Bad => "bad",
        }
    }

    /// The dot class the tile's label leads with. Reuses the shell's status
    /// dots (`.dot.ok` / `.dot.warn` / `.dot.err`) rather than defining a
    /// second vocabulary for the same three colours.
    pub fn dot_class(self) -> &'static str {
        match self {
            Self::Neutral => "dot",
            Self::Ok => "dot ok",
            Self::Warn => "dot warn",
            Self::Bad => "dot err",
        }
    }
}

/// One status tile: a label, a headline number, and the line that qualifies it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewTile {
    /// Stable key the tile is addressed by (`pods`, `nodes`, …).
    pub key: String,
    /// The micro-label, e.g. `Pods`.
    pub label: String,
    /// The headline, e.g. `40/42`.
    pub value: String,
    /// The qualifier, e.g. `running of all pods`.
    pub detail: String,
    /// How the number reads.
    pub tone: TileTone,
}

impl OverviewTile {
    /// A tile from its parts.
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        value: impl Into<String>,
        detail: impl Into<String>,
        tone: TileTone,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value: value.into(),
            detail: detail.into(),
            tone,
        }
    }
}

/// What the overview dashboard paints: the cluster's identity and its tiles.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewSummary {
    /// The context label the host is reading, when it has one.
    pub context: String,
    /// The gateway kind: `server-side` or `in-process`.
    pub gateway: String,
    /// `Connected` or `Disconnected`, as the person reading it should hear it.
    pub connection: String,
    /// The tiles, in the order they are painted.
    pub tiles: Vec<OverviewTile>,
}

impl OverviewSummary {
    /// The state this summary paints, given the host's read status.
    ///
    /// Pending wins over a refusal (the read has not finished), a refusal wins
    /// over empty (an error is not an absence of data), and a summary with no
    /// tiles is empty rather than a grid of zeroes.
    pub fn state(&self, busy: bool, error: Option<&str>) -> &'static str {
        if busy {
            "loading"
        } else if error.is_some() {
            "error"
        } else if self.tiles.is_empty() {
            "empty"
        } else {
            "ready"
        }
    }
}

/// The dashboard surface at `/`.
///
/// The state is passed in rather than derived from `summary` alone: a host's
/// pending read and its refusal are host facts, and the caller has them.
#[component]
pub fn OverviewDashboard(
    #[props(default)] summary: Option<OverviewSummary>,
    /// `ready`, `empty`, `loading` or `error` — the value painted as
    /// `data-state` on the surface and on the node that carries the state.
    #[props(default = "ready".to_string())]
    state: String,
    /// The cluster client's refusal, painted in the error state.
    #[props(default)]
    error: Option<String>,
    /// Refresh the summary. Omitted by a host that cannot re-read.
    #[props(default)]
    on_refresh: Option<EventHandler<()>>,
) -> Element {
    let summary = summary.unwrap_or_default();
    let context = if summary.context.is_empty() {
        "context unknown".to_string()
    } else {
        summary.context.clone()
    };
    let gateway = if summary.gateway.is_empty() {
        "unknown".to_string()
    } else {
        summary.gateway.clone()
    };

    // The identity line joins only what the host reported: a host with no
    // connection label must not leave a hollow separator in the sentence.
    let mut parts = vec![context];
    if !summary.connection.is_empty() {
        parts.push(summary.connection.clone());
    }
    parts.push(format!("gateway {gateway}"));
    let meta = parts.join(" · ");
    // The pill states the surface's own health, in the same vocabulary the
    // resource pages already use.
    let pill = match state.as_str() {
        "loading" => StatusKind::Pending,
        "error" => StatusKind::Failed,
        "empty" => StatusKind::Unknown,
        _ => StatusKind::Ready,
    };

    let body: Element = if state == "loading" {
        rsx! {
            div { class: "table-state table-loading", "data-state": "loading",
                "Reading the cluster…"
            }
        }
    } else if state == "error" {
        let message = error.unwrap_or_else(|| "the cluster client could not answer".to_string());
        rsx! {
            div { class: "table-state table-error", "data-state": "error", "data-error": "1",
                "The cluster client could not read the overview: {message}"
            }
        }
    } else if state == "empty" {
        rsx! {
            div { class: "table-state", "data-state": "empty",
                "Nothing to summarize yet — the cluster client returned no pods, nodes, deployments or services."
            }
        }
    } else {
        rsx! {
            div { class: "overview-grid",
                for tile in summary.tiles.iter() {
                    div { class: "stat-tile", "data-tile": "{tile.key}", "data-tone": "{tile.tone.as_str()}",
                        span { class: "stat-tile-label",
                            span { class: "{tile.tone.dot_class()}" }
                            "{tile.label}"
                        }
                        span { class: "stat-tile-value", "{tile.value}" }
                        span { class: "stat-tile-detail", "{tile.detail}" }
                    }
                }
            }
        }
    };

    rsx! {
        section { class: "panel", "data-surface": "overview", "data-state": "{state}",
            div { class: "overview-head",
                div {
                    div { class: "eyebrow", "Overview" }
                    h2 { "Cluster" }
                    p { class: "overview-meta", "{meta}" }
                }
                div { class: "page-actions",
                    StatusPill { status: pill }
                    if let Some(on_refresh) = on_refresh {
                        button {
                            class: "btn btn-primary",
                            r#type: "button",
                            "data-action": "refresh",
                            onclick: move |_| on_refresh.call(()),
                            "Refresh"
                        }
                    }
                }
            }
            {body}
        }
    }
}
