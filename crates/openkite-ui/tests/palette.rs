//! The command palette surface (OKT-136 migration).
//!
//! The palette moved out of `openkite-desktop` into the shared crate, so these
//! tests own what the desktop's `tests/palette.rs` and `tests/palette_mount.rs`
//! used to cover: the registry, the fuzzy filter, the cursor math, the
//! global-signal state machine, and — new, because the crate no longer needs a
//! host router to render — the open panel itself.
//!
//! Command registration is driven by the host hooks plus the capability
//! descriptor the host publishes, so the desktop-shaped cases here seed
//! `Capabilities::in_process()` and the browser-shaped ones
//! `Capabilities::server_side()`.

mod support;

use std::sync::Mutex;

use dioxus::prelude::*;
use openkite_api::capability::Capabilities;
use openkite_ui::components::palette::{
    advance_cursor, annotate_system_with_effective, commands, filter_commands, CommandAction,
    CommandPalette, PaletteActions, PaletteHost, TitleBarTheme, PALETTE_OPEN, PALETTE_QUERY,
};
use openkite_ui::runtime::{set_gateway, set_published_capabilities};

// The capability slots are process-global; hold this guard in every test that
// reads or writes them so parallel test threads cannot see each other's host.
static CAPS: Mutex<()> = Mutex::new(());

fn caps_lock() -> std::sync::MutexGuard<'static, ()> {
    CAPS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Publish a descriptor for the duration of one test (the guard releases the
/// lock; the value is left in place, so each test publishes what it needs).
fn publish(caps: Capabilities) {
    set_gateway(None);
    set_published_capabilities(Some(caps));
}

/// The desktop's wiring: every hook the palette can dispatch through.
fn desktop_host() -> PaletteHost {
    PaletteHost {
        navigate: Some(EventHandler::new(|_| {})),
        cycle_theme: Some(EventHandler::new(|_| {})),
        toggle_menu_bar: Some(EventHandler::new(|_| {})),
        set_title_bar_theme: Some(EventHandler::new(|_| {})),
        new_resource: Some(EventHandler::new(|_| {})),
    }
}

/// The registry view of that wiring, without paying for the callbacks: the
/// registry reads [`PaletteActions`] only.
fn desktop_actions() -> PaletteActions {
    PaletteActions {
        navigate: true,
        cycle_theme: true,
        toggle_menu_bar: true,
        set_title_bar_theme: true,
        new_resource: true,
    }
}

fn ids(commands: &[openkite_ui::components::palette::Command]) -> Vec<&'static str> {
    commands.iter().map(|command| command.id).collect()
}

fn descriptions(commands: &[openkite_ui::components::palette::Command]) -> Vec<String> {
    commands
        .iter()
        .map(|command| command.description.clone())
        .collect()
}

#[test]
fn registry_contains_core_navigation_commands() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    let registry = commands(desktop_actions());
    let labels: Vec<&str> = registry.iter().map(|command| command.label).collect();
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

/// The display order a blank query renders, pinned end to end: the Go-to-*
/// entries, then the OS-chrome entries the descriptor allows, then the
/// cluster switcher and the resource actions.
#[test]
fn registry_order_is_stable_for_the_desktop_host() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    assert_eq!(
        ids(&commands(desktop_actions())),
        vec![
            "view.workloads",
            "view.logs",
            "view.cluster",
            "view.config",
            "view.home",
            "view.title-bar-theme-system",
            "view.title-bar-theme-light",
            "view.title-bar-theme-dark",
            "view.toggle-menu-bar",
            "cluster.switch",
            "new.pod",
            "new.deployment",
            "new.service",
            "new.configmap",
            "new.secret",
            "settings.theme",
        ]
    );
}

/// A browser host wires navigation and nothing else: the palette offers
/// exactly the entries a browser can complete, and the OS-chrome entries stay
/// out because the descriptor does not claim them.
#[test]
fn browser_host_is_offered_navigation_only() {
    let _guard = caps_lock();
    publish(Capabilities::server_side());

    let actions = PaletteActions {
        navigate: true,
        ..Default::default()
    };
    assert_eq!(
        ids(&commands(actions)),
        vec![
            "view.workloads",
            "view.logs",
            "view.cluster",
            "view.config",
            "view.home",
        ]
    );
}

