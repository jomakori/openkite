//! Pure model for the inspector slide-over: which tabs a kind gets, the quick-action
//! bar, and the panel data the caller hands in.
//!
//! Manifests arrive on demand. [`InspectorData`] carries `manifest: Option<Manifest>` and
//! [`Row`](crate::table::Row) has no manifest field at all, so a list row can never drag
//! YAML along with it.

/// One inspector tab. The order of [`Tab::ALL`] is the order the prototype renders.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tab {
    #[default]
    Overview,
    Containers,
    Metrics,
    Resources,
    Schema,
    Yaml,
    Events,
}

impl Tab {
    pub const ALL: [Tab; 7] = [
        Tab::Overview,
        Tab::Containers,
        Tab::Metrics,
        Tab::Resources,
        Tab::Schema,
        Tab::Yaml,
        Tab::Events,
    ];

    /// Stable id used for `data-tab` and as the panel discriminator.
    pub fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Containers => "containers",
            Tab::Metrics => "metrics",
            Tab::Resources => "resources",
            Tab::Schema => "schema",
            Tab::Yaml => "yaml",
            Tab::Events => "events",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Containers => "Containers",
            Tab::Metrics => "Metrics",
            Tab::Resources => "Resources",
            Tab::Schema => "Schema",
            Tab::Yaml => "YAML",
            Tab::Events => "Events",
        }
    }
}

/// Kinds that publish metrics; everything else only gets the overview, YAML and events.
pub const METRIC_KINDS: &[&str] = &[
    "pods",
    "deployments",
    "statefulsets",
    "daemonsets",
    "replicasets",
    "namespaces",
    "nodes",
];

/// Tab set for a kind, in render order — the prototype's `tabsDef` rule for rule.
pub fn tabs_for(kind: &str) -> Vec<Tab> {
    let mut tabs = vec![Tab::Overview];
    if kind == "pods" {
        tabs.push(Tab::Containers);
    }
    if METRIC_KINDS.contains(&kind) {
        tabs.push(Tab::Metrics);
    }
    match kind {
        "crds" => tabs.extend([Tab::Resources, Tab::Schema]),
        "helm" => tabs.push(Tab::Resources),
        _ => {}
    }
    tabs.extend([Tab::Yaml, Tab::Events]);
    tabs
}

/// Whether a kind exposes a tab. Used to keep the active tab valid after the kind changes.
pub fn has_tab(kind: &str, tab: Tab) -> bool {
    tabs_for(kind).contains(&tab)
}

/// Which row is open and which tab is showing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InspectorState {
    pub open: Option<String>,
    pub tab: Tab,
    /// Whether the overflow action menu is showing.
    pub menu_open: bool,
}

impl InspectorState {
    /// Open a row. A different row starts on Overview, the same row keeps its tab.
    pub fn open(&mut self, id: impl Into<String>, kind: &str) {
        let id = id.into();
        if self.open.as_deref() != Some(id.as_str()) {
            self.tab = Tab::Overview;
        }
        if !has_tab(kind, self.tab) {
            self.tab = Tab::Overview;
        }
        self.open = Some(id);
    }

    pub fn close(&mut self) {
        self.open = None;
        self.tab = Tab::Overview;
        self.menu_open = false;
    }

    pub fn toggle_menu(&mut self) {
        self.menu_open = !self.menu_open;
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Switch tabs, refusing a tab the kind does not have.
    pub fn show_tab(&mut self, kind: &str, tab: Tab) -> bool {
        if !has_tab(kind, tab) {
            return false;
        }
        self.tab = tab;
        true
    }
}

/// One inspector action button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuickAction {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub danger: bool,
}

impl QuickAction {
    /// The prototype styles Logs as the primary action.
    pub fn primary(self) -> bool {
        self.id == "logs"
    }
}

const ACTIONS: &[QuickAction] = &[
    QuickAction {
        id: "logs",
        label: "Logs",
        icon: "logs",
        danger: false,
    },
    QuickAction {
        id: "shell",
        label: "Shell",
        icon: "terminal",
        danger: false,
    },
    QuickAction {
        id: "scale",
        label: "Scale",
        icon: "scale",
        danger: false,
    },
    QuickAction {
        id: "restart",
        label: "Restart",
        icon: "refresh",
        danger: false,
    },
    QuickAction {
        id: "trigger",
        label: "Trigger now",
        icon: "sync",
        danger: false,
    },
    QuickAction {
        id: "cordon",
        label: "Cordon",
        icon: "close",
        danger: false,
    },
    QuickAction {
        id: "drain",
        label: "Drain",
        icon: "arrow",
        danger: false,
    },
    QuickAction {
        id: "edit",
        label: "Edit YAML",
        icon: "pencil",
        danger: false,
    },
    QuickAction {
        id: "delete",
        label: "Delete",
        icon: "trash",
        danger: true,
    },
];

