//! Overview board model: which widgets exist, how the layout is stored, and what
//! data each visible widget asks for.
//!
//! The board never queries on its own. It reports the interests its visible
//! widgets need, so hiding a widget stops the caller fetching for it.

use std::collections::BTreeMap;

use crate::table::Row;

/// The nine board widgets, in their default order.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum WidgetId {
    Attention,
    Usage,
    Problems,
    Restarts,
    Capacity,
    Nodes,
    Health,
    Argo,
    Events,
}

impl WidgetId {
    /// Default order, matching the reference board.
    pub const ALL: [WidgetId; 9] = [
        WidgetId::Attention,
        WidgetId::Usage,
        WidgetId::Problems,
        WidgetId::Restarts,
        WidgetId::Capacity,
        WidgetId::Nodes,
        WidgetId::Health,
        WidgetId::Argo,
        WidgetId::Events,
    ];

    /// Stable key used in markup and in the saved layout.
    pub fn id(self) -> &'static str {
        match self {
            WidgetId::Attention => "attention",
            WidgetId::Usage => "usage",
            WidgetId::Problems => "problems",
            WidgetId::Restarts => "restarts",
            WidgetId::Capacity => "capacity",
            WidgetId::Nodes => "nodes",
            WidgetId::Health => "health",
            WidgetId::Argo => "argo",
            WidgetId::Events => "events",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        WidgetId::ALL.iter().copied().find(|w| w.id() == id)
    }

    pub fn title(self) -> &'static str {
        match self {
            WidgetId::Attention => "Needs attention",
            WidgetId::Usage => "Cluster usage",
            WidgetId::Problems => "Pending & failing pods",
            WidgetId::Restarts => "High pod restarts",
            WidgetId::Capacity => "Capacity",
            WidgetId::Nodes => "Nodes",
            WidgetId::Health => "Workload health",
            WidgetId::Argo => "Argo CD sync",
            WidgetId::Events => "Recent events",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            WidgetId::Attention => "Failing pods, degraded workloads and warnings at a glance.",
            WidgetId::Usage => "CPU and memory across the selected namespaces.",
            WidgetId::Problems => "Pods that are not running, with restart counts.",
            WidgetId::Restarts => "Pods that restart most often. Set your own threshold.",
            WidgetId::Capacity => "Requests against allocatable CPU, memory, pods and storage.",
            WidgetId::Nodes => "Per-node CPU and memory utilisation.",
            WidgetId::Health => "Pods by status and the busiest namespaces.",
            WidgetId::Argo => "Applications that are out of sync or unhealthy.",
            WidgetId::Events => "Latest cluster events with a warnings filter.",
        }
    }

    /// Width a widget takes when the saved layout has no opinion.
    pub fn default_size(self) -> Size {
        match self {
            WidgetId::Attention | WidgetId::Usage | WidgetId::Events => Size::Full,
            _ => Size::Half,
        }
    }

    /// Row data this widget reads.
    pub fn interests(self) -> &'static [Interest] {
        match self {
            WidgetId::Attention => &[
                Interest::Pods,
                Interest::Deployments,
                Interest::Nodes,
                Interest::ArgoApps,
                Interest::Events,
            ],
            WidgetId::Usage | WidgetId::Problems | WidgetId::Restarts => &[Interest::Pods],
            WidgetId::Capacity => &[],
            WidgetId::Nodes => &[Interest::Nodes],
            WidgetId::Health => &[Interest::Pods, Interest::Namespaces],
            WidgetId::Argo => &[Interest::ArgoApps],
            WidgetId::Events => &[Interest::Events],
        }
    }
}

/// A query a widget needs. Hiding the widget drops the interest.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Interest {
    Pods,
    Deployments,
    Nodes,
    ArgoApps,
    Events,
    Namespaces,
}

impl Interest {
    pub fn kind(self) -> &'static str {
        match self {
            Interest::Pods => "pods",
            Interest::Deployments => "deployments",
            Interest::Nodes => "nodes",
            Interest::ArgoApps => "argoapps",
            Interest::Events => "events",
            Interest::Namespaces => "namespaces",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    Full,
    Half,
}

impl Size {
    pub fn id(self) -> &'static str {
        match self {
            Size::Full => "full",
            Size::Half => "half",
        }
    }

