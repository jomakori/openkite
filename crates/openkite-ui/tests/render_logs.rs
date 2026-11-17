//! Model and render tests for the log dock.

use dioxus::prelude::*;
use openkite_ui::logs::{
    DockAction, Level, LogDock, LogPane, PodChoice, FRAME_BUDGET_BYTES, PANE_CAPACITY,
};
use openkite_ui::logs_view::Dock;

fn pod(name: &str) -> PodChoice {
    PodChoice::new(&format!("pod/prod/{name}"), name, "prod")
        .with_containers(&["main", "istio-proxy"])
}

fn pane() -> LogPane {
    LogPane::new(pod("api-7d9c4f8b6-2xqzt"))
}

fn lines(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("10:24:{:02}.001 INFO request {i} handled", i % 60))
        .collect()
}

/// Bytes of a stream, split into lines of roughly `width` characters.
fn stream(total_bytes: usize, width: usize) -> Vec<String> {
    let per = total_bytes / width.max(1);
    (0..per)
        .map(|i| {
            let head = format!("10:24:{:02}.001 ", i % 60);
            let pad = width.saturating_sub(head.len() + 2);
            format!("{head}{}", "x".repeat(pad))
        })
        .collect()
}

#[test]
fn levels_are_read_off_the_line_text() {
    assert_eq!(
        Level::parse("2026-11-17 ERROR failed to connect"),
        Level::Error
    );
    assert_eq!(Level::parse("WARN cache miss for key"), Level::Warn);
    assert_eq!(Level::parse("INFO listening on :8080"), Level::Info);
    assert_eq!(Level::parse("DEBUG retry 2 of 5"), Level::Debug);
    assert_eq!(Level::parse("TRACE span closed"), Level::Trace);
    assert_eq!(Level::parse("just a plain line"), Level::Info);
}

#[test]
fn a_leading_timestamp_is_split_off_and_kept_verbatim() {
    let mut p = pane();
    p.ingest(&["10:24:33.451 INFO server started".to_string()]);
    let line = p.lines.front().expect("line buffered");
    assert_eq!(line.stamp.as_deref(), Some("10:24:33.451"));
    assert_eq!(line.text, "INFO server started");
    assert_eq!(line.level, Level::Info);
}

#[test]
fn a_line_without_a_stamp_keeps_its_text_whole() {
    let mut p = pane();
    p.ingest(&["no timestamp here".to_string()]);
    let line = p.lines.front().expect("line buffered");
    assert_eq!(line.stamp, None);
    assert_eq!(line.text, "no timestamp here");
}

#[test]
fn pausing_stops_reads_outright() {
    let mut p = pane();
    assert!(p.wants(), "a fresh pane follows");
    assert!(p.ingest(&lines(3)) > 0);
    let before = p.bytes_in();

    p.paused = true;
    assert!(!p.wants());
    assert_eq!(p.ingest(&lines(50)), 0, "a paused pane accepts bytes");
    assert_eq!(
        p.bytes_in(),
        before,
        "a paused pane read {p0} bytes",
        p0 = p.bytes_in() - before
    );
    assert_eq!(p.lines.len(), 3, "a paused pane buffered lines");
}

#[test]
fn stopping_follow_also_stops_reads() {
    let mut p = pane();
    p.follow = false;
    assert!(!p.wants());
    assert_eq!(p.ingest(&lines(10)), 0);
    assert!(p.lines.is_empty());
}

#[test]
fn the_pump_never_calls_the_reader_for_a_paused_pane() {
    let mut dock = LogDock::default();
    dock.open_pod(pod("api"));
    dock.open_pod(pod("worker"));
    assert_eq!(dock.readers(), 1, "a temp pane is reused, not stacked");

    let mut asked = 0;
    dock.apply(DockAction::TogglePause);
    dock.pump(|_| {
        asked += 1;
        lines(2)
    });
    assert_eq!(asked, 0, "the reader ran for a paused pane");
    assert_eq!(dock.panes[0].bytes_in(), 0);

    dock.apply(DockAction::TogglePause);
    dock.pump(|_| {
        asked += 1;
        lines(2)
    });
    assert_eq!(asked, 1);
    assert!(dock.panes[0].bytes_in() > 0);
}

