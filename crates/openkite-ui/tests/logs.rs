//! Headless mounts of the standalone log viewer (OKT-136 surface migration,
//! OKT-163 reference chrome).
//!
//! Mirrors `support::mount_html` from the existing surface: build a
//! throwaway `VirtualDom`, seed `SELECTED_POD` inside the vdom runtime,
//! rebuild in place, and snapshot the rendered HTML. The container, buffer and
//! pause helpers stay as pure logic in `openkite_api::pod` /
//! `openkite_ui::runtime`; the tests for those live alongside them so a future
//! wasm build only has to type-check one definition site.

mod support;

use dioxus::prelude::*;
use openkite_api::pod::{parse_log_line, ContainerInfo, PodObject, PodSummary};
use openkite_ui::components::logs::{LogLineRow, LogsView};
use openkite_ui::runtime::{
    close_log_sheet, dismiss_log_sheet_on_selection, pause_logs, resume_logs, toggle_log_sheet,
    toggle_logs_paused, LOGS_BUFFER, LOGS_CONTAINER, LOGS_HELD, LOGS_PAUSED, LOGS_SHEET_OPEN,
    SELECTED_POD,
};

/// A two-container pod, so the picker and the default are both observable.
fn log_pod() -> PodObject {
    PodObject {
        name: "web-1".into(),
        namespace: Some("default".into()),
        summary: PodSummary::default(),
        containers: vec![
            ContainerInfo {
                name: "web".into(),
                image: "nginx".into(),
                ready: true,
                restarts: 0,
                state: "Running".into(),
            },
            ContainerInfo {
                name: "sidecar".into(),
                image: "envoy".into(),
                ready: false,
                restarts: 3,
                state: "Waiting".into(),
            },
        ],
        labels: Default::default(),
        annotations: Default::default(),
        yaml: String::new(),
    }
}

fn empty_app() -> Element {
    rsx! { LogsView {} }
}

fn app_with_anchor() -> Element {
    rsx! {
        LogsView {}
        div { "shell-anchor" }
    }
}

fn log_row() -> Element {
    rsx! {
        LogLineRow {
            line: parse_log_line("2026-10-03T10:42:09.204Z ERROR GET /internal/queue depth exceeded"),
        }
    }
}

/// Seed the pod, the chosen container and the buffer a streamed panel would
/// have, so a mount paints lines instead of an empty state.
fn seed_streamed_panel() {
    *SELECTED_POD.write() = Some(log_pod());
    *LOGS_CONTAINER.write() = "web".to_string();
    LOGS_BUFFER.write().clear();
    LOGS_HELD.write().clear();
    *LOGS_PAUSED.write() = false;
    {
        let mut buffer = LOGS_BUFFER.write();
        buffer.push("2026-10-03T10:42:07.114Z INFO GET /api/v1/orders 200 18.4ms".to_string());
        buffer.push("2026-10-03T10:42:08.647Z WARN upstream 10.0.4.22:8080 slow 312ms".to_string());
        buffer
            .push("2026-10-03T10:42:09.204Z ERROR GET /internal/queue depth exceeded".to_string());
    }
}

fn with_runtime<O>(f: impl FnOnce() -> O) -> O {
    fn stub() -> Element {
        rsx! { div {} }
    }
    let vdom = VirtualDom::new(stub);
    vdom.in_runtime(f)
}

#[test]
fn no_selected_pod_renders_the_panel_chrome_and_the_prompt() {
    let html = support::mount_html(empty_app, || {});
    for want in ["log-panel", "log-header", "log-title", "log-body"] {
        assert!(html.contains(want), "missing {want:?}: {html}");
    }
    assert!(
        html.contains("Select a pod to view its logs"),
        "got: {html}"
    );
}

#[test]
fn selected_pod_renders_the_reference_header_and_actions() {
    let html = support::mount_html(empty_app, || {
        *openkite_ui::runtime::SELECTED_POD.write() = Some(log_pod());
    });
    for want in [
        "class=\"log-pod\"",
        "web-1",
        ">web<",
        ">sidecar<",
        "aria-label=\"Pause logs\"",
        "aria-pressed=\"false\"",
        "aria-label=\"Clear logs\"",
        "Open in inspector",
    ] {
        assert!(html.contains(want), "missing {want:?}: {html}");
    }
    // The pause control is hidden until a pod is chosen, but the drag handle
    // the ≤767px bottom sheet composes is always in the markup.
    assert!(html.contains("class=\"log-handle\""), "got: {html}");
}

#[test]
fn container_picker_default_is_first_non_empty() {
    let html = support::mount_html(empty_app, || {
        *openkite_ui::runtime::SELECTED_POD.write() = Some(log_pod());
    });
    assert!(
        html.contains("value=\"web\""),
        "select value should default to the first container, got: {html}"
    );
}