    fn flip(self) -> Self {
        match self {
            Size::Full => Size::Half,
            Size::Half => Size::Full,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lane {
    Left,
    Right,
}

impl Lane {
    pub fn id(self) -> &'static str {
        match self {
            Lane::Left => "L",
            Lane::Right => "R",
        }
    }
}

/// One rendered row of the board: a full-width widget, or a band of two lanes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Group {
    Full(WidgetId),
    Band {
        left: Vec<WidgetId>,
        right: Vec<WidgetId>,
    },
}

/// Expanded state of the Usage panel, plus the metric series whose detail it
/// shows. The panel owns this itself; the workload list's tree toggle lives on
/// `crate::table::TableState`, so neither one moves the other.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UsagePanel {
    open: bool,
    metric: Option<&'static str>,
}

impl Default for UsagePanel {
    fn default() -> Self {
        Self {
            open: true,
            metric: None,
        }
    }
}

impl UsagePanel {
    /// Whether the panel body is showing.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// `data-open` / `aria-expanded` value for the markup.
    pub fn state(&self) -> &'static str {
        if self.open {
            "true"
        } else {
            "false"
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// The series whose metric detail is open. Collapsing the panel keeps the
    /// choice, so reopening lands back on the same detail.
    pub fn metric(&self) -> Option<&'static str> {
        self.metric
    }

    /// Show the detail for `series`; an open panel stays open.
    pub fn open_metric(&mut self, series: &'static str) {
        self.metric = Some(series);
        self.open = true;
    }

    pub fn close_metric(&mut self) {
        self.metric = None;
    }

    /// True when the body should render the metric detail instead of the gauges.
    pub fn shows_detail(&self) -> bool {
        self.open && self.metric.is_some()
    }
}

/// Board layout: order, visibility, per-widget size and lane, restart threshold.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OverviewLayout {
    pub order: Vec<WidgetId>,
    pub hidden: Vec<WidgetId>,
    pub sizes: BTreeMap<WidgetId, Size>,
    pub lanes: BTreeMap<WidgetId, Lane>,
    pub threshold: u32,
    pub editing: bool,
}

impl Default for OverviewLayout {
    fn default() -> Self {
        Self {
            order: WidgetId::ALL.to_vec(),
            hidden: Vec::new(),
            sizes: BTreeMap::new(),
            lanes: BTreeMap::new(),
            threshold: 3,
            editing: false,
        }
    }
}

impl OverviewLayout {
    pub fn size(&self, id: WidgetId) -> Size {
        self.sizes.get(&id).copied().unwrap_or(id.default_size())
    }

    pub fn is_hidden(&self, id: WidgetId) -> bool {
        self.hidden.contains(&id)
    }

    pub fn visible(&self) -> Vec<WidgetId> {
        self.order
            .iter()
            .copied()
            .filter(|id| !self.is_hidden(*id))
            .collect()
    }

    /// Rows the board needs: full-width widgets stand alone, half-width widgets
    /// share a band, balanced by rough height so the lanes stay level.
    pub fn groups(&self) -> Vec<Group> {
        let mut out = Vec::new();
        let mut band: Option<(Vec<WidgetId>, Vec<WidgetId>)> = None;
        let (mut left, mut right) = (0u32, 0u32);

        for id in self.visible() {
            if self.size(id) == Size::Full {
                if let Some((l, r)) = band.take() {
                    out.push(Group::Band { left: l, right: r });
                }
                left = 0;
                right = 0;
                out.push(Group::Full(id));
                continue;
            }
            let (l, r) = band.get_or_insert_with(|| (Vec::new(), Vec::new()));
            let lane = match self.lanes.get(&id).copied() {
                Some(lane) => lane,
                None if left <= right => Lane::Left,
                None => Lane::Right,
            };
            match lane {
                Lane::Left => {
                    l.push(id);
                    left += weight(id);
                }
                Lane::Right => {
                    r.push(id);
                    right += weight(id);
                }
            }
        }
        if let Some((l, r)) = band {
            out.push(Group::Band { left: l, right: r });
        }
        out
    }