/// The OS menu-bar toggle and the title-bar themes follow the published
/// descriptor, not the platform this test happens to run on.
#[test]
fn os_chrome_commands_follow_the_capability_descriptor() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    let with_chrome = commands(desktop_actions());
    assert!(with_chrome
        .iter()
        .any(|command| command.action == CommandAction::ToggleMenuBar));
    let theme_actions: Vec<&str> = with_chrome
        .iter()
        .filter(|command| matches!(command.action, CommandAction::SetTitleBarTheme(_)))
        .map(|command| command.label)
        .collect();
    assert_eq!(
        theme_actions,
        vec![
            "Title Bar Theme: System",
            "Title Bar Theme: Light",
            "Title Bar Theme: Dark",
        ]
    );
    assert_eq!(
        with_chrome
            .iter()
            .find(|command| matches!(command.action, CommandAction::SetTitleBarTheme(_)))
            .map(|command| command.action.clone()),
        Some(CommandAction::SetTitleBarTheme(TitleBarTheme::System))
    );

    publish(Capabilities::server_side());
    let without_chrome = commands(desktop_actions());
    assert!(!without_chrome
        .iter()
        .any(|command| command.action == CommandAction::ToggleMenuBar));
    assert!(!without_chrome
        .iter()
        .any(|command| matches!(command.action, CommandAction::SetTitleBarTheme(_))));
}

/// `filter_commands` with a blank query returns the registry unchanged
/// (display order preserved).
#[test]
fn blank_query_returns_all_in_registry_order() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    let registry = commands(desktop_actions());
    assert_eq!(filter_commands(&registry, "   "), registry);
}

/// Typing "go to logs" ranks the Go-to-Logs command first, so Enter (which
/// runs the first candidate) navigates to Logs. This is the exact sequence the
/// desktop E2E flow 04 drives against the real webview.
#[test]
fn query_go_to_logs_ranks_logs_command_first() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    let filtered = filter_commands(&commands(desktop_actions()), "go to logs");
    assert!(
        !filtered.is_empty(),
        "query must match at least one command"
    );
    assert_eq!(
        filtered[0].action,
        CommandAction::Navigate("/logs"),
        "first ranked command for 'go to logs' should navigate to Logs; got {:?}",
        filtered[0].label
    );
}

/// Typing "theme" surfaces the Cycle Theme settings command (E2E flow 05).
#[test]
fn query_theme_ranks_cycle_theme_first() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    let filtered = filter_commands(&commands(desktop_actions()), "theme");
    assert!(!filtered.is_empty());
    assert_eq!(
        filtered[0].action,
        CommandAction::CycleTheme,
        "first ranked command for 'theme' should be Cycle Theme; got {:?}",
        filtered[0].label
    );
}

/// Typing "go to cluster" ranks Cluster navigation (the visual baseline
/// capture uses this to reach the Cluster surface deterministically).
#[test]
fn query_go_to_cluster_ranks_cluster_command_first() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    let filtered = filter_commands(&commands(desktop_actions()), "go to cluster");
    assert!(!filtered.is_empty());
    assert_eq!(
        filtered[0].action,
        CommandAction::Navigate("/cluster"),
        "first ranked command for 'go to cluster' should navigate to Cluster; got {:?}",
        filtered[0].label
    );
}

/// A nonsense query yields no candidates (the palette shows the empty state
/// instead of running a stale selection).
#[test]
fn nonsense_query_yields_no_candidates() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    assert!(filter_commands(&commands(desktop_actions()), "zzzznope").is_empty());
}

/// The read-back annotation rewrites the System entry's description with the
/// theme the OS reports; a `None` read-back (or a registry without the entry)
/// leaves it untouched.
#[test]
fn system_description_reports_the_effective_theme() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    let mut registry = commands(desktop_actions());
    annotate_system_with_effective(&mut registry, None);
    assert!(
        descriptions(&registry)
            .iter()
            .any(|description| description == "Follow the OS decoration theme"),
        "a None read-back must leave the System description alone"
    );

    annotate_system_with_effective(&mut registry, Some("dark"));
    let system = registry
        .iter()
        .find(|command| command.action == CommandAction::SetTitleBarTheme(TitleBarTheme::System))
        .expect("System action registered when the descriptor allows it");
    assert!(
        system.description.contains("dark"),
        "got: {}",
        system.description
    );

    publish(Capabilities::server_side());
    let mut no_chrome = commands(desktop_actions());
    let before = descriptions(&no_chrome);
    annotate_system_with_effective(&mut no_chrome, Some("dark"));
    assert_eq!(
        descriptions(&no_chrome),
        before,
        "no System entry to annotate without the descriptor"
    );
}

/// Cursor advancement wraps both directions and clamps a stale out-of-range
/// selection (regression: palette must never index OOB).
#[test]
fn advance_cursor_wraps_and_clamps() {
    assert_eq!(advance_cursor(None, 0, 1), None);
    assert_eq!(advance_cursor(Some(3), 0, 1), None);
    assert_eq!(advance_cursor(None, 1, 1), Some(0));
    assert_eq!(advance_cursor(Some(0), 1, -1), Some(0));
    assert_eq!(advance_cursor(Some(0), 3, 1), Some(1));
    assert_eq!(advance_cursor(Some(2), 3, 1), Some(0));
    assert_eq!(advance_cursor(Some(0), 3, -1), Some(2));
    assert_eq!(advance_cursor(Some(7), 3, 1), Some(0));
    assert_eq!(advance_cursor(Some(7), 3, -1), Some(1));
}

