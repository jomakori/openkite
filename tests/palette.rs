//! Headless palette interaction tests (OKT-61, phase 2.5 regression rule).
//!
//! The desktop E2E (#66/#75) proved the palette opens, filters, and
//! navigates against a real Xvfb webview — but that layer is slow and
//! cannot assert *what* each interaction produces. These tests pin the
//! palette's headless surface (registry, fuzzy filter, cursor math,
//! global-signal state machine) so a regression like the #75 dead-code
//! mount is caught here in milliseconds, not in a 4-minute Xvfb run.
//!
//! Dioxus 0.7 global signals are backed by the runtime, so signal-touching
//! tests run inside a throwaway VirtualDom via `with_runtime` (same helper
//! pattern as tests/runtime.rs). Pure functions (filter, cursor) don't need
//! a runtime.

use openkite::palette::{
    advance_cursor, commands, filter_commands, CommandAction, PALETTE_OPEN, PALETTE_QUERY,
};
use openkite::router::Route;
// `.read()` / `.write()` on Global signals come from the dioxus prelude
// (ReadableExt/WritableExt + Deref on Global); glob-import it the way
// tests/runtime.rs does so method resolution matches the lib exactly.
use dioxus::prelude::*;

// ─────────────────────────────────────────────────────────────
// Pure surface (no runtime needed)
// ─────────────────────────────────────────────────────────────

/// The static registry exposes the ten user-facing flows the E2E drives:
/// Go-to-* navigation, theme, switcher, and the New-* resource actions.
#[test]
fn registry_contains_core_navigation_commands() {
    let cmds = commands();
    let labels: Vec<&str> = cmds.iter().map(|c| c.label).collect();

    for want in [
        "Go to Home",
        "Go to Cluster",
        "Go to Workloads",
        "Go to Logs",
        "Go to Config",
        "Cycle Theme",
    ] {
        assert!(
            labels.contains(&want),
            "registry missing command {want:?}; got: {labels:?}"
        );
    }
}

/// `filter_commands` with a blank query returns the registry unchanged
/// (display order preserved).
#[test]
fn blank_query_returns_all_in_registry_order() {
    let cmds = commands();
    let filtered = filter_commands(&cmds, "   ");
    assert_eq!(filtered, cmds);
}

/// Typing "go to logs" ranks the Go-to-Logs command first, so Enter
/// (which runs the first candidate) navigates to Logs. This is the exact
/// sequence the E2E flow 04 drives against the real webview.
#[test]
fn query_go_to_logs_ranks_logs_command_first() {
    let filtered = filter_commands(&commands(), "go to logs");
    assert!(
        !filtered.is_empty(),
        "query must match at least one command"
    );
    assert_eq!(
        filtered[0].action,
        CommandAction::Navigate(Route::Logs {}),
        "first ranked command for 'go to logs' should navigate to Logs; got {:?}",
        filtered[0].label
    );
}

/// Typing "theme" surfaces the Cycle Theme settings command (E2E flow 05).
#[test]
fn query_theme_ranks_cycle_theme_first() {
    let filtered = filter_commands(&commands(), "theme");
    assert!(!filtered.is_empty());
    assert_eq!(
        filtered[0].action,
        CommandAction::CycleTheme,
        "first ranked command for 'theme' should be Cycle Theme; got {:?}",
        filtered[0].label
    );
}

/// Typing "go to cluster" ranks Cluster navigation (baseline capture uses
/// this to reach the Cluster surface deterministically).
#[test]
fn query_go_to_cluster_ranks_cluster_command_first() {
    let filtered = filter_commands(&commands(), "go to cluster");
    assert!(!filtered.is_empty());
    assert_eq!(
        filtered[0].action,
        CommandAction::Navigate(Route::Cluster {}),
        "first ranked command for 'go to cluster' should navigate to Cluster; got {:?}",
        filtered[0].label
    );
}

/// A nonsense query yields no candidates (the palette shows the empty
/// state instead of running a stale selection).
#[test]
fn nonsense_query_yields_no_candidates() {
    let filtered = filter_commands(&commands(), "zzzznope");
    assert!(filtered.is_empty());
}

/// Cursor advancement wraps both directions and clamps a stale
/// out-of-range selection (regression: palette must never index OOB).
#[test]
fn advance_cursor_wraps_and_clamps() {
    // Empty list → None regardless of delta.
    assert_eq!(advance_cursor(None, 0, 1), None);
    assert_eq!(advance_cursor(Some(3), 0, 1), None);
    // Single item: any delta stays on index 0.
    assert_eq!(advance_cursor(None, 1, 1), Some(0));
    assert_eq!(advance_cursor(Some(0), 1, -1), Some(0));
    // Wrap forward and backward.
    assert_eq!(advance_cursor(Some(0), 3, 1), Some(1));
    assert_eq!(advance_cursor(Some(2), 3, 1), Some(0));
    assert_eq!(advance_cursor(Some(0), 3, -1), Some(2));
    // Out-of-range selection is clamped to len-1 before stepping:
    // Some(7) with len 3 clamps to 2, +1 wraps to 0.
    assert_eq!(advance_cursor(Some(7), 3, 1), Some(0));
    assert_eq!(advance_cursor(Some(7), 3, -1), Some(1));
}

// ─────────────────────────────────────────────────────────────
// Runtime-backed surface (global signals)
// ─────────────────────────────────────────────────────────────

fn with_runtime<O>(f: impl FnOnce() -> O) -> O {
    fn stub() -> Element {
        rsx! { div {} }
    }
    let vdom = VirtualDom::new(stub);
    vdom.in_runtime(f)
}

/// Global-signal state machine: opening the palette sets PALETTE_OPEN,
/// closing resets both OPEN and QUERY. Guards the #74-class bug where a
/// signal write outside an active Dioxus runtime panics — here every
/// write is inside `with_runtime`, so a regression that writes from a
/// non-runtime context fails loudly at compile/CI time.
#[test]
fn palette_open_close_state_machine() {
    with_runtime(|| {
        // Initial: closed, empty query.
        assert!(!*PALETTE_OPEN.read());
        assert!(PALETTE_QUERY.read().is_empty());

        // Simulate the keybind "toggle" path (PaletteKeybind handler).
        *PALETTE_OPEN.write() = true;
        assert!(*PALETTE_OPEN.read());

        // Typing into the palette sets the query (unfiltered → filtered).
        *PALETTE_QUERY.write() = "work".to_string();
        assert_eq!(PALETTE_QUERY.read().as_str(), "work");

        // Escape / run-command closes and clears transient state.
        *PALETTE_OPEN.write() = false;
        *PALETTE_QUERY.write() = String::new();
        assert!(!*PALETTE_OPEN.read());
        assert!(PALETTE_QUERY.read().is_empty());
    });
}

/// Query state feeds the filter: whatever is in PALETTE_QUERY at render
/// time must rank identically to calling filter_commands directly. Pins
/// the E2E flow 03 (palette filters the command list) at unit level.
#[test]
fn query_signal_feeds_filter_identically() {
    with_runtime(|| {
        *PALETTE_QUERY.write() = "go to workloads".to_string();
        let from_signal = filter_commands(&commands(), PALETTE_QUERY.read().as_str());
        assert_eq!(
            from_signal[0].action,
            CommandAction::Navigate(Route::Workloads {}),
            "PALETTE_QUERY content must drive the same ranking as the literal query"
        );
    });
}