#[test]
fn five_megabytes_a_second_fits_inside_one_frame_budget() {
    // 5 MB/s at 60 fps is the ticket's bar.
    let per_frame = 5_000_000 / 60;
    assert!(
        per_frame < FRAME_BUDGET_BYTES,
        "budget {FRAME_BUDGET_BYTES} cannot hold a {per_frame}-byte frame"
    );

    let mut p = pane();
    let frame = stream(per_frame, 120);
    let taken = p.ingest(&frame);
    assert_eq!(
        taken,
        frame.iter().map(String::len).sum::<usize>(),
        "a whole frame was refused"
    );
    assert!(p.bytes_in() == taken as u64);

    // A whole second of that stream, frame by frame, is fully accounted for.
    let mut total = taken;
    for _ in 0..59 {
        assert_eq!(
            p.ingest(&frame),
            taken,
            "a frame of this size must always fit"
        );
        total += taken;
    }
    assert_eq!(total, taken * 60, "the second's bytes went missing");
}

#[test]
fn a_burst_past_the_budget_is_refused_rather_than_buffered() {
    let mut p = pane();
    let burst = stream(FRAME_BUDGET_BYTES * 2, 200);
    let taken = p.ingest(&burst);
    assert!(
        taken <= FRAME_BUDGET_BYTES,
        "took {taken} bytes in one frame"
    );
    let all: usize = burst.iter().map(String::len).sum();
    assert!(taken < all);
}

#[test]
fn the_ring_evicts_from_the_front_and_counts_it() {
    let mut p = pane();
    let many: Vec<String> = (0..PANE_CAPACITY + 250)
        .map(|i| format!("10:00:00.000 INFO line {i}"))
        .collect();
    for chunk in many.chunks(1_000) {
        p.ingest(chunk);
    }
    assert_eq!(p.lines.len(), PANE_CAPACITY);
    assert_eq!(p.dropped(), 250);
    assert!(p
        .lines
        .back()
        .unwrap()
        .text
        .contains(&format!("line {}", PANE_CAPACITY + 249)));
}

#[test]
fn the_search_filters_on_text_and_level() {
    let mut p = pane();
    p.ingest(&[
        "10:00:00.000 INFO started".to_string(),
        "10:00:01.000 ERROR boom".to_string(),
        "10:00:02.000 WARN slow".to_string(),
    ]);
    p.filter = "error".to_string();
    let visible: Vec<&str> = p.visible().map(|l| l.text.as_str()).collect();
    assert_eq!(visible, vec!["ERROR boom"]);

    p.filter = "WARN".to_string();
    assert_eq!(p.visible().count(), 1);
    p.filter = String::new();
    assert_eq!(p.visible().count(), 3);
    assert_eq!(p.problems(), 2, "warn and error both count as problems");
}

#[test]
fn switching_container_drops_the_old_lines() {
    let mut p = pane();
    p.ingest(&lines(5));
    p.set_container("istio-proxy");
    assert_eq!(p.container, "istio-proxy");
    assert!(p.lines.is_empty());
    assert_eq!(p.bytes_in(), 0);
}

#[test]
fn opening_a_second_pod_keeps_one_pane_and_retargets_it() {
    let mut dock = LogDock::default();
    dock.open_pod(pod("api"));
    dock.open_pod(pod("worker"));
    assert_eq!(dock.panes.len(), 1);
    assert_eq!(dock.panes[0].pod.name, "worker");
    assert!(dock.open);
}

#[test]
fn split_gives_two_panes_and_merge_returns_one() {
    let mut dock = LogDock::default();
    dock.set_choices(vec![pod("api"), pod("worker")]);
    dock.open_pod(pod("api"));
    assert!(dock.apply(DockAction::Split));
    assert!(dock.split());
    assert_eq!(dock.panes.len(), 2);
    assert_eq!(dock.focused, 1);
    assert!(
        !dock.apply(DockAction::Split),
        "a third pane would break the layout"
    );

    assert!(dock.apply(DockAction::Merge));
    assert_eq!(dock.panes.len(), 1);
    assert_eq!(dock.focused, 0);
}

#[test]
fn the_picker_retargets_the_focused_pane() {
    let mut dock = LogDock::default();
    let api = pod("api");
    let worker = pod("worker");
    dock.set_choices(vec![api.clone(), worker.clone()]);
    dock.open_pod(api);
    assert!(dock.apply(DockAction::PickPod(worker.id.clone())));
    assert_eq!(dock.panes[0].pod.name, "worker");
    assert!(
        !dock.apply(DockAction::PickPod("pod/missing".to_string())),
        "unknown pod accepted"
    );
    assert_eq!(dock.panes[0].pod.name, "worker");
}

#[test]
fn focus_moves_between_panes_and_is_refused_out_of_range() {
    let mut dock = LogDock::default();
    dock.open_pod(pod("api"));
    dock.apply(DockAction::Split);
    assert!(dock.apply(DockAction::Focus(0)));
    assert_eq!(dock.focused, 0);
    assert!(!dock.apply(DockAction::Focus(9)));
    assert_eq!(dock.focused, 0);
}