fn with_runtime<O>(f: impl FnOnce() -> O) -> O {
    fn stub() -> Element {
        rsx! { div {} }
    }
    let vdom = VirtualDom::new(stub);
    vdom.in_runtime(f)
}

/// Global-signal state machine: opening the palette sets PALETTE_OPEN,
/// closing resets both OPEN and QUERY. Every write happens inside a Dioxus
/// runtime, so a regression that writes off-runtime fails loudly.
#[test]
fn palette_open_close_state_machine() {
    with_runtime(|| {
        assert!(!*PALETTE_OPEN.read());
        assert!(PALETTE_QUERY.read().is_empty());

        *PALETTE_OPEN.write() = true;
        assert!(*PALETTE_OPEN.read());

        *PALETTE_QUERY.write() = "work".to_string();
        assert_eq!(PALETTE_QUERY.read().as_str(), "work");

        *PALETTE_OPEN.write() = false;
        *PALETTE_QUERY.write() = String::new();
        assert!(!*PALETTE_OPEN.read());
        assert!(PALETTE_QUERY.read().is_empty());
    });
}

/// Query state feeds the filter: whatever is in PALETTE_QUERY at render time
/// must rank identically to calling filter_commands directly. Pins the E2E
/// flow 03 (palette filters the command list) at unit level.
#[test]
fn query_signal_feeds_filter_identically() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    with_runtime(|| {
        *PALETTE_QUERY.write() = "go to workloads".to_string();
        let from_signal =
            filter_commands(&commands(desktop_actions()), PALETTE_QUERY.read().as_str());
        assert_eq!(
            from_signal[0].action,
            CommandAction::Navigate("/workloads"),
            "PALETTE_QUERY content must drive the same ranking as the literal query"
        );
    });
}

fn closed_palette() -> Element {
    rsx! {
        CommandPalette { host: desktop_host() }
        div { "shell" }
    }
}

fn opened_palette() -> Element {
    rsx! {
        CommandPalette {
            host: desktop_host(),
            title_bar_effective: Some("dark".to_string()),
        }
        div { "shell" }
    }
}

/// Closed is the default: the overlay contributes nothing to the tree.
#[test]
fn closed_palette_renders_nothing() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    let html = support::mount_html(closed_palette, || {});
    assert!(html.contains("shell"), "got: {html}");
    assert!(!html.contains("palette"), "got: {html}");
}

/// Open renders the chrome the desktop webview shows: backdrop, filter field,
/// section labels and the command rows in registry order.
#[test]
fn open_palette_renders_sections_and_rows() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    let html = support::mount_html(opened_palette, || {
        *PALETTE_OPEN.write() = true;
    });
    for want in [
        "palette-backdrop",
        "palette-input",
        "palette-list",
        "palette-section",
        ">View<",
        ">Settings<",
        "Go to Workloads",
        "Cycle Theme",
        "Toggle Menu Bar",
    ] {
        assert!(html.contains(want), "missing {want:?}: {html}");
    }
    assert!(
        html.contains("OS reports dark"),
        "the host's read-back reaches the System entry: {html}"
    );
}

/// The query signal drives what the open panel renders: a filtered list keeps
/// the match and drops the rest.
#[test]
fn open_palette_renders_the_filtered_list() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    let html = support::mount_html(opened_palette, || {
        *PALETTE_QUERY.write() = "go to logs".to_string();
        *PALETTE_OPEN.write() = true;
    });
    assert!(html.contains("Go to Logs"), "got: {html}");
    assert!(!html.contains("Go to Home"), "got: {html}");
}

/// A browser host's open palette shows its navigation entries and nothing the
/// browser cannot run.
#[test]
fn open_palette_for_a_browser_host_has_no_host_only_rows() {
    let _guard = caps_lock();
    publish(Capabilities::server_side());

    fn browser_palette() -> Element {
        rsx! {
            CommandPalette { host: PaletteHost::navigation(EventHandler::new(|_| {})) }
        }
    }

    let html = support::mount_html(browser_palette, || {
        *PALETTE_OPEN.write() = true;
    });
    assert!(html.contains("Go to Home"), "got: {html}");
    assert!(!html.contains("New Pod"), "got: {html}");
    assert!(!html.contains("Cycle Theme"), "got: {html}");
    assert!(!html.contains("Switch Cluster"), "got: {html}");
}

/// The empty state replaces the list when the query matches nothing.
#[test]
fn open_palette_renders_the_empty_state() {
    let _guard = caps_lock();
    publish(Capabilities::in_process());

    let html = support::mount_html(opened_palette, || {
        *PALETTE_QUERY.write() = "zzzznope".to_string();
        *PALETTE_OPEN.write() = true;
    });
    assert!(html.contains("no matching command"), "got: {html}");
    assert!(!html.contains("Go to Home"), "got: {html}");
}