    /// Moves a widget one slot inside its own lane or the full-width run.
    pub fn move_by(&mut self, id: WidgetId, delta: isize) -> bool {
        let mut groups = self.groups();
        let Some(gi) = groups.iter().position(|g| match g {
            Group::Full(w) => *w == id,
            Group::Band { left, right } => left.contains(&id) || right.contains(&id),
        }) else {
            return false;
        };

        match &mut groups[gi] {
            Group::Full(_) => {
                let j = gi as isize + delta;
                if j < 0 || j as usize >= groups.len() {
                    return false;
                }
                groups.swap(gi, j as usize);
            }
            Group::Band { left, right } => {
                let lane = if left.contains(&id) { left } else { right };
                let i = match lane.iter().position(|w| *w == id) {
                    Some(i) => i,
                    None => return false,
                };
                let j = i as isize + delta;
                if j < 0 || j as usize >= lane.len() {
                    return false;
                }
                lane.swap(i, j as usize);
            }
        }
        self.commit(groups);
        true
    }

    /// Moves a half-width widget into the other lane of its band.
    pub fn swap_lane(&mut self, id: WidgetId) -> bool {
        let mut groups = self.groups();
        let Some(group) = groups.iter_mut().find(|g| match g {
            Group::Band { left, right } => left.contains(&id) || right.contains(&id),
            Group::Full(_) => false,
        }) else {
            return false;
        };
        let Group::Band { left, right } = group else {
            return false;
        };
        if left.contains(&id) {
            left.retain(|w| *w != id);
            right.push(id);
        } else {
            right.retain(|w| *w != id);
            left.push(id);
        }
        self.commit(groups);
        true
    }

    /// Flattens bands back into order and records the lane each widget landed in.
    fn commit(&mut self, groups: Vec<Group>) {
        let mut order = Vec::new();
        let mut lanes = std::collections::BTreeMap::new();
        for group in groups {
            match group {
                Group::Full(w) => order.push(w),
                Group::Band { left, right } => {
                    for w in left {
                        order.push(w);
                        lanes.insert(w, Lane::Left);
                    }
                    for w in right {
                        order.push(w);
                        lanes.insert(w, Lane::Right);
                    }
                }
            }
        }
        for w in &self.order {
            if !order.contains(w) {
                order.push(*w);
            }
        }
        self.order = order;
        self.lanes = lanes;
    }

    pub fn toggle_size(&mut self, id: WidgetId) {
        let size = self.size(id).flip();
        self.sizes.insert(id, size);
        if size == Size::Full {
            self.lanes.remove(&id);
        }
    }

    pub fn hide(&mut self, id: WidgetId) {
        if !self.hidden.contains(&id) {
            self.hidden.push(id);
        }
    }

    pub fn show(&mut self, id: WidgetId) {
        self.hidden.retain(|w| *w != id);
    }

    pub fn reset(&mut self) {
        let editing = self.editing;
        *self = OverviewLayout {
            editing,
            ..OverviewLayout::default()
        };
    }