/// Actions kept out of the button row and tucked into the overflow menu.
pub const TUCKED: &[&str] = &["scale", "restart", "cordon", "drain", "delete"];

/// Actions a kind offers, in prototype order.
pub fn actions_for(kind: &str) -> &'static [&'static str] {
    match kind {
        "pods" => &["logs", "shell", "edit", "delete"],
        "deployments" | "statefulsets" | "daemonsets" => {
            &["logs", "scale", "restart", "edit", "delete"]
        }
        "nodes" => &["cordon", "drain", "edit"],
        "cronjobs" => &["trigger", "edit", "delete"],
        "jobs" | "replicasets" | "namespaces" | "services" | "secrets" | "configmaps" | "pvcs"
        | "storageclasses" | "helm" => &["logs", "edit", "delete"],
        _ => &["edit"],
    }
}

/// Look up one action by id.
pub fn action(id: &str) -> Option<QuickAction> {
    ACTIONS.iter().copied().find(|a| a.id == id)
}

/// How the bar is split: Logs/Shell sit left, the rest right, the tucked set goes to the
/// overflow menu with Edit first and Delete last.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActionBar {
    pub left: Vec<QuickAction>,
    pub right: Vec<QuickAction>,
    pub menu: Vec<QuickAction>,
}

pub fn action_bar(kind: &str) -> ActionBar {
    let offered: Vec<QuickAction> = actions_for(kind)
        .iter()
        .filter_map(|id| action(id))
        .collect();
    let visible = |a: &QuickAction| a.id != "edit" && !TUCKED.contains(&a.id);
    let bar = ActionBar {
        left: offered
            .iter()
            .copied()
            .filter(|a| visible(a) && (a.id == "logs" || a.id == "shell"))
            .collect(),
        right: offered
            .iter()
            .copied()
            .filter(|a| visible(a) && a.id != "logs" && a.id != "shell")
            .collect(),
        menu: offered
            .iter()
            .copied()
            .filter(|a| a.id == "edit")
            .chain(
                offered
                    .iter()
                    .copied()
                    .filter(|a| TUCKED.contains(&a.id) && a.id != "delete"),
            )
            .chain(offered.iter().copied().filter(|a| a.id == "delete"))
            .collect(),
    };
    bar
}

/// A fetched manifest. Only the inspector ever holds one of these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub text: String,
}

impl Manifest {
    pub fn new(text: impl Into<String>) -> Self {
        Manifest { text: text.into() }
    }

    /// Line count, used for the gutter.
    pub fn line_count(&self) -> usize {
        self.text.lines().count().max(1)
    }
}

/// Overview key/value rows, labels, annotations and conditions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Detail {
    pub props: Vec<(String, String)>,
    pub labels: Vec<(String, String)>,
    pub annotations: Vec<(String, String)>,
    /// Condition name and whether it currently passes.
    pub conditions: Vec<(String, bool)>,
}

/// One usage gauge in the overview and metrics panels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gauge {
    pub label: &'static str,
    pub value: String,
    /// Fill percentage, clamped when rendered.
    pub pct: u8,
}

/// One container card in the Containers panel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Container {
    pub name: String,
    pub image: String,
    pub ports: String,
    pub restarts: u32,
    pub requests: String,
    pub limits: String,
    pub liveness: String,
    pub mounts: String,
}

/// One row in the Events panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    /// `Normal` or `Warning`; rendered as the list item class.
    pub kind: &'static str,
    pub reason: String,
    pub message: String,
    pub age: String,
}

/// One linked instance in the Resources panel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceLink {
    pub id: String,
    pub label: String,
    pub status: Option<String>,
}

/// One field in the Schema panel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SchemaNode {
    pub name: String,
    pub ty: String,
    pub description: String,
    pub children: Vec<SchemaNode>,
}

impl SchemaNode {
    pub fn new(
        name: impl Into<String>,
        ty: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        SchemaNode {
            name: name.into(),
            ty: ty.into(),
            description: description.into(),
            children: Vec::new(),
        }
    }

    pub fn with_children(mut self, children: Vec<SchemaNode>) -> Self {
        self.children = children;
        self
    }
}

/// Everything the panels render. Fields the active kind does not use stay empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InspectorData {
    pub detail: Detail,
    pub gauges: Vec<Gauge>,
    pub containers: Vec<Container>,
    pub events: Vec<Event>,
    pub resources: Vec<ResourceLink>,
    pub schema: Vec<SchemaNode>,
    /// `None` until the manifest has been fetched for this row.
    pub manifest: Option<Manifest>,
}

