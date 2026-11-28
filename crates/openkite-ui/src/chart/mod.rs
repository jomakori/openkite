//! Metric charts: the sparkline a list row carries, the per-workload chart panel, and
//! the states a chart is drawn in when there is nothing to plot.
//!
//! Model lives in [`crate::metrics`]. Chart chrome (`.chart-*`, `.spark`, `.legend`) is
//! the vendored reference sheet's, reused as-is; only the rules the prototype does not
//! have - threshold marks, the paused state and the empty state - live in `chart.css`
//! beside this file, never in the shared stylesheet.
//!
//! A hidden chart draws no series. The surface tells the chart whether it is on screen,
//! and stops feeding it at the same time, so a folded panel costs nothing.

use dioxus::prelude::*;

use crate::metrics::{MetricSource, Series, Threshold, Tone};

/// Stylesheet for the states this module adds. Served next to `console.css`.
pub const CHART_CSS: &str = include_str!("chart.css");

/// Chart plot box inside the 320x124 viewBox: left gutter for the value axis, bottom
/// gutter for the ticks, 8 px padding on the other two sides.
const PLOT_LEFT: f64 = 40.0;
const PLOT_TOP: f64 = 8.0;
const PLOT_W: f64 = 272.0;
const PLOT_H: f64 = 98.0;
const VIEW_W: f64 = 320.0;
const VIEW_H: f64 = 124.0;

/// Colour a reading takes for its band. Marks use semantic tokens, never raw hex.
fn tone_color(tone: Tone) -> &'static str {
    match tone {
        Tone::Normal => "var(--accent)",
        Tone::Warn => "var(--warn)",
        Tone::Crit => "var(--danger)",
    }
}

/// One plotted series: the legend name, the token it draws in, and its samples.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WorkloadSeries {
    pub name: String,
    pub color: String,
    pub data: Series,
}

impl WorkloadSeries {
    pub fn new(name: impl Into<String>, color: impl Into<String>, data: Series) -> Self {
        WorkloadSeries {
            name: name.into(),
            color: color.into(),
            data,
        }
    }

