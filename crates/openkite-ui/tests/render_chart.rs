//! Rendered chart markup: the row sparkline, the workload chart, the threshold mark,
//! and the states a chart falls back to.

use dioxus::prelude::*;
use openkite_ui::chart::{Sparkline, WorkloadChart, WorkloadSeries};
use openkite_ui::metrics::{MetricSource, Series, Threshold};

fn series(values: impl IntoIterator<Item = u8>) -> Series {
    Series::with_samples(values)
}

fn render_hidden(visible: bool, source: MetricSource, values: Vec<u8>) -> String {
    let mut dom = VirtualDom::new_with_props(
        ChartHarness,
        ChartHarnessProps {
            visible,
            source,
            values,
        },
    );
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[derive(Props, Clone, PartialEq)]
struct ChartHarnessProps {
    visible: bool,
    source: MetricSource,
    values: Vec<u8>,
}

#[component]
fn ChartHarness(props: ChartHarnessProps) -> Element {
    rsx! {
        WorkloadChart {
            title: "CPU",
            series: vec![WorkloadSeries::new("api", "var(--accent)", series(props.values))],
            source: props.source,
            threshold: Threshold::default(),
            visible: props.visible,
        }
    }
}

fn render_spark(values: Vec<u8>) -> String {
    let mut dom = VirtualDom::new_with_props(SparkHarness, SparkHarnessProps { values });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[derive(Props, Clone, PartialEq)]
struct SparkHarnessProps {
    values: Vec<u8>,
}

#[component]
fn SparkHarness(props: SparkHarnessProps) -> Element {
    rsx! {
        Sparkline {
            series: series(props.values),
            threshold: Threshold::default(),
            label: "CPU trend",
        }
    }
}

#[test]
fn a_row_sparkline_draws_its_window_and_marks_a_high_reading() {
    let html = render_spark(vec![40, 62, 91]);
    assert!(html.contains("class=\"trend-cell\""), "{html}");
    assert!(html.contains("class=\"spark\""), "{html}");
    assert!(html.contains("data-tone=\"crit\""), "{html}");
    assert!(html.contains("91%"), "{html}");
    // The line is a real path, not a placeholder.
    assert!(html.contains("class=\"line\""), "{html}");
    assert!(
        html.contains(" d=\"M") || html.contains(" d=\"0.0"),
        "{html}"
    );
}

#[test]
fn a_row_sparkline_without_readings_shows_a_dash_not_a_flat_line() {
    let html = render_spark(Vec::new());
    assert!(html.contains("data-empty=\"true\""), "{html}");
    assert!(!html.contains("class=\"spark\""), "{html}");
}

#[test]
fn a_workload_chart_marks_the_threshold_it_is_judged_by() {
    let html = render_hidden(true, MetricSource::MetricsServer, vec![24, 51, 78]);
    assert!(html.contains("data-state=\"ready\""), "{html}");
    assert!(html.contains("data-source=\"metrics-server\""), "{html}");
    assert!(html.contains("data-threshold=\"70\""), "{html}");
    assert!(html.contains("data-lg=\"threshold\""), "{html}");
    // The latest reading is over the warn band, so the caption carries the mark.
    assert!(html.contains("data-tone=\"warn\""), "{html}");
}

#[test]
fn an_absent_metrics_source_renders_its_empty_state() {
    let html = render_hidden(true, MetricSource::Absent, Vec::new());
    assert!(html.contains("data-state=\"unavailable\""), "{html}");
    assert!(html.contains("data-empty=\"source\""), "{html}");
    assert!(!html.contains("class=\"line\""), "{html}");
}

#[test]
fn a_hidden_chart_pauses_instead_of_plotting() {
    let html = render_hidden(false, MetricSource::MetricsServer, vec![24, 51, 78]);
    assert!(html.contains("data-state=\"paused\""), "{html}");
    assert!(html.contains("data-paused=\"true\""), "{html}");
    assert!(!html.contains("class=\"line\""), "{html}");
}