impl InspectorData {
    pub fn with_detail(detail: Detail) -> Self {
        InspectorData {
            detail,
            ..InspectorData::default()
        }
    }

    pub fn manifest_fetched(&self) -> bool {
        self.manifest.is_some()
    }
}

/// Kinds without a published schema fall back to this single field.
pub fn default_schema() -> Vec<SchemaNode> {
    vec![SchemaNode::new(
        "spec",
        "object",
        "No schema published for this definition",
    )]
}

/// Tile the tab list into one string for logs and assertions.
pub fn tab_ids(kind: &str) -> Vec<&'static str> {
    tabs_for(kind).into_iter().map(Tab::id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_rules_follow_the_reference() {
        assert_eq!(
            tab_ids("pods"),
            ["overview", "containers", "metrics", "yaml", "events"]
        );
        assert_eq!(
            tab_ids("deployments"),
            ["overview", "metrics", "yaml", "events"]
        );
        assert_eq!(
            tab_ids("crds"),
            ["overview", "resources", "schema", "yaml", "events"]
        );
        assert_eq!(tab_ids("helm"), ["overview", "resources", "yaml", "events"]);
        assert_eq!(tab_ids("services"), ["overview", "yaml", "events"]);
    }

    #[test]
    fn every_kind_opens_on_overview_and_ends_on_events() {
        for kind in ["pods", "nodes", "crds", "helm", "secrets", "unknown-kind"] {
            let tabs = tabs_for(kind);
            assert_eq!(tabs.first(), Some(&Tab::Overview), "{kind}");
            assert_eq!(tabs.last(), Some(&Tab::Events), "{kind}");
            assert!(tabs.contains(&Tab::Yaml), "{kind}");
        }
    }

    #[test]
    fn opening_another_row_returns_to_overview() {
        let mut state = InspectorState::default();
        state.open("pod/a", "pods");
        assert!(state.show_tab("pods", Tab::Metrics));
        state.open("pod/b", "pods");
        assert_eq!(state.tab, Tab::Overview);
        state.open("pod/b", "pods");
        assert_eq!(state.tab, Tab::Overview);
    }

    #[test]
    fn the_action_menu_starts_closed_and_closes_with_the_inspector() {
        let mut state = InspectorState::default();
        state.open("pod/a", "pods");
        assert!(!state.menu_open);
        state.toggle_menu();
        assert!(state.menu_open);
        state.close();
        assert!(!state.menu_open);
    }

    #[test]
    fn unavailable_tabs_are_refused() {
        let mut state = InspectorState::default();
        state.open("svc/a", "services");
        assert!(!state.show_tab("services", Tab::Metrics));
        assert_eq!(state.tab, Tab::Overview);
        assert!(state.show_tab("services", Tab::Yaml));
    }

    #[test]
    fn actions_split_into_logs_left_and_overflow_menu() {
        let pods = action_bar("pods");
        assert_eq!(
            pods.left.iter().map(|a| a.id).collect::<Vec<_>>(),
            ["logs", "shell"]
        );
        assert!(pods.right.is_empty());
        assert_eq!(
            pods.menu.iter().map(|a| a.id).collect::<Vec<_>>(),
            ["edit", "delete"]
        );
        assert!(pods.menu.last().unwrap().danger);
    }

    #[test]
    fn deployments_tuck_scale_and_restart() {
        let bar = action_bar("deployments");
        assert_eq!(bar.left.iter().map(|a| a.id).collect::<Vec<_>>(), ["logs"]);
        assert_eq!(
            bar.menu.iter().map(|a| a.id).collect::<Vec<_>>(),
            ["edit", "scale", "restart", "delete"]
        );
    }

    #[test]
    fn cronjobs_keep_trigger_in_the_row() {
        let bar = action_bar("cronjobs");
        assert!(bar.left.is_empty());
        assert_eq!(
            bar.right.iter().map(|a| a.id).collect::<Vec<_>>(),
            ["trigger"]
        );
    }

    #[test]
    fn manifest_only_exists_once_fetched() {
        let mut data = InspectorData::default();
        assert!(!data.manifest_fetched());
        data.manifest = Some(Manifest::new("kind: Pod\nmetadata:\n  name: api\n"));
        assert!(data.manifest_fetched());
        assert_eq!(data.manifest.as_ref().unwrap().line_count(), 3);
    }

    #[test]
    fn schema_fallback_is_one_object_field() {
        let fallback = default_schema();
        assert_eq!(fallback.len(), 1);
        assert_eq!(fallback[0].name, "spec");
    }
}
