//! Status badge — semantic pill for resource status (OKT-8).

#![allow(dead_code)] // consumed by resource_table (OKT-8) and the views (OKT-10+)
#![allow(non_snake_case)]

use dioxus::prelude::*;

/// Semantic statuses a resource can be in. The badge maps each to a CSS class
/// (`status-ok` / `status-warn` / `status-err` / `status-muted`) whose palette
/// lives in the theme CSS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Running,
    Ready,
    Pending,
    Succeeded,
    Failed,
    CrashLoop,
    Unknown,
    OutOfSync,
    Degraded,
    Suspended,
}

impl StatusKind {
    /// CSS class for the pill (semantic color).
    pub fn class(self) -> &'static str {
        match self {
            StatusKind::Running | StatusKind::Ready | StatusKind::Succeeded => "status-ok",
            StatusKind::Pending | StatusKind::OutOfSync => "status-warn",
            StatusKind::Failed | StatusKind::CrashLoop | StatusKind::Degraded => "status-err",
            StatusKind::Unknown | StatusKind::Suspended => "status-muted",
        }
    }

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            StatusKind::Running => "Running",
            StatusKind::Ready => "Ready",
            StatusKind::Pending => "Pending",
            StatusKind::Succeeded => "Succeeded",
            StatusKind::Failed => "Failed",
            StatusKind::CrashLoop => "CrashLoop",
            StatusKind::Unknown => "Unknown",
            StatusKind::OutOfSync => "OutOfSync",
            StatusKind::Degraded => "Degraded",
            StatusKind::Suspended => "Suspended",
        }
    }
}

/// A small colored pill denoting `status`.
#[component]
pub fn StatusBadge(status: StatusKind) -> Element {
    rsx! {
        span { class: "status-badge {status.class()}", "{status.label()}" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_has_class_and_label() {
        let all = [
            StatusKind::Running,
            StatusKind::Ready,
            StatusKind::Pending,
            StatusKind::Succeeded,
            StatusKind::Failed,
            StatusKind::CrashLoop,
            StatusKind::Unknown,
            StatusKind::OutOfSync,
            StatusKind::Degraded,
            StatusKind::Suspended,
        ];
        for status in all {
            assert!(!status.class().is_empty(), "{status:?} missing class");
            assert!(!status.label().is_empty(), "{status:?} missing label");
        }
    }

    #[test]
    fn semantic_colors_group_by_severity() {
        assert_eq!(StatusKind::Running.class(), StatusKind::Ready.class());
        assert_eq!(StatusKind::Failed.class(), StatusKind::CrashLoop.class());
        assert_ne!(StatusKind::Running.class(), StatusKind::Failed.class());
    }
}