    /// Interests of the visible widgets only, sorted and deduplicated.
    pub fn interests(&self) -> Vec<Interest> {
        let mut out: Vec<Interest> = self
            .visible()
            .iter()
            .flat_map(|id| id.interests().iter().copied())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Saved form: `threshold;order=a,b;hidden=x;size=a:half;lane=b:R`.
    pub fn encode(&self) -> String {
        let order: Vec<&str> = self.order.iter().map(|w| w.id()).collect();
        let hidden: Vec<&str> = self.hidden.iter().map(|w| w.id()).collect();
        let sizes: Vec<String> = self
            .sizes
            .iter()
            .map(|(w, s)| format!("{}:{}", w.id(), s.id()))
            .collect();
        let lanes: Vec<String> = self
            .lanes
            .iter()
            .map(|(w, l)| format!("{}:{}", w.id(), l.id()))
            .collect();
        format!(
            "{};order={};hidden={};size={};lane={}",
            self.threshold,
            order.join(","),
            hidden.join(","),
            sizes.join(","),
            lanes.join(",")
        )
    }

    /// Reads a saved layout, dropping keys this build no longer knows.
    pub fn decode(raw: &str) -> Self {
        let mut layout = OverviewLayout::default();
        let mut parts = raw.split(';');
        if let Some(thr) = parts.next().and_then(|t| t.parse::<u32>().ok()) {
            layout.threshold = thr;
        }
        let mut order: Vec<WidgetId> = Vec::new();
        for part in parts {
            let Some((key, value)) = part.split_once('=') else {
                continue;
            };
            match key {
                "order" => {
                    for id in value.split(',').filter_map(WidgetId::from_id) {
                        if !order.contains(&id) {
                            order.push(id);
                        }
                    }
                }
                "hidden" => {
                    layout.hidden = value.split(',').filter_map(WidgetId::from_id).collect();
                }
                "size" => {
                    for (id, mode) in pairs(value) {
                        if let Some(id) = WidgetId::from_id(id) {
                            let size = if mode == "full" {
                                Size::Full
                            } else {
                                Size::Half
                            };
                            layout.sizes.insert(id, size);
                        }
                    }
                }
                "lane" => {
                    for (id, lane) in pairs(value) {
                        if let Some(id) = WidgetId::from_id(id) {
                            let lane = if lane == "R" { Lane::Right } else { Lane::Left };
                            layout.lanes.insert(id, lane);
                        }
                    }
                }
                _ => {}
            }
        }
        for id in WidgetId::ALL {
            if !order.contains(&id) {
                order.push(id);
            }
        }
        layout.order = order;
        layout
    }
}

/// Everything the board can ask for. The view emits these; the caller owns the state.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BoardAction {
    Move { id: WidgetId, delta: isize },
    SwapLane(WidgetId),
    ToggleSize(WidgetId),
    Hide(WidgetId),
    Show(WidgetId),
    Reset,
    ToggleEdit,
    Threshold(u32),
    Goto(String),
    OpenKind(&'static str),
    ClearNamespaces,
}

/// Splits `a:half,b:full` into pairs.
fn pairs(value: &str) -> impl Iterator<Item = (&str, &str)> {
    value.split(',').filter_map(|entry| entry.split_once(':'))
}

fn weight(id: WidgetId) -> u32 {
    match id {
        WidgetId::Usage | WidgetId::Nodes | WidgetId::Events => 2,
        _ => 1,
    }
}

/// A pod that needs a look, as the board shows it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProblemRow {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub status: String,
    pub restarts: u32,
}