#[test]
fn seeded_renders_alongside_sibling_anchor() {
    let html = support::mount_html(app_with_anchor, || {
        *openkite_ui::runtime::SELECTED_POD.write() = Some(log_pod());
    });
    assert!(html.contains("shell-anchor"), "got: {html}");
    assert!(html.contains("log-panel"), "got: {html}");
    assert!(html.contains(">web<"), "got: {html}");
}

#[test]
fn a_log_line_splits_into_the_reference_columns() {
    let html = support::mount_html(log_row, || {});
    assert!(
        html.contains("class=\"log-time\">10:42:09.204<"),
        "got: {html}"
    );
    assert!(
        html.contains("class=\"log-level error\">ERROR<"),
        "got: {html}"
    );
    assert!(html.contains("class=\"log-method\">GET<"), "got: {html}");
    assert!(
        html.contains("class=\"log-msg error\">/internal/queue depth exceeded<"),
        "got: {html}"
    );
}

#[test]
fn buffered_lines_render_with_their_level_colours() {
    let html = support::mount_html(empty_app, seed_streamed_panel);
    assert!(html.contains("log-time\">10:42:07.114<"), "got: {html}");
    assert!(html.contains("class=\"log-level\">INFO<"), "got: {html}");
    assert!(
        html.contains("class=\"log-level warn\">WARN<"),
        "got: {html}"
    );
    assert!(
        html.contains("class=\"log-level error\">ERROR<"),
        "got: {html}"
    );
    assert!(html.contains("log-method\">GET<"), "got: {html}");
    assert!(
        html.contains(">upstream 10.0.4.22:8080 slow 312ms<"),
        "got: {html}"
    );
}

#[test]
fn a_paused_panel_shows_the_banner_and_holds_its_window() {
    let html = support::mount_html(empty_app, || {
        seed_streamed_panel();
        pause_logs();
        LOGS_BUFFER
            .write()
            .push("2026-10-03T10:42:12.900Z INFO arrived while paused".to_string());
    });
    assert!(html.contains("class=\"log-panel paused\""), "got: {html}");
    assert!(html.contains("role=\"status\""), "got: {html}");
    assert!(html.contains("Log stream paused"), "got: {html}");
    assert!(
        html.contains(">upstream 10.0.4.22:8080 slow 312ms<"),
        "the held window must keep the lines that were on screen: {html}"
    );
    assert!(
        !html.contains("arrived while paused"),
        "a paused panel must not paint lines that arrived after the pause: {html}"
    );
}

#[test]
fn pause_holds_the_window_and_resume_drops_no_line() {
    with_runtime(|| {
        seed_streamed_panel();
        let streamed = LOGS_BUFFER.read().lines().len();

        pause_logs();
        assert!(*LOGS_PAUSED.read(), "pause_logs must set the global");
        assert_eq!(LOGS_HELD.read().lines().len(), streamed);

        // The host keeps draining: only the painted window is held.
        LOGS_BUFFER
            .write()
            .push("2026-10-03T10:42:12.900Z INFO arrived while paused".to_string());
        assert_eq!(
            LOGS_HELD.read().lines().len(),
            streamed,
            "the held window must not grow while paused"
        );
        assert_eq!(LOGS_BUFFER.read().lines().len(), streamed + 1);

        resume_logs();
        assert!(!*LOGS_PAUSED.read(), "resume_logs must clear the global");
        assert!(LOGS_HELD.read().is_empty(), "the held window is released");
        assert_eq!(
            LOGS_BUFFER.read().lines().len(),
            streamed + 1,
            "resuming must not drop the stretch streamed while paused"
        );
    });
}

#[test]
fn toggle_logs_paused_alternates() {
    with_runtime(|| {
        *LOGS_PAUSED.write() = false;
        toggle_logs_paused();
        assert!(*LOGS_PAUSED.read());
        toggle_logs_paused();
        assert!(!*LOGS_PAUSED.read());
    });
}

