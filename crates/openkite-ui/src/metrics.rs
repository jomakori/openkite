//! Metrics model: where samples come from, the bounded history a chart draws, the
//! pause rule for hidden surfaces, and the threshold a reading is judged against.
//!
//! Nothing here queries. The caller decides which source answered and feeds samples
//! in; a surface that is not on screen never asks for one, which is what lets a
//! hidden chart cost nothing.

/// Samples a series keeps. One minute at a 5 s scrape fills this; older samples age out.
pub const SAMPLE_CAP: usize = 60;

/// Where live usage samples come from. Detection is a by-product of lists the console
/// already fetched, so an absent add-on costs no extra round trip.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MetricSource {
    /// `metrics.k8s.io` answers: CPU and memory for nodes and pods.
    MetricsServer,
    /// No metrics API, but Prometheus is scraping: the series are a query away.
    Prometheus,
    /// Neither answered. Charts render their empty state instead of a flat line.
    #[default]
    Absent,
}

impl MetricSource {
    /// Autodetect from what the cluster already told us: a running metrics-server pod
    /// wins, then a running Prometheus pod, else absent.
    pub fn detect(probe: &SourceProbe) -> Self {
        if probe.metrics_server.is_some() {
            MetricSource::MetricsServer
        } else if probe.prometheus.is_some() {
            MetricSource::Prometheus
        } else {
            MetricSource::Absent
        }
    }

    /// Whether a series can carry a real reading.
    pub fn is_available(self) -> bool {
        !matches!(self, MetricSource::Absent)
    }

    /// Stable key used in markup and in the source chip.
    pub fn id(self) -> &'static str {
        match self {
            MetricSource::MetricsServer => "metrics-server",
            MetricSource::Prometheus => "prometheus",
            MetricSource::Absent => "absent",
        }
    }

    /// What the source chip says.
    pub fn label(self) -> &'static str {
        match self {
            MetricSource::MetricsServer => "metrics-server",
            MetricSource::Prometheus => "Prometheus",
            MetricSource::Absent => "No metrics",
        }
    }

    /// The empty state an absent source renders: what is missing and what fixes it.
    pub fn empty_state(self) -> &'static str {
        match self {
            MetricSource::MetricsServer => "No samples yet",
            MetricSource::Prometheus => "Prometheus has not answered yet",
            MetricSource::Absent => {
                "Metrics unavailable — install metrics-server or point the console at Prometheus"
            }
        }
    }
}

/// The metrics a surface draws with: which source answered, the band it judges by, and
/// the window of aggregate samples for each metric. This is what the overview board
/// carries so its charts need no second fetch.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct MetricsView {
    pub source: MetricSource,
    pub threshold: Threshold,
    pub cpu: Series,
    pub mem: Series,
}

impl MetricsView {
    /// The window for a metric key (`"cpu"` or `"mem"`), if the key is known.
    pub fn series(&self, metric: &str) -> Option<&Series> {
        match metric {
            "cpu" => Some(&self.cpu),
            "mem" => Some(&self.mem),
            _ => None,
        }
    }

    /// Offers one reading to a metric's window; unknown keys are dropped.
    pub fn offer(&mut self, metric: &str, pct: u8) {
        match metric {
            "cpu" => self.cpu.push(pct),
            "mem" => self.mem.push(pct),
            _ => {}
        }
    }

    /// Tone of a metric's latest reading.
    pub fn tone(&self, metric: &str) -> Tone {
        self.series(metric)
            .map(|series| series.tone(self.threshold))
            .unwrap_or_default()
    }
}

/// Pods whose presence means a source is live, named as `<namespace>/<pod>`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SourceProbe {
    pub metrics_server: Option<String>,
    pub prometheus: Option<String>,
}

impl SourceProbe {
    /// Records the first running pod of each add-on from a list the caller already has.
    /// `namespace` and `name` are compared lowercased; a stopped pod does not count.
    pub fn observe(&mut self, namespace: &str, name: &str, running: bool) {
        if !running {
            return;
        }
        let pod = format!("{namespace}/{name}");
        let lower = name.to_ascii_lowercase();
        if self.metrics_server.is_none() && lower.starts_with("metrics-server") {
            self.metrics_server = Some(pod);
        } else if self.prometheus.is_none() && lower.starts_with("prometheus") {
            self.prometheus = Some(pod);
        }
    }

    /// The source these observations add up to.
    pub fn detect(&self) -> MetricSource {
        MetricSource::detect(self)
    }
}

/// A bounded run of percentage samples, oldest first. Values are already rounded to
/// whole percent, which is the precision every surface shows.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Series {
    samples: Vec<u8>,
    cap: usize,
}

impl Default for Series {
    fn default() -> Self {
        Series::new(SAMPLE_CAP)
    }
}

impl Series {
    /// A series that keeps at most `cap` samples (0 is treated as 1).
    pub fn new(cap: usize) -> Self {
        Series {
            samples: Vec::new(),
            cap: cap.max(1),
        }
    }

