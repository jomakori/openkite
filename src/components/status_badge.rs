//! Status badge — semantic pill for resource status.

// Some `StatusKind` variants have no consumer yet.
#![allow(dead_code)]
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
    /// CSS class for the legacy badge (semantic color).
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

    /// Design-system pill variant: maps the semantic status onto the
    /// `.pill.success` / `.pill.warn` / `.pill.danger` / `.pill.muted`
    /// classes from the design-system stylesheet.
    pub fn pill_class(self) -> &'static str {
        match self {
            StatusKind::Running | StatusKind::Ready | StatusKind::Succeeded => "success",
            StatusKind::Pending | StatusKind::OutOfSync => "warn",
            StatusKind::Failed | StatusKind::CrashLoop | StatusKind::Degraded => "danger",
            StatusKind::Unknown | StatusKind::Suspended => "muted",
        }
    }
}

/// A small colored pill denoting `status` (legacy `.status-badge` styling).
#[component]
pub fn StatusBadge(status: StatusKind) -> Element {
    rsx! {
        span { class: "status-badge {status.class()}", "{status.label()}" }
    }
}

/// A design-system status pill (`.pill` + semantic variant) denoting `status`.
#[component]
pub fn StatusPill(status: StatusKind) -> Element {
    rsx! {
        span { class: "pill {status.pill_class()}", "{status.label()}" }
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

    #[test]
    fn pill_classes_group_by_severity() {
        assert_eq!(
            StatusKind::Running.pill_class(),
            StatusKind::Ready.pill_class()
        );
        assert_eq!(
            StatusKind::Failed.pill_class(),
            StatusKind::CrashLoop.pill_class()
        );
        assert_ne!(
            StatusKind::Running.pill_class(),
            StatusKind::Failed.pill_class()
        );
    }
}