/// Whether the shipped stylesheet declares a bare `.class` rule.
fn stylesheet_declares(class: &str) -> bool {
    let needle = format!(".{class}");
    let mut rest = openkite_ui::MAIN_CSS;
    while let Some(at) = rest.find(&needle) {
        let tail = &rest[at + needle.len()..];
        let boundary = tail
            .chars()
            .next()
            .map(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(true);
        if boundary {
            return true;
        }
        rest = tail;
    }
    false
}

/// Every class token in a rendered `class="…"` attribute.
fn rendered_classes(html: &str) -> Vec<String> {
    let mut classes = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("class=\"") {
        rest = &rest[at + "class=\"".len()..];
        let Some(end) = rest.find('"') else { break };
        classes.extend(rest[..end].split_whitespace().map(str::to_string));
        rest = &rest[end..];
    }
    classes.sort();
    classes.dedup();
    classes
}

/// The failure mode this pins is silent: a class the panel renders that no
/// stylesheet rule paints.
#[test]
fn every_class_the_panel_renders_is_declared() {
    let states = [
        support::mount_html(empty_app, || {}),
        support::mount_html(empty_app, seed_streamed_panel),
        support::mount_html(empty_app, || {
            seed_streamed_panel();
            pause_logs();
        }),
        support::mount_html(empty_app, || {
            seed_streamed_panel();
            *LOGS_SHEET_OPEN.write() = true;
        }),
    ];
    for html in states {
        for class in rendered_classes(&html) {
            assert!(
                stylesheet_declares(&class),
                "the log panel renders .{class} with no rule in assets/main.css: {html}"
            );
        }
    }
}

#[test]
fn level_colours_come_from_the_log_tokens() {
    let css = openkite_ui::MAIN_CSS;
    for selector in [
        ".log-level { color: var(--log-info)",
        ".log-level.warn { color: var(--warn)",
        ".log-level.error { color: var(--log-error)",
        ".log-msg.error { color: var(--log-error)",
        ".log-title .icon { color: var(--log-method)",
    ] {
        assert!(
            css.contains(selector),
            "the reference colours a log level through its own token; missing: {selector}"
        );
    }
}

// --- OKT-168: the ≤767px bottom sheet, its handle and Escape ---

#[test]
fn sheet_is_closed_by_default_so_it_does_not_cover_content() {
    let html = support::mount_html(empty_app, || {});
    assert!(html.contains("class=\"log-panel\""), "got: {html}");
    assert!(
        !html.contains("log-panel open"),
        "the sheet starts off-frame: {html}"
    );
    assert!(
        html.contains("class=\"log-handle\"") && html.contains("aria-expanded=\"false\""),
        "the handle is the collapsed affordance: {html}"
    );
}

#[test]
fn an_open_sheet_carries_the_open_class_and_an_expanded_handle() {
    let html = support::mount_html(empty_app, || {
        *LOGS_SHEET_OPEN.write() = true;
    });
    assert!(html.contains("class=\"log-panel open\""), "got: {html}");
    assert!(
        html.contains("aria-expanded=\"true\""),
        "the handle reports the open sheet: {html}"
    );
}

#[test]
fn an_open_paused_sheet_keeps_both_state_classes() {
    let html = support::mount_html(empty_app, || {
        seed_streamed_panel();
        pause_logs();
        *LOGS_SHEET_OPEN.write() = true;
    });
    assert!(
        html.contains("class=\"log-panel paused open\""),
        "paused and open are independent: {html}"
    );
}

#[test]
fn toggle_and_close_drive_the_sheet_state() {
    with_runtime(|| {
        *LOGS_SHEET_OPEN.write() = false;
        toggle_log_sheet();
        assert!(*LOGS_SHEET_OPEN.read(), "the handle opens a closed sheet");
        toggle_log_sheet();
        assert!(
            !*LOGS_SHEET_OPEN.read(),
            "the handle dismisses an open sheet"
        );
        *LOGS_SHEET_OPEN.write() = true;
        close_log_sheet();
        assert!(
            !*LOGS_SHEET_OPEN.read(),
            "Escape's close clears the open state"
        );
    });
}

#[test]
fn escape_is_wired_through_the_keybind_source() {
    let js = openkite_ui::components::logs::LOG_SHEET_KEYBIND_JS;
    assert!(js.contains("keydown"), "must be a key handler: {js}");
    assert!(js.contains("'Escape'"), "Escape is the dismiss key: {js}");
    assert!(
        js.contains("dioxus.send('close')"),
        "Escape must reach the Rust close path: {js}"
    );
}

/// The brace-balanced body of the first `@media` block whose header matches.
fn media_block(css: &str, header: &str) -> String {
    let start = css.find(header).expect("media header present");
    let open = css[start..].find('{').expect("media block opens") + start;
    let mut depth = 0usize;
    for (i, c) in css[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return css[open + 1..open + i].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced media block: {header}");
}

/// The sheet is a `≤767px` surface only — the `≥768px` blocks are
/// baseline-captured and must stay byte-identical.
#[test]
fn the_mobile_sheet_rules_live_only_below_the_breakpoint() {
    let css = openkite_ui::MAIN_CSS;
    let mobile = media_block(css, "@media (max-width: 767px)");
    assert!(
        mobile.contains(".log-panel {\n    position: fixed;"),
        "the fixed sheet lives in the mobile block: {mobile}"
    );
    assert!(
        mobile.contains(".log-panel.open { transform: translateY(0); }"),
        "the open state lives in the mobile block: {mobile}"
    );
    assert!(
        mobile.contains(".log-handle { display: flex; }"),
        "the handle only shows on mobile: {mobile}"
    );

    for header in [
        "@media (max-width: 1024px) and (min-width: 768px)",
        "@media (min-width: 1025px)",
    ] {
        let block = media_block(css, header);
        assert!(
            !block.contains(".log-panel"),
            "the sheet must not leak into {header}: {block}"
        );
    }

    let base = css
        .split(".log-panel {")
        .nth(1)
        .expect(".log-panel rule present");
    let base = base.split('}').next().expect(".log-panel block closes");
    assert!(
        !base.contains("position: fixed"),
        "the panel stays in-flow above the breakpoint: {base}"
    );
}

// --- OKT-168 gap fix: safe-area, page scroll lock, scroll containment, selection ---

/// The sheet clears the home indicator; the ≥768px panel keeps its own metrics.
#[test]
fn the_sheet_respects_the_safe_area_inset_only_on_mobile() {
    let css = openkite_ui::MAIN_CSS;
    let mobile = media_block(css, "@media (max-width: 767px)");
    assert!(
        mobile.contains("padding-bottom: env(safe-area-inset-bottom);"),
        "the sheet must clear the home indicator: {mobile}"
    );

    for header in [
        "@media (max-width: 1024px) and (min-width: 768px)",
        "@media (min-width: 1025px)",
    ] {
        let block = media_block(css, header);
        assert!(
            !block.contains("env(safe-area-inset-bottom)"),
            "the inset must not reach {header}: {block}"
        );
    }

    let base = css
        .split(".log-panel {")
        .nth(1)
        .expect(".log-panel rule present");
    let base = base.split('}').next().expect(".log-panel block closes");
    assert!(
        !base.contains("env(safe-area-inset-bottom)"),
        "the desktop panel is baseline-captured: {base}"
    );
}

/// The body must not scroll behind the open sheet, and must get its scroll back.
#[test]
fn the_open_sheet_locks_the_page_and_dismissal_releases_it() {
    use openkite_ui::components::logs::{LOG_SHEET_SCROLL_LOCK_JS, LOG_SHEET_SCROLL_RELEASE_JS};
    assert!(
        LOG_SHEET_SCROLL_LOCK_JS.contains("document.body.style.overflow = 'hidden'"),
        "opening the sheet pins the page: {LOG_SHEET_SCROLL_LOCK_JS}"
    );
    assert!(
        LOG_SHEET_SCROLL_RELEASE_JS.contains("document.body.style.overflow = ''"),
        "dismissal restores the page: {LOG_SHEET_SCROLL_RELEASE_JS}"
    );
}

/// Scrolling inside the sheet must not chain to the page it covers.
#[test]
fn the_sheet_body_contains_its_own_scroll() {
    let css = openkite_ui::MAIN_CSS;
    let mobile = media_block(css, "@media (max-width: 767px)");
    assert!(
        mobile.contains("overflow-y: auto; overscroll-behavior: contain;"),
        "the scroll container must not chain to the page: {mobile}"
    );

    let base = css
        .split(".log-body {")
        .nth(1)
        .expect(".log-body rule present");
    let base = base.split('}').next().expect(".log-body block closes");
    assert!(
        !base.contains("overscroll-behavior"),
        "the desktop log body is baseline-captured: {base}"
    );
}

/// A new pod dismisses the sheet; re-selecting the same one is not a change.
#[test]
fn a_selection_change_dismisses_the_sheet_and_a_reselect_does_not() {
    with_runtime(|| {
        *LOGS_SHEET_OPEN.write() = true;
        assert!(
            dismiss_log_sheet_on_selection(Some("web-1"), Some("api-2")),
            "a new pod must dismiss the sheet"
        );
        assert!(!*LOGS_SHEET_OPEN.read(), "the sheet is closed");

        *LOGS_SHEET_OPEN.write() = true;
        assert!(
            !dismiss_log_sheet_on_selection(Some("web-1"), Some("web-1")),
            "re-selecting the same pod keeps the sheet"
        );
        assert!(*LOGS_SHEET_OPEN.read(), "the sheet stays open");

        *LOGS_SHEET_OPEN.write() = true;
        assert!(
            dismiss_log_sheet_on_selection(Some("web-1"), None),
            "clearing the selection dismisses the sheet"
        );
        assert!(!*LOGS_SHEET_OPEN.read(), "the sheet is closed");
    });
}