    /// A series seeded with history, oldest first; anything beyond the cap is dropped.
    pub fn with_samples(samples: impl IntoIterator<Item = u8>) -> Self {
        let mut series = Series::default();
        for sample in samples {
            series.push(sample);
        }
        series
    }

    /// Appends one sample, evicting the oldest once the cap is reached.
    pub fn push(&mut self, sample: u8) {
        if self.samples.len() == self.cap {
            self.samples.remove(0);
        }
        self.samples.push(sample.min(100));
    }

    pub fn samples(&self) -> &[u8] {
        &self.samples
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Most recent sample, or 0 when there is none.
    pub fn last(&self) -> u8 {
        self.samples.last().copied().unwrap_or(0)
    }

    /// Largest sample in the window, or 0 when there is none.
    pub fn peak(&self) -> u8 {
        self.samples.iter().copied().max().unwrap_or(0)
    }

    /// Tone of the latest sample against `threshold`.
    pub fn tone(&self, threshold: Threshold) -> Tone {
        threshold.tone(self.last())
    }

    /// Path data for a sparkline in a `w` x `h` viewBox, or `None` when there is
    /// nothing to draw. Geometry matches the reference sheet: a 2 px inset and a
    /// 10% headroom over the window peak so the line never touches the top edge.
    pub fn geometry(&self, w: f64, h: f64) -> Option<Spark> {
        let top = (self.peak() as f64) * 1.1;
        Some(self.geometry_over(w, h, if top <= 0.0 { 1.0 } else { top })?)
    }

    /// Path data plotted against a fixed scale top (0-100 for a chart whose threshold
    /// line has to sit at its own percentage rather than at the window peak).
    pub fn geometry_over(&self, w: f64, h: f64, top: f64) -> Option<Spark> {
        let last = self.samples.last()?;
        let top = if top <= 0.0 { 1.0 } else { top };
        let y = |value: u8| -> f64 { h - 2.0 - (value as f64) / top * (h - 4.0) };
        let points: Vec<String> = if self.samples.len() == 1 {
            // One sample has no slope: draw it flat across the box rather than divide by zero.
            let yy = y(*last);
            vec![format!("0.0 {yy:.1}"), format!("{w:.1} {yy:.1}")]
        } else {
            let step = w / (self.samples.len() - 1) as f64;
            self.samples
                .iter()
                .enumerate()
                .map(|(i, value)| format!("{:.1} {:.1}", i as f64 * step, y(*value)))
                .collect()
        };
        let line = format!("M{}", points.join(" L"));
        let area = format!("{line} L{w:.1} {h:.1} L0.0 {h:.1} Z");
        Some(Spark { line, area })
    }
}

/// A sparkline's two path `d` attributes: the stroked line and the filled area under it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Spark {
    pub line: String,
    pub area: String,
}

/// A chart's live buffer. It takes a sample only while its surface is visible, so a
/// folded panel or an off-screen list stops accumulating; `skipped` is the count of
/// samples offered while hidden, which is what the pause behaviour is asserted on.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChartFeed {
    series: Series,
    visible: bool,
    skipped: usize,
}

impl Default for ChartFeed {
    fn default() -> Self {
        ChartFeed::new(SAMPLE_CAP)
    }
}

impl ChartFeed {
    pub fn new(cap: usize) -> Self {
        ChartFeed {
            series: Series::new(cap),
            visible: true,
            skipped: 0,
        }
    }

    /// Show or hide the surface the feed belongs to. Hiding keeps the history, so the
    /// chart comes back with the window it had rather than an empty box.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Offers one sample. Returns whether it was taken; a hidden feed drops it and
    /// counts the drop.
    pub fn offer(&mut self, sample: u8) -> bool {
        if !self.visible {
            self.skipped += 1;
            return false;
        }
        self.series.push(sample);
        true
    }

    pub fn series(&self) -> &Series {
        &self.series
    }

    /// Samples dropped because the surface was hidden.
    pub fn skipped(&self) -> usize {
        self.skipped
    }
}

/// The band a reading is judged against: `warn` from 70, `crit` from 85, matching the
/// reference gauges. A threshold also draws the marked line on a chart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Threshold {
    pub warn: u8,
    pub crit: u8,
}

impl Default for Threshold {
    fn default() -> Self {
        Threshold { warn: 70, crit: 85 }
    }
}

impl Threshold {
    pub fn new(warn: u8, crit: u8) -> Self {
        let warn = warn.min(100);
        Threshold {
            warn,
            crit: crit.max(warn).min(100),
        }
    }

    /// Which band `pct` falls in.
    pub fn tone(self, pct: u8) -> Tone {
        if pct >= self.crit {
            Tone::Crit
        } else if pct >= self.warn {
            Tone::Warn
        } else {
            Tone::Normal
        }
    }

    /// Whether a reading is marked at all - what gates the highlight on a row.
    pub fn marks(self, pct: u8) -> bool {
        !matches!(self.tone(pct), Tone::Normal)
    }