/// Everything the board renders from: counts plus the windowed rows it asked for.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct OverviewData {
    pub cluster: String,
    pub counts: BTreeMap<&'static str, usize>,
    pub pods_total: usize,
    pub pods_running: usize,
    pub pods_by_status: Vec<(String, usize)>,
    pub problems: Vec<ProblemRow>,
    pub restarts: Vec<ProblemRow>,
    pub nodes: Vec<NodeRow>,
    pub argo: Vec<ArgoRow>,
    pub events: Vec<EventRow>,
    pub namespaces: Vec<(String, usize)>,
    pub capacity: Vec<CapacityGauge>,
    /// Source, band and windowed samples the usage charts draw from. Empty until a
    /// caller attaches one, which is what a cluster without metrics-server looks like.
    pub metrics: crate::metrics::MetricsView,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NodeRow {
    pub id: String,
    pub name: String,
    pub cpu_pct: u8,
    pub mem_pct: u8,
    pub note: String,
    pub degraded: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ArgoRow {
    pub id: String,
    pub name: String,
    pub sync: String,
    pub health: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EventRow {
    pub target: Option<String>,
    pub kind: String,
    pub reason: String,
    pub object: String,
    pub message: String,
    pub age: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CapacityGauge {
    pub label: String,
    pub value: String,
    pub pct: u8,
    pub tone: &'static str,
}

/// What the "needs attention" widget counts.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Attention {
    pub failing: usize,
    pub pending: usize,
    pub restarts: usize,
    pub degraded: usize,
    pub nodes: usize,
    pub argo: usize,
    pub warnings: usize,
}

impl OverviewData {
    /// Counts the attention widget from rows already in hand.
    pub fn attention(&self, threshold: u32) -> Attention {
        let failing = self
            .problems
            .iter()
            .filter(|p| p.status == "CrashLoopBackOff" || p.status == "Failed")
            .count();
        let pending = self
            .problems
            .iter()
            .filter(|p| p.status == "Pending" || p.status == "ContainerCreating")
            .count();
        Attention {
            failing,
            pending,
            restarts: self
                .restarts
                .iter()
                .filter(|p| p.restarts >= threshold)
                .count(),
            degraded: self.nodes.iter().filter(|n| n.degraded).count(),
            nodes: self.nodes.iter().filter(|n| n.degraded).count(),
            argo: self
                .argo
                .iter()
                .filter(|a| a.sync != "Synced" || a.health != "Healthy")
                .count(),
            warnings: self.events.iter().filter(|e| e.kind == "Warning").count(),
        }
    }

    /// Board data from the caller's counts plus whatever rows were fetched.
    pub fn from_counts(cluster: impl Into<String>, counts: &crate::shell::Counts) -> Self {
        OverviewData {
            cluster: cluster.into(),
            counts: counts.as_map().clone(),
            ..OverviewData::default()
        }
    }

    /// Attaches the metrics window the usage charts draw from.
    pub fn with_metrics(mut self, metrics: crate::metrics::MetricsView) -> Self {
        self.metrics = metrics;
        self
    }

    /// Sorts pod rows into the problems and restarts lists.
    pub fn with_pods(mut self, pods: Vec<Row>, threshold: u32) -> Self {
        self.pods_total = pods.len();
        let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
        for pod in &pods {
            let status = pod.cell("status").to_string();
            if status == "Running" {
                self.pods_running += 1;
            }
            *statuses.entry(status.clone()).or_default() += 1;
            let restarts = pod.cell("restarts").parse::<u32>().unwrap_or(0);
            let entry = ProblemRow {
                id: pod.id.clone(),
                name: pod.name.clone(),
                namespace: pod.namespace.clone().unwrap_or_default(),
                status: status.clone(),
                restarts,
            };
            if matches!(
                status.as_str(),
                "Pending" | "ContainerCreating" | "CrashLoopBackOff" | "Failed"
            ) {
                self.problems.push(entry.clone());
            }
            if restarts >= 1 {
                self.restarts.push(entry);
            }
        }
        self.problems.sort_by(|a, b| b.restarts.cmp(&a.restarts));
        self.restarts.retain(|p| p.restarts >= threshold);
        self.restarts.sort_by(|a, b| b.restarts.cmp(&a.restarts));
        self.pods_by_status = statuses.into_iter().collect();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::{Row, TableState};

    #[test]
    fn the_usage_panel_starts_open_on_its_gauges() {
        let panel = UsagePanel::default();
        assert!(panel.is_open());
        assert_eq!(panel.state(), "true");
        assert_eq!(panel.metric(), None);
        assert!(!panel.shows_detail());
    }

    #[test]
    fn activating_a_gauge_opens_that_series_and_back_returns_to_the_gauges() {
        let mut panel = UsagePanel::default();
        panel.open_metric("cpu");
        assert_eq!(panel.metric(), Some("cpu"));
        assert!(panel.shows_detail());
        panel.open_metric("mem");
        assert_eq!(panel.metric(), Some("mem"));
        panel.close_metric();
        assert_eq!(panel.metric(), None);
        assert!(!panel.shows_detail());
    }

    #[test]
    fn activating_a_gauge_reopens_a_collapsed_panel() {
        let mut panel = UsagePanel::default();
        panel.toggle();
        assert!(!panel.is_open());
        panel.open_metric("cpu");
        assert!(panel.is_open());
        assert!(panel.shows_detail());
    }

    #[test]
    fn collapsing_the_usage_panel_leaves_the_workload_tree_alone() {
        let mut panel = UsagePanel::default();
        let mut tree = TableState::default();
        let rows = [Row::new("pods", "pods").with_kids(vec![Row::new("pod", "api-0")])];

        tree.toggle_expand_all(&rows);
        assert!(tree.all_expanded(&rows));
        assert!(
            panel.is_open(),
            "expanding the tree collapsed the usage panel"
        );

        panel.toggle();
        assert!(!panel.is_open());
        assert!(
            tree.all_expanded(&rows),
            "collapsing the usage panel moved the workload tree"
        );

        panel.open_metric("cpu");
        assert!(
            tree.all_expanded(&rows),
            "opening a metric detail moved the workload tree"
        );
        assert!(panel.is_open());
    }
}
