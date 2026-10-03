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
    pause_logs, resume_logs, toggle_logs_paused, LOGS_BUFFER, LOGS_CONTAINER, LOGS_HELD,
    LOGS_PAUSED, SELECTED_POD,
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