    /// Height of the threshold line in a `h`-tall chart that plots 0-100.
    pub fn y(self, h: f64) -> f64 {
        h - (self.warn as f64 / 100.0) * h
    }
}

/// Severity of a reading against a [`Threshold`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tone {
    #[default]
    Normal,
    Warn,
    Crit,
}

impl Tone {
    /// `data-tone` value; the stylesheet colours the mark from it.
    pub fn id(self) -> &'static str {
        match self {
            Tone::Normal => "normal",
            Tone::Warn => "warn",
            Tone::Crit => "crit",
        }
    }

    pub fn marked(self) -> bool {
        !matches!(self, Tone::Normal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_metrics_server_wins_over_prometheus() {
        let mut probe = SourceProbe::default();
        probe.observe("kube-system", "metrics-server-7c9f8b6d5-qj4vt", true);
        probe.observe("monitoring", "prometheus-server-0", true);
        assert_eq!(probe.detect(), MetricSource::MetricsServer);
        assert_eq!(probe.detect().id(), "metrics-server");
    }

    #[test]
    fn a_stopped_add_on_does_not_count_and_no_add_on_reads_absent() {
        let mut probe = SourceProbe::default();
        probe.observe("kube-system", "metrics-server-0", false);
        assert_eq!(probe.detect(), MetricSource::Absent);
        assert!(!probe.detect().is_available());
        assert!(probe.detect().empty_state().contains("metrics-server"));

        probe.observe("monitoring", "prometheus-k8s-0", true);
        assert_eq!(probe.detect(), MetricSource::Prometheus);
        assert!(probe.detect().is_available());
    }

    #[test]
    fn series_evicts_the_oldest_sample_at_the_cap() {
        let mut series = Series::new(3);
        for sample in [10, 20, 30, 40] {
            series.push(sample);
        }
        assert_eq!(series.samples(), &[20, 30, 40]);
        assert_eq!(series.last(), 40);
        assert_eq!(series.peak(), 40);
    }

    #[test]
    fn sparkline_geometry_stays_inside_the_box_and_clears_the_top() {
        let series = Series::with_samples([0, 50, 100, 25]);
        let spark = series.geometry(72.0, 22.0).expect("samples render");
        assert!(spark.line.starts_with("M0.0 "));
        assert!(spark.area.ends_with("L0.0 22.0 Z"));
        // 100% on a 10%-headroom scale never reaches the 2px inset.
        assert!(spark.line.contains("2.0"), "peak sits at the inset: {spark:?}");
    }

    #[test]
    fn a_single_sample_still_draws_a_line() {
        let spark = Series::with_samples([42])
            .geometry(72.0, 22.0)
            .expect("one sample renders");
        assert!(spark.line.contains("72.0"), "flat line spans the box");
        assert!(!spark.line.contains("NaN"));
    }

    #[test]
    fn an_empty_series_has_no_geometry() {
        assert!(Series::default().geometry(72.0, 22.0).is_none());
    }

    #[test]
    fn a_hidden_chart_takes_no_samples_and_says_how_many_it_dropped() {
        let mut feed = ChartFeed::default();
        assert!(feed.offer(40));
        feed.set_visible(false);
        for _ in 0..5 {
            assert!(!feed.offer(90));
        }
        assert_eq!(feed.series().len(), 1);
        assert_eq!(feed.skipped(), 5);

        feed.set_visible(true);
        assert!(feed.offer(55));
        assert_eq!(feed.series().samples(), &[40, 55]);
        assert_eq!(feed.skipped(), 5);
    }

    #[test]
    fn a_view_feeds_each_metric_its_own_window() {
        let mut view = MetricsView::default();
        assert_eq!(view.source, MetricSource::Absent);
        view.offer("cpu", 30);
        view.offer("cpu", 88);
        view.offer("mem", 12);
        view.offer("gpu", 99);
        assert_eq!(view.series("cpu").unwrap().samples(), &[30, 88]);
        assert_eq!(view.series("mem").unwrap().samples(), &[12]);
        assert_eq!(view.tone("cpu"), Tone::Crit);
        assert_eq!(view.tone("mem"), Tone::Normal);
        assert!(view.series("gpu").is_none());
    }

    #[test]
    fn threshold_bands_and_the_marked_line() {
        let threshold = Threshold::default();
        assert_eq!(threshold.tone(69), Tone::Normal);
        assert_eq!(threshold.tone(70), Tone::Warn);
        assert_eq!(threshold.tone(85), Tone::Crit);
        assert!(!threshold.marks(12));
        assert!(threshold.marks(86));
        assert_eq!(threshold.tone(90).id(), "crit");
        assert!((threshold.y(100.0) - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_threshold_cannot_invert_its_bands() {
        let threshold = Threshold::new(90, 40);
        assert_eq!(threshold.warn, 90);
        assert_eq!(threshold.crit, 90);
    }
}