    /// Most recent value, or 0 when the series has no samples.
    pub fn last(&self) -> u8 {
        self.data.last()
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct SparklineProps {
    /// The window to draw. Empty renders the dash an absent source leaves behind.
    #[props(default)]
    pub series: Series,
    /// Bands the latest sample is judged against.
    #[props(default)]
    pub threshold: Threshold,
    /// Accessible name for the line, e.g. "CPU trend".
    #[props(default)]
    pub label: String,
}

/// A list cell's trend: the shape of the last minute plus the current reading. Rows
/// whose reading is over the threshold carry `data-tone="warn"|"crit"` so the cell is
/// marked without the caller deciding what counts as high.
#[component]
pub fn Sparkline(props: SparklineProps) -> Element {
    let tone = props.series.tone(props.threshold);
    let color = tone_color(tone);
    let aria = if props.label.is_empty() {
        format!("Trend, now {} percent", props.series.last())
    } else {
        format!("{}, now {} percent", props.label, props.series.last())
    };
    rsx! {
        span { class: "trend-cell", "data-tone": tone.id(),
            match props.series.geometry(72.0, 22.0) {
                Some(spark) => rsx! {
                    svg {
                        class: "spark",
                        view_box: "0 0 72 22",
                        role: "img",
                        "aria-label": "{aria}",
                        path { class: "area", d: "{spark.area}", fill: "{color}" }
                        path { class: "line", d: "{spark.line}", stroke: "{color}" }
                    }
                    span { class: "spark-val", style: "color: {color}", "{props.series.last()}%" }
                },
                None => rsx! {
                    span { class: "spark-val spark-empty", "data-empty": "true", "—" }
                },
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct WorkloadChartProps {
    /// What the chart plots, e.g. "CPU" or "Memory".
    pub title: String,
    /// Right-hand caption: the unit, the range, or why the chart is idle.
    #[props(default)]
    pub sub: String,
    /// Series to plot; one for CPU or memory, two for network in and out.
    pub series: Vec<WorkloadSeries>,
    /// Where the samples came from - shown as a chip so an add-on the cluster lacks is
    /// visible rather than silently flat.
    #[props(default)]
    pub source: MetricSource,
    /// Band the marked line draws at, and the tone the latest value takes.
    #[props(default)]
    pub threshold: Threshold,
    /// Whether the surface holding this chart is on screen. A hidden chart draws no
    /// series and says so; the caller stops feeding it.
    #[props(default = true)]
    pub visible: bool,
}

impl WorkloadChartProps {
    /// Whether any series has a sample to plot.
    fn has_samples(&self) -> bool {
        self.series.iter().any(|line| !line.data.is_empty())
    }

    /// Latest reading across series, and the tone it takes.
    fn latest(&self) -> Option<(u8, Tone)> {
        self.series
            .iter()
            .map(|line| line.last())
            .max()
            .map(|pct| (pct, self.threshold.tone(pct)))
    }
}

/// A per-workload chart: the plot box, its ticks, the threshold line, the legend and
/// the states it falls back to. Used for a workload's own usage and for the aggregate
/// a summary panel shows.
#[component]
pub fn WorkloadChart(props: WorkloadChartProps) -> Element {
    let state = if !props.visible {
        "paused"
    } else if !props.source.is_available() {
        "unavailable"
    } else if !props.has_samples() {
        "empty"
    } else {
        "ready"
    };
    let tick = props.threshold.warn;
    let now = props.latest();
    let caption = if props.sub.is_empty() {
        format!("last {} samples", props.series.first().map(|s| s.data.len()).unwrap_or(0))
    } else {
        props.sub.clone()
    };

    rsx! {
        figure {
            class: "chart-card",
            "data-od-id": "workload-chart",
            "data-state": state,
            "data-visible": if props.visible { "true" } else { "false" },
            "data-source": props.source.id(),
            "data-threshold": "{tick}",
            figcaption {
                span { class: "ct", "{props.title}" }
                span { class: "cs",
                    if let Some((pct, tone)) = now {
                        span {
                            class: "chart-now",
                            "data-tone": tone.id(),
                            "data-now": "{pct}",
                            style: "color: {tone_color(tone)}",
                            "{pct}%"
                        }
                    }
                    span { class: "chart-src", "data-source": props.source.id(), "{props.source.label()}" }
                    span { class: "chart-sub", "{caption}" }
                }
            }
            match state {
                "paused" => rsx! {
                    div { class: "chart-paused", role: "status", "data-paused": "true",
                        "Paused — this surface is hidden"
                    }
                },
                "unavailable" => rsx! {
                    div { class: "chart-empty", role: "status", "data-empty": "source",
                        span { class: "chart-empty-title", "No metrics source" }
                        span { class: "chart-empty-hint", "{props.source.empty_state()}" }
                    }
                },
                "empty" => rsx! {
                    div { class: "chart-empty", role: "status", "data-empty": "samples",
                        span { class: "chart-empty-title", "No samples yet" }
                        span { class: "chart-empty-hint", "Waiting for the first scrape" }
                    }
                },
                _ => rsx! {
                    ChartPlot { series: props.series.clone(), threshold: props.threshold }
                    div { class: "legend",
                        for line in props.series.iter() {
                            span { class: "lg",
                                i { style: "background: {line.color}" }
                                "{line.name}"
                                b { "{line.last()}%" }
                            }
                        }
                        span { class: "lg", "data-lg": "threshold",
                            i { class: "dash" }
                            "threshold {tick}%"
                        }
                    }
                },
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct ChartPlotProps {
    series: Vec<WorkloadSeries>,
    threshold: Threshold,
}

/// The plotted box: grid, ticks, one path pair per series, and the marked threshold.
#[component]
fn ChartPlot(props: ChartPlotProps) -> Element {
    let line_y = props.threshold.y(PLOT_H);
    let mut ticks: Vec<u8> = vec![0, 25, 50, 75, 100];
    if !ticks.contains(&props.threshold.warn) {
        ticks.push(props.threshold.warn);
        ticks.sort_unstable();
    }
    rsx! {
        div { class: "chart-wrap",
            svg {
                class: "chart-svg",
                view_box: "0 0 {VIEW_W} {VIEW_H}",
                role: "img",
                "aria-label": "Usage over the last minute",
                for pct in ticks.iter().copied() {
                    line {
                        class: "grid",
                        x1: "{PLOT_LEFT}",
                        x2: "{PLOT_LEFT + PLOT_W}",
                        y1: "{PLOT_TOP + PLOT_H - (pct as f64 / 100.0) * PLOT_H:.1}",
                        y2: "{PLOT_TOP + PLOT_H - (pct as f64 / 100.0) * PLOT_H:.1}",
                    }
                    text {
                        class: "axis",
                        x: "{PLOT_LEFT - 6.0}",
                        y: "{PLOT_TOP + PLOT_H - (pct as f64 / 100.0) * PLOT_H + 3.0:.1}",
                        text_anchor: "end",
                        "{pct}%"
                    }
                }
                g { transform: "translate({PLOT_LEFT},{PLOT_TOP})",
                    line {
                        class: "ref req",
                        "data-ref": "threshold",
                        x1: "0",
                        x2: "{PLOT_W}",
                        y1: "{line_y:.1}",
                        y2: "{line_y:.1}",
                    }
                    text {
                        class: "axis",
                        "data-ref-label": "threshold",
                        x: "{PLOT_W}",
                        y: "{line_y - 3.0:.1}",
                        text_anchor: "end",
                        "warn {props.threshold.warn}%"
                    }
                    for line in props.series.iter() {
                        if let Some(spark) = line.data.geometry_over(PLOT_W, PLOT_H, 100.0) {
                            path {
                                class: "area",
                                "data-series": "{line.name}",
                                d: "{spark.area}",
                                fill: "{line.color}",
                            }
                            path {
                                class: "line",
                                "data-series": "{line.name}",
                                d: "{spark.line}",
                                stroke: "{line.color}",
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(values: &[u8]) -> Series {
        Series::with_samples(values.iter().copied())
    }

    #[test]
    fn a_marked_reading_takes_the_band_colour() {
        assert_eq!(tone_color(Tone::Normal), "var(--accent)");
        assert_eq!(tone_color(Tone::Warn), "var(--warn)");
        assert_eq!(tone_color(Tone::Crit), "var(--danger)");
    }

    #[test]
    fn the_chart_reports_its_latest_reading_and_tone() {
        let props = WorkloadChartProps {
            title: "CPU".into(),
            sub: String::new(),
            series: vec![
                WorkloadSeries::new("usage", "var(--accent)", series(&[10, 92])),
                WorkloadSeries::new("requests", "var(--muted)", series(&[40, 41])),
            ],
            source: MetricSource::MetricsServer,
            threshold: Threshold::default(),
            visible: true,
        };
        assert_eq!(props.latest(), Some((92, Tone::Crit)));
        assert!(props.has_samples());
    }

    #[test]
    fn chart_geometry_plots_against_a_fixed_hundred_so_the_mark_lines_up() {
        let spark = series(&[50])
            .geometry_over(PLOT_W, PLOT_H, 100.0)
            .expect("sample renders");
        // 50% sits at half the plot box, not at the window peak.
        assert!(spark.line.contains("49.0"), "{spark:?}");
        // The default warn line at 70% is above it.
        assert!(Threshold::default().y(PLOT_H) < 49.0);
    }
}