#[test]
fn the_toggle_opens_and_closes_the_dock() {
    let mut dock = LogDock::default();
    assert!(!dock.open);
    dock.apply(DockAction::Toggle);
    assert!(dock.open);
    dock.apply(DockAction::Close);
    assert!(!dock.open);
}

// ---- render

#[derive(Props, Clone, PartialEq)]
struct HarnessProps {
    dock: LogDock,
}

#[allow(non_snake_case)]
fn Harness(props: HarnessProps) -> Element {
    let on_action = use_callback(|_: DockAction| {});
    rsx! { Dock { dock: props.dock.clone(), on_action } }
}

fn render(dock: LogDock) -> String {
    let mut dom = VirtualDom::new_with_props(Harness, HarnessProps { dock });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn running_dock() -> LogDock {
    let mut dock = LogDock::default();
    let mut p = LogPane::new(pod("api-7d9c4f8b6-2xqzt"));
    p.ingest(&[
        "10:24:31.001 INFO server started on :8080".to_string(),
        "10:24:33.451 WARN upstream slow (412ms)".to_string(),
        "10:24:35.220 ERROR dial tcp 10.0.3.7:5432: connect: refused".to_string(),
        "10:24:36.010 DEBUG retry 1 of 5".to_string(),
        "10:24:37.900 TRACE span closed".to_string(),
    ]);
    dock.panes.push(p);
    dock.open = true;
    dock
}

#[test]
fn a_collapsed_dock_renders_its_bar_only() {
    let html = render(LogDock::default());
    assert!(html.contains("data-act=\"toggle\""), "{html}");
    assert!(!html.contains("class=\"dock open\""), "{html}");
    assert!(!html.contains("log-body"), "{html}");
}

#[test]
fn the_bar_carries_the_picker_and_the_split_control() {
    let mut dock = running_dock();
    dock.set_choices(vec![
        pod("api-7d9c4f8b6-2xqzt"),
        pod("worker-6b8f7d5c4-zz2lt"),
    ]);
    let html = render(dock);
    assert!(html.contains("data-pick=\"pod\""), "{html}");
    assert!(html.contains("prod/worker-6b8f7d5c4-zz2lt"), "{html}");
    assert!(html.contains("data-act=\"split\""), "{html}");
    assert!(html.contains("data-act=\"close\""), "{html}");
}

#[test]
fn every_level_gets_its_own_class_and_column() {
    let html = render(running_dock());
    for level in ["info", "warn", "error", "debug", "trace"] {
        assert!(
            html.contains(&format!("log-level {level}")),
            "missing level {level}: {html}"
        );
    }
    assert!(
        html.contains("log-msg error"),
        "error text not highlighted: {html}"
    );
    assert!(html.contains("log-time"), "timestamps missing: {html}");
    assert!(html.contains("data-lvl=\"error\""), "{html}");
}

#[test]
fn turning_timestamps_off_drops_the_column_but_not_the_text() {
    let mut dock = running_dock();
    dock.panes[0].timestamps = false;
    let html = render(dock);
    assert!(!html.contains("log-time"), "{html}");
    assert!(html.contains("server started on :8080"), "{html}");
}

#[test]
fn a_paused_pane_says_so_and_shows_resume() {
    let mut dock = running_dock();
    dock.panes[0].paused = true;
    let html = render(dock);
    assert!(html.contains("data-paused=\"true\""), "{html}");
    assert!(html.contains(">Resume<"), "{html}");
    assert!(html.contains("log-body paused"), "{html}");
    assert!(html.contains("not read while paused"), "{html}");
}

#[test]
fn the_split_renders_both_panes_with_the_focused_one_marked() {
    let mut dock = running_dock();
    dock.apply(DockAction::Split);
    let html = render(dock);
    assert_eq!(html.matches("class=\"dpane log").count(), 2, "{html}");
    assert!(html.contains("dpane log focused"), "{html}");
    assert!(html.contains("dock-body split"), "{html}");
}

#[test]
fn an_empty_pane_explains_itself() {
    let mut dock = LogDock::default();
    dock.open_pod(pod("idle"));
    let html = render(dock);
    assert!(html.contains("No log lines yet."), "{html}");
}

#[test]
fn a_search_with_no_hits_says_so_instead_of_showing_a_blank_pane() {
    let mut dock = running_dock();
    dock.panes[0].filter = "needle".to_string();
    let html = render(dock);
    assert!(html.contains("No lines match the search."), "{html}");
    assert!(html.contains("0 of 5 lines match"), "{html}");
}
