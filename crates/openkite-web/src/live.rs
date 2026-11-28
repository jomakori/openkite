//! Live cluster adapter: turns `Engine` queries into the view-models `openkite-ui` renders.
//!
//! Every list call gets its own timeout and turns a failure into an empty result for that
//! slice, so one flaky query degrades a widget instead of failing the whole page.

use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{Event, Namespace, Node, Pod};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::api::{Api, ListParams};
use kube::core::{ApiResource, DynamicObject, GroupVersionKind};
use openkite_engine::Engine;
use openkite_ui::overview::{
    ArgoRow, CapacityGauge, EventRow, NodeRow, OverviewData, OverviewLayout,
};
use openkite_ui::metrics::{
    ChartFeed, MetricSource, MetricsView, Series, SourceProbe, SAMPLE_CAP,
};
use openkite_ui::shell::{Connection, Counts};
use openkite_ui::table::Row;

/// Budget for any single cluster list call; a slower query yields an empty result, not a hang.
const QUERY_TIMEOUT: Duration = Duration::from_secs(10);

/// Cluster access for the web shell: a connected engine, or a stand-in that answers every
/// query with empty data (used when the initial connect fails, and by tests).
pub enum Live {
    Connected(Arc<Engine>),
    Unavailable,
}

impl Live {
    /// Connects now; falls back to [`Live::Unavailable`] on failure so the server still starts
    /// and serves an empty console instead of refusing to boot without a cluster. With the
    /// `fixtures` feature and `OPENKITE_DATA_MODE=fixtures`, the seeded local cluster is used
    /// instead and no cluster credential is read at all.
    pub async fn connect() -> Self {
        #[cfg(feature = "fixtures")]
        if crate::fixtures::requested() {
            return crate::fixtures::connect().await;
        }

        match Engine::connect(None).await {
            Ok(engine) => Live::from_engine(engine),
            Err(error) => {
                tracing::warn!(%error, "engine connect failed; serving an empty console");
                Live::Unavailable
            }
        }
    }

    /// Wraps an engine a caller already connected.
    pub fn from_engine(engine: Engine) -> Self {
        Live::Connected(Arc::new(engine))
    }

    /// A state that answers every query with empty data: the CI/test fallback, no kube creds needed.
    pub fn unavailable() -> Self {
        Live::Unavailable
    }

    /// Sidebar connection indicator for the current state.
    pub fn connection(&self) -> Connection {
        match self {
            Live::Connected(_) => Connection::Live,
            Live::Unavailable => Connection::Offline,
        }
    }

    /// Cluster label shown in the shell chrome.
    pub fn cluster(&self) -> String {
        match self {
            Live::Connected(engine) => engine.context().to_owned(),
            Live::Unavailable => "unavailable".to_owned(),
        }
    }

    /// Aggregated events for `/events`, optionally scoped to one involved object.
    ///
    /// The object scope is a field selector on the apiserver (`involvedObject.uid=<uid>`), so
    /// a per-object tab lists exactly the events the server attributes to that object.
    pub async fn events(&self, object_uid: Option<&str>) -> Vec<openkite_ui::inspector::Event> {
        let Live::Connected(engine) = self else {
            return Vec::new();
        };
        let client = engine.client();
        let raw = match object_uid {
            Some(uid) => timed(openkite_engine::events::list_for_object(&client, uid)).await,
            None => timed(list_all::<Event>(client.clone())).await,
        };
        aggregated_event_rows(&raw.unwrap_or_default())
    }

    /// Board data for `/`. See module docs for the per-field timeout/fallback behaviour.
    pub async fn overview(&self) -> OverviewData {
        let Live::Connected(engine) = self else {
            return OverviewData::from_counts("unavailable", &Counts::new());
        };
        let client = engine.client();

        // One round-trip window for all six lists: `join!` keeps a slow kind from
        // stacking its timeout on top of the others.
        let (pods, nodes_raw, namespaces_raw, events_raw, deployments, argo, pod_usage, node_usage) =
            tokio::join!(
                timed(list_all::<Pod>(client.clone())),
                timed(list_all::<Node>(client.clone())),
                timed(list_all::<Namespace>(client.clone())),
                timed(list_all::<Event>(client.clone())),
                timed(list_all::<Deployment>(client.clone())),
                timed(fetch_argo_apps(client.clone())),
                timed(fetch_pod_usage(client.clone())),
                timed(fetch_node_usage(client.clone())),
            );
        let pods = pods.unwrap_or_default();
        let nodes_raw = nodes_raw.unwrap_or_default();
        let namespaces_raw = namespaces_raw.unwrap_or_default();
        let events_raw = events_raw.unwrap_or_default();
        let deployments = deployments.unwrap_or_default();
        let argo = argo.unwrap_or_default();
        let pod_usage_answered = pod_usage.is_some();
        let node_usage_answered = node_usage.is_some();
        let pod_usage = pod_usage.unwrap_or_default();
        let node_usage = node_usage.unwrap_or_default();

        let pressure = Pressure::read(&nodes_raw, &pods);
        let counts = Counts::new()
            .with("pods", pods.len())
            .with("nodes", nodes_raw.len())
            .with("namespaces", namespaces_raw.len())
            .with("events", events_raw.len())
            .with("deployments", deployments.len())
            .with("cpu_pct", pressure.cpu_pct() as usize)
            .with("mem_pct", pressure.mem_pct() as usize);

        let threshold = OverviewLayout::default().threshold;
        let mut data = OverviewData::from_counts(engine.context(), &counts)
            .with_pods(pod_rows(&pods, &pod_usage, false), threshold)
            .with_metrics(metrics_view(
                &pods,
                &nodes_raw,
                &node_usage,
                &pressure,
                pod_usage_answered || node_usage_answered,
            ));
        data.nodes = node_rows(&nodes_raw, &pods);
        data.argo = argo;
        data.events = event_rows(&events_raw);
        data.namespaces = namespace_counts(&namespaces_raw, &pods);
        data.capacity = pressure.gauges(pods.len());
        data
    }

    /// One row per pod for `/table`.
    /// What the identity may do to one resource, as the view model wants it.
    ///
    /// A review that fails leaves the console as it was rather than blanking a working
    /// screen; the failure is logged so it can be seen.
    pub async fn permissions(
        &self,
        group: &str,
        resource: &str,
        namespace: Option<&str>,
    ) -> openkite_ui::access::Permissions {
        let Live::Connected(engine) = self else {
            return openkite_ui::access::Permissions::default();
        };
        match engine
            .gate()
            .capabilities(&engine.client(), group, resource, namespace)
            .await
        {
            Ok(capabilities) => {
                let mut permissions = openkite_ui::access::Permissions::default();
                for (verb, view) in [
                    (
                        openkite_engine::access::Verb::Get,
                        openkite_ui::access::Verb::Get,
                    ),
                    (
                        openkite_engine::access::Verb::List,
                        openkite_ui::access::Verb::List,
                    ),
                    (
                        openkite_engine::access::Verb::Create,
                        openkite_ui::access::Verb::Create,
                    ),
                    (
                        openkite_engine::access::Verb::Update,
                        openkite_ui::access::Verb::Update,
                    ),
                    (
                        openkite_engine::access::Verb::Delete,
                        openkite_ui::access::Verb::Delete,
                    ),
                ] {
                    permissions.set(view, capabilities.allows(verb), capabilities.reason(verb));
                }
                permissions
            }
            Err(error) => {
                tracing::warn!(%error, resource, "authorization review failed; showing controls");
                openkite_ui::access::Permissions::default()
            }
        }
    }

    pub async fn table(&self, trends: bool) -> Vec<Row> {
        let Live::Connected(engine) = self else {
            return Vec::new();
        };
        let client = engine.client();
        let (pods, usage) = tokio::join!(
            timed(list_all::<Pod>(client.clone())),
            timed(fetch_pod_usage(client)),
        );
        pod_rows(&pods.unwrap_or_default(), &usage.unwrap_or_default(), trends)
    }
}

/// Runs `fut`, turning a timeout or a cluster error into `None` and logging the reason.
async fn timed<F, T>(fut: F) -> Option<T>
where
    F: std::future::Future<Output = Result<T, kube::Error>>,
{
    match tokio::time::timeout(QUERY_TIMEOUT, fut).await {
        Ok(Ok(value)) => Some(value),
        Ok(Err(error)) => {
            tracing::warn!(%error, "cluster query failed");
            None
        }
        Err(_) => {
            tracing::warn!("cluster query timed out");
            None
        }
    }
}

/// Lists every object of a typed kind, cluster-wide, in one page.
async fn list_all<K>(client: kube::Client) -> Result<Vec<K>, kube::Error>
where
    K: kube::Resource<DynamicType = ()> + Clone + serde::de::DeserializeOwned + std::fmt::Debug,
{
    let api: Api<K> = Api::all(client);
    let list = api.list(&ListParams::default()).await?;
    Ok(list.items)
}

/// Argo CD `Application` CRs in the `argocd` namespace, best-effort (CRD may not be installed).
async fn fetch_argo_apps(client: kube::Client) -> Result<Vec<ArgoRow>, kube::Error> {
    let gvk = GroupVersionKind {
        group: "argoproj.io".to_owned(),
        version: "v1alpha1".to_owned(),
        kind: "Application".to_owned(),
    };
    let resource = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::namespaced_with(client, "argocd", &resource);
    let list = api.list(&ListParams::default()).await?;
    Ok(list
        .items
        .iter()
        .map(|item| {
            let name = item.metadata.name.clone().unwrap_or_default();
            let sync = dynamic_str(item, &["status", "sync", "status"])
                .unwrap_or_else(|| "Unknown".to_owned());
            let health = dynamic_str(item, &["status", "health", "status"])
                .unwrap_or_else(|| "Unknown".to_owned());
            ArgoRow {
                id: format!("argo/{name}"),
                name,
                sync,
                health,
            }
        })
        .collect())
}

/// Walks a dotted path into a `DynamicObject`'s free-form data and reads a string leaf.
/// Where a trend column keeps its history: one feed per row id, so a row's line survives
/// page loads for as long as the process lives. A feed that was told its surface is
/// hidden records nothing until it is shown again.
static TRENDS: LazyLock<Mutex<BTreeMap<String, ChartFeed>>> = LazyLock::new(Default::default);

/// Offers one reading to a row's feed and returns the window to draw.
///
/// `visible` is the caller's answer to "is the trend column on screen". A hidden column
/// pauses its feed instead of plotting a line nobody sees, which is the ticket's
/// requirement stated as code rather than as an intention.
fn feed_trend(row_id: &str, pct: u8, visible: bool) -> Option<Series> {
    let mut feeds = TRENDS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let feed = feeds
        .entry(row_id.to_string())
        .or_insert_with(|| ChartFeed::new(SAMPLE_CAP));
    feed.set_visible(visible);
    feed.offer(pct);
    Some(feed.series().clone())
}

/// Live usage as the metrics API reports it, keyed `namespace/name`.
#[derive(Default)]
struct Usage {
    cpu: BTreeMap<String, f64>,
    mem: BTreeMap<String, f64>,
}

/// CPU and memory per pod from `metrics.k8s.io/v1beta1/PodMetrics`.
///
/// The API is optional on a cluster, so a missing API is an error the caller turns into
/// "no source" rather than a failure of the page.
async fn fetch_pod_usage(client: kube::Client) -> Result<Usage, kube::Error> {
    let resource =
        ApiResource::from_gvk_with_plural(&GroupVersionKind::gvk("metrics.k8s.io", "v1beta1", "PodMetrics"), "pods");
    let items = Api::<DynamicObject>::all_with(client, &resource)
        .list(&ListParams::default())
        .await?
        .items;
    let mut usage = Usage::default();
    for item in &items {
        let Some(key) = metric_key(item) else { continue };
        let containers = item
            .data
            .pointer("/containers")
            .and_then(|value| value.as_array());
        let mut cpu = 0.0;
        let mut mem = 0.0;
        for container in containers.into_iter().flatten() {
            cpu += container
                .pointer("/usage/cpu")
                .and_then(|value| value.as_str())
                .and_then(parse_quantity)
                .unwrap_or(0.0);
            mem += container
                .pointer("/usage/memory")
                .and_then(|value| value.as_str())
                .and_then(parse_quantity)
                .unwrap_or(0.0);
        }
        usage.cpu.insert(key.clone(), cpu);
        usage.mem.insert(key, mem);
    }
    Ok(usage)
}

/// Cluster-wide usage from `NodeMetrics`, for the overview's aggregate charts.
async fn fetch_node_usage(client: kube::Client) -> Result<Usage, kube::Error> {
    let resource = ApiResource::from_gvk_with_plural(
        &GroupVersionKind::gvk("metrics.k8s.io", "v1beta1", "NodeMetrics"),
        "nodes",
    );
    let items = Api::<DynamicObject>::all_with(client, &resource)
        .list(&ListParams::default())
        .await?
        .items;
    let mut usage = Usage::default();
    for item in &items {
        let Some(name) = item.metadata.name.clone() else { continue };
        let cpu = item
            .data
            .pointer("/usage/cpu")
            .and_then(|value| value.as_str())
            .and_then(parse_quantity)
            .unwrap_or(0.0);
        let mem = item
            .data
            .pointer("/usage/memory")
            .and_then(|value| value.as_str())
            .and_then(parse_quantity)
            .unwrap_or(0.0);
        usage.cpu.insert(name.clone(), cpu);
        usage.mem.insert(name, mem);
    }
    Ok(usage)
}

/// `namespace/name` for a namespaced metrics item; `None` when the item says neither.
fn metric_key(item: &DynamicObject) -> Option<String> {
    let name = item.metadata.name.clone()?;
    Some(match item.metadata.namespace.clone() {
        Some(namespace) => format!("{namespace}/{name}"),
        None => name,
    })
}

/// What a pod's usage is measured against: the request it reserved for that resource, so
/// `100%` means "using exactly what it asked for". A pod with no request has no scale and
/// gets no trend rather than a made-up one.
fn usage_pct(used: f64, reserved: f64) -> Option<u8> {
    if reserved <= 0.0 {
        return None;
    }
    Some(((used / reserved) * 100.0).round().clamp(0.0, 999.0) as u8)
}

fn dynamic_str(item: &DynamicObject, path: &[&str]) -> Option<String> {
    let mut value = &item.data;
    for key in path {
        value = value.get(key)?;
    }
    value.as_str().map(str::to_owned)
}

/// Pod rows for both the overview board and the pods table. Columns mirror
/// `crates/openkite-ui/src/table_columns.rs`'s `pods` entry; no new columns invented.
fn pod_rows(pods: &[Pod], usage: &Usage, visible: bool) -> Vec<Row> {
    pods.iter()
        .map(|pod| {
            let name = pod.metadata.name.clone().unwrap_or_default();
            let namespace = pod.metadata.namespace.clone().unwrap_or_default();
            let status = pod
                .status
                .as_ref()
                .and_then(|status| status.phase.clone())
                .unwrap_or_default();
            let container_statuses = pod
                .status
                .as_ref()
                .and_then(|status| status.container_statuses.as_ref());
            let (ready_count, total, restarts) = container_statuses
                .map(|statuses| {
                    let ready = statuses.iter().filter(|cs| cs.ready).count();
                    let total = statuses.len();
                    let restarts: i32 = statuses.iter().map(|cs| cs.restart_count).sum();
                    (ready, total, restarts)
                })
                .unwrap_or((0, 0, 0));
            let qos = pod
                .status
                .as_ref()
                .and_then(|status| status.qos_class.clone())
                .unwrap_or_default();
            let node = pod
                .spec
                .as_ref()
                .and_then(|spec| spec.node_name.clone())
                .unwrap_or_default();
            let controller = pod
                .metadata
                .owner_references
                .as_ref()
                .and_then(|owners| {
                    owners
                        .iter()
                        .find(|owner| owner.controller.unwrap_or(false))
                })
                .map(|owner| format!("{}/{}", owner.kind, owner.name))
                .unwrap_or_default();

            let key = format!("{namespace}/{name}");
            let pct = usage
                .cpu
                .get(&key)
                .copied()
                .and_then(|used| usage_pct(used, pod_cpu_request(pod)));
            let mut row = Row::new(format!("pod/{namespace}/{name}"), name.clone())
                .with_namespace(namespace)
                .with_cell("status", status)
                .with_cell("ready", format!("{ready_count}/{total}"))
                .with_cell("restarts", restarts.to_string())
                .with_cell("ctrl", controller)
                .with_cell("node", node)
                .with_cell("qos", qos)
                .with_cell("age", age_of(pod.metadata.creation_timestamp.as_ref()));
            // A hidden trend column pauses the feed instead of collecting samples no one
            // draws; a pod without usage keeps no history at all.
            if let Some(series) = feed_trend(&key, pct.unwrap_or(0), visible && pct.is_some()) {
                if visible && pct.is_some() {
                    row = row.with_trend(series);
                }
            }
            row
        })
        .collect()
}

/// Node rows for the overview board's node widget. Percentages are *scheduled requests*
/// against allocatable, matching the capacity gauges; live usage needs `metrics-server`,
/// which this slice does not query.
fn node_rows(nodes: &[Node], pods: &[Pod]) -> Vec<NodeRow> {
    nodes
        .iter()
        .map(|node| {
            let name = node.metadata.name.clone().unwrap_or_default();
            let ready = node
                .status
                .as_ref()
                .and_then(|status| status.conditions.as_ref())
                .and_then(|conditions| {
                    conditions
                        .iter()
                        .find(|condition| condition.type_ == "Ready")
                })
                .map(|condition| condition.status == "True")
                .unwrap_or(false);

            let on_node = || {
                pods.iter().filter(|pod| {
                    pod.spec.as_ref().and_then(|spec| spec.node_name.as_deref())
                        == Some(name.as_str())
                })
            };
            let allocatable = |key: &str| -> f64 {
                node.status
                    .as_ref()
                    .and_then(|status| status.allocatable.as_ref())
                    .and_then(|entries| entries.get(key))
                    .and_then(|quantity| parse_quantity(&quantity.0))
                    .unwrap_or(0.0)
            };
            let pct = |used: f64, total: f64| -> u8 {
                if total <= 0.0 {
                    return 0;
                }
                ((used / total) * 100.0).round().clamp(0.0, 100.0) as u8
            };
            let cpu_pct = pct(on_node().map(pod_cpu_request).sum(), allocatable("cpu"));
            let mem_pct = pct(
                on_node().map(pod_memory_request).sum(),
                allocatable("memory"),
            );

            NodeRow {
                id: format!("node/{name}"),
                name,
                cpu_pct,
                mem_pct,
                note: if ready {
                    "Ready".to_owned()
                } else {
                    "NotReady".to_owned()
                },
                degraded: !ready,
            }
        })
        .collect()
}

/// Newest-first event rows, capped at 20, per the ticket.
fn event_rows(events: &[Event]) -> Vec<EventRow> {
    let mut sorted: Vec<&Event> = events.iter().collect();
    sorted.sort_by_key(|event| std::cmp::Reverse(event_time(event)));
    sorted
        .into_iter()
        .take(20)
        .map(|event| {
            let involved = &event.involved_object;
            let kind = involved.kind.clone().unwrap_or_default();
            let name = involved.name.clone().unwrap_or_default();
            let target = if kind.is_empty() || name.is_empty() {
                None
            } else {
                let lower = kind.to_lowercase();
                Some(match involved.namespace.as_deref() {
                    Some(ns) if !ns.is_empty() => format!("{lower}/{ns}/{name}"),
                    _ => format!("{lower}/{name}"),
                })
            };
            let object = if kind.is_empty() {
                name.clone()
            } else {
                format!("{kind}/{name}")
            };
            EventRow {
                target,
                kind: event.type_.clone().unwrap_or_default(),
                reason: event.reason.clone().unwrap_or_default(),
                object,
                message: event.message.clone().unwrap_or_default(),
                age: age_of(event_time_ref(event)),
            }
        })
        .collect()
}

/// The timestamp an event's age is measured from: last observation, then the event
/// object's own creation.
fn event_time_ref(event: &Event) -> Option<&Time> {
    event
        .last_timestamp
        .as_ref()
        .or(event.metadata.creation_timestamp.as_ref())
}

/// Owned copy of [`event_time_ref`], so sort keys do not borrow from the sorted slice.
fn event_time(event: &Event) -> Option<Time> {
    event_time_ref(event).cloned()
}

/// Age of an object from its creation (or last-observation) timestamp, kubectl-style:
/// `12s`, `5m`, `3h`, `9d`, `14d`, `2y`. Empty when the timestamp is absent.
fn age_of(time: Option<&Time>) -> String {
    let Some(time) = time else {
        return String::new();
    };
    let then = time.0.as_second();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or(then);
    units(now.saturating_sub(then).max(0))
}

/// Age of an aggregated row, whose last observation is milliseconds since the epoch.
fn age_of_millis(millis: Option<u64>) -> String {
    let Some(millis) = millis else {
        return String::new();
    };
    let now = openkite_engine::events::now_millis();
    units(now.saturating_sub(millis) as i64 / 1000)
}

/// First-to-last observation of an aggregated row, e.g. `40s`; empty when it never repeated.
fn span_of(first: Option<u64>, last: Option<u64>) -> String {
    match (first, last) {
        (Some(first), Some(last)) if last > first => units((last - first) as i64 / 1000),
        _ => String::new(),
    }
}

/// kubectl-style duration: `40s`, `5m`, `2h`, `3d`, `1y`.
fn units(seconds: i64) -> String {
    match seconds.max(0) {
        0..=59 => format!("{seconds}s"),
        60..=3599 => format!("{}m", seconds / 60),
        3600..=86_399 => format!("{}h", seconds / 3600),
        86_400..=31_535_999 => format!("{}d", seconds / 86_400),
        _ => format!("{}y", seconds / 31_536_000),
    }
}

/// Collapses raw events into the rows the events view renders: one row per cause inside the
/// aggregation window, newest first, carrying its occurrence count and observed span.
fn aggregated_event_rows(events: &[Event]) -> Vec<openkite_ui::inspector::Event> {
    let now = openkite_engine::events::now_millis();
    let mut aggregator = openkite_engine::events::EventAggregator::new();
    for event in events {
        aggregator.observe(event, now);
    }
    aggregator.expire(now);

    aggregator
        .rows()
        .into_iter()
        .filter_map(|row| {
            let openkite_engine::projection::RowStatus::Event(status) = row.status else {
                return None;
            };
            Some(openkite_ui::inspector::Event {
                kind: if status.type_.as_deref() == Some("Warning") {
                    "Warning"
                } else {
                    "Normal"
                },
                reason: status.reason.unwrap_or_default(),
                message: status.message.unwrap_or_default(),
                age: age_of_millis(status.last_seen),
                count: status.count.unwrap_or(1).max(0) as u32,
                span: span_of(status.first_seen, status.last_seen),
            })
        })
        .collect()
}

/// Cluster-wide pressure, measured as *scheduled requests* against node allocatable.
///
/// This is the accounting the scheduler itself uses, and it comes from the pod and node
/// lists already in hand — no `metrics-server` round trip. Live CPU/memory *usage* would
/// need `metrics.k8s.io`, which this slice deliberately does not query (see the ticket
/// report), so the usage meters and capacity gauges both speak in requests.
/// What the overview's charts draw from: the source that answered, the default band, and
/// one window per aggregate metric with this refresh appended. The windows live as long as
/// the process, so a second visit to the board has a second point to join.
fn metrics_view(
    pods: &[Pod],
    nodes: &[Node],
    node_usage: &Usage,
    pressure: &Pressure,
    usage_answered: bool,
) -> MetricsView {
    let mut probe = SourceProbe::default();
    for pod in pods {
        probe.observe(
            pod.metadata.namespace.as_deref().unwrap_or_default(),
            pod.metadata.name.as_deref().unwrap_or_default(),
            pod.status.as_ref().and_then(|status| status.phase.as_deref()) == Some("Running"),
        );
    }
    let source = if usage_answered {
        MetricSource::MetricsServer
    } else {
        probe.detect()
    };
    let mut view = MetricsView {
        source,
        ..MetricsView::default()
    };
    let cpu = aggregate_pct(nodes, node_usage, "cpu").unwrap_or_else(|| pressure.cpu_pct());
    let mem = aggregate_pct(nodes, node_usage, "memory").unwrap_or_else(|| pressure.mem_pct());
    view.offer("cpu", cpu);
    view.offer("mem", mem);
    view
}

/// Cluster usage as a share of allocatable, from `NodeMetrics` and the node list. `None`
/// when the cluster reports no allocatable total, which leaves the caller's fallback in
/// charge rather than plotting a fabricated zero.
fn aggregate_pct(nodes: &[Node], usage: &Usage, key: &str) -> Option<u8> {
    let mut used = 0.0;
    let mut total = 0.0;
    for node in nodes {
        let name = node.metadata.name.as_deref().unwrap_or_default();
        total += node
            .status
            .as_ref()
            .and_then(|status| status.allocatable.as_ref())
            .and_then(|entries| entries.get(key))
            .and_then(|quantity| parse_quantity(&quantity.0))
            .unwrap_or(0.0);
        let by_node = if key == "cpu" { &usage.cpu } else { &usage.mem };
        used += by_node.get(name).copied().unwrap_or(0.0);
    }
    if total <= 0.0 {
        return None;
    }
    Some(((used / total) * 100.0).round().clamp(0.0, 100.0) as u8)
}

struct Pressure {
    cpu_used: f64,
    cpu_total: f64,
    mem_used: f64,
    mem_total: f64,
    pod_slots: f64,
}

impl Pressure {
    fn read(nodes: &[Node], pods: &[Pod]) -> Self {
        let allocatable = |key: &str| -> f64 {
            nodes
                .iter()
                .filter_map(|node| node.status.as_ref())
                .filter_map(|status| status.allocatable.as_ref())
                .filter_map(|entries| entries.get(key))
                .filter_map(|quantity| parse_quantity(&quantity.0))
                .sum()
        };
        Pressure {
            cpu_used: pods.iter().map(pod_cpu_request).sum(),
            cpu_total: allocatable("cpu"),
            mem_used: pods.iter().map(pod_memory_request).sum(),
            mem_total: allocatable("memory"),
            pod_slots: allocatable("pods"),
        }
    }

    fn cpu_pct(&self) -> u8 {
        percent(self.cpu_used, self.cpu_total)
    }

    fn mem_pct(&self) -> u8 {
        percent(self.mem_used, self.mem_total)
    }

    /// One gauge per capacity kind the cluster reports; kinds with no allocatable total
    /// (a cluster that reports none, or a failed node list) are omitted rather than shown
    /// as `0 / 0`.
    fn gauges(&self, pods: usize) -> Vec<CapacityGauge> {
        [
            gauge(
                "CPU requests",
                format!("{:.1} / {:.0} cores", self.cpu_used, self.cpu_total),
                self.cpu_used,
                self.cpu_total,
            ),
            gauge(
                "Memory requests",
                format!(
                    "{:.0} / {:.0} GiB",
                    self.mem_used / GIB,
                    self.mem_total / GIB
                ),
                self.mem_used,
                self.mem_total,
            ),
            gauge(
                "Pods",
                format!("{} / {:.0}", pods, self.pod_slots),
                pods as f64,
                self.pod_slots,
            ),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Used-share of a total, rounded and clamped; `0` when the cluster reports no total.
fn percent(used: f64, total: f64) -> u8 {
    if total <= 0.0 {
        return 0;
    }
    ((used / total) * 100.0).round().clamp(0.0, 100.0) as u8
}

/// A gauge row, or `None` when the cluster reported no capacity to measure against.
fn gauge(label: &str, value: String, used: f64, total: f64) -> Option<CapacityGauge> {
    if total <= 0.0 {
        return None;
    }
    let pct = percent(used, total);
    Some(CapacityGauge {
        label: label.to_owned(),
        value,
        pct,
        tone: if pct >= 80 { "warn" } else { "" },
    })
}

/// CPU a pod has requested, in cores: the sum over app containers, floored by the
/// largest init container, which is how the scheduler accounts for it.
fn pod_cpu_request(pod: &Pod) -> f64 {
    let Some(spec) = pod.spec.as_ref() else {
        return 0.0;
    };
    let sum: f64 = spec.containers.iter().map(container_cpu).sum();
    let init = spec
        .init_containers
        .iter()
        .flatten()
        .map(container_cpu)
        .fold(0.0_f64, f64::max);
    sum.max(init)
}

/// Memory a pod has requested, in bytes; same accounting as [`pod_cpu_request`].
fn pod_memory_request(pod: &Pod) -> f64 {
    let Some(spec) = pod.spec.as_ref() else {
        return 0.0;
    };
    let sum: f64 = spec.containers.iter().map(container_memory).sum();
    let init = spec
        .init_containers
        .iter()
        .flatten()
        .map(container_memory)
        .fold(0.0_f64, f64::max);
    sum.max(init)
}

fn container_cpu(container: &k8s_openapi::api::core::v1::Container) -> f64 {
    container
        .resources
        .as_ref()
        .and_then(|resources| resources.requests.as_ref())
        .and_then(|requests| requests.get("cpu"))
        .and_then(|quantity| parse_quantity(&quantity.0))
        .unwrap_or(0.0)
}

fn container_memory(container: &k8s_openapi::api::core::v1::Container) -> f64 {
    container
        .resources
        .as_ref()
        .and_then(|resources| resources.requests.as_ref())
        .and_then(|requests| requests.get("memory"))
        .and_then(|quantity| parse_quantity(&quantity.0))
        .unwrap_or(0.0)
}

/// Parses a Kubernetes resource quantity into base units (cores, bytes). Returns `None`
/// for anything unparseable so a stray value counts as zero instead of poisoning a total.
fn parse_quantity(raw: &str) -> Option<f64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(value) = raw.parse::<f64>() {
        return Some(value);
    }
    let split = raw
        .find(|character: char| character.is_ascii_alphabetic())
        .unwrap_or(raw.len());
    let (number, suffix) = raw.split_at(split);
    let value: f64 = number.parse().ok()?;
    let multiplier = match suffix {
        "n" => 1e-9,
        "u" => 1e-6,
        "m" => 1e-3,
        "k" => 1e3,
        "M" => 1e6,
        "G" => 1e9,
        "T" => 1e12,
        "P" => 1e15,
        "E" => 1e18,
        "Ki" => 1024.0,
        "Mi" => 1024.0f64.powi(2),
        "Gi" => GIB,
        "Ti" => 1024.0f64.powi(4),
        "Pi" => 1024.0f64.powi(5),
        "Ei" => 1024.0f64.powi(6),
        _ => return None,
    };
    Some(value * multiplier)
}

/// Pod count per namespace, from a namespace list plus the pod list already in hand.
fn namespace_counts(namespaces: &[Namespace], pods: &[Pod]) -> Vec<(String, usize)> {
    namespaces
        .iter()
        .filter_map(|namespace| namespace.metadata.name.clone())
        .map(|name| {
            let count = pods
                .iter()
                .filter(|pod| pod.metadata.namespace.as_deref() == Some(name.as_str()))
                .count();
            (name, count)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{Container, PodSpec};

    fn pod_with_cpu(cpu: &str) -> Pod {
        let container = Container {
            name: "app".to_owned(),
            resources: Some(k8s_openapi::api::core::v1::ResourceRequirements {
                requests: Some(std::collections::BTreeMap::from([(
                    "cpu".to_owned(),
                    k8s_openapi::apimachinery::pkg::api::resource::Quantity(cpu.to_owned()),
                )])),
                ..Default::default()
            }),
            ..Default::default()
        };
        Pod {
            spec: Some(PodSpec {
                containers: vec![container],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn quantities_parse_to_base_units() {
        assert_eq!(parse_quantity("250m"), Some(0.25));
        assert_eq!(parse_quantity("2"), Some(2.0));
        assert_eq!(parse_quantity("1Gi"), Some(1024.0 * 1024.0 * 1024.0));
        assert_eq!(parse_quantity("1.5"), Some(1.5));
        assert_eq!(parse_quantity("bogus"), None);
        assert_eq!(parse_quantity(""), None);
    }

    #[test]
    fn pod_requests_sum_across_containers() {
        let pod = pod_with_cpu("250m");
        assert_eq!(pod_cpu_request(&pod), 0.25);
        assert_eq!(pod_memory_request(&pod), 0.0);
    }

    #[test]
    fn ages_render_in_kubectl_units() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock before the epoch")
            .as_secs() as i64;
        assert_eq!(age_of(None), "");
        for (seconds, expected) in [(30, "30s"), (300, "5m"), (7_200, "2h"), (172_800, "2d")] {
            let time = Time(k8s_openapi::jiff::Timestamp::from_second(now - seconds).unwrap());
            assert_eq!(age_of(Some(&time)), expected);
        }
    }

    #[test]
    fn repeated_events_reach_the_view_as_one_counted_row() {
        let now = openkite_engine::events::now_millis();
        let events: Vec<Event> = (0..3)
            .map(|index| Event {
                metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                    name: Some(format!("cache-2f1a-uvwxy.{index:04}")),
                    namespace: Some("demo".to_owned()),
                    uid: Some(format!("ev-backoff-{index}")),
                    ..Default::default()
                },
                type_: Some("Warning".to_owned()),
                reason: Some("BackOff".to_owned()),
                message: Some("Back-off restarting failed container app".to_owned()),
                count: Some(1),
                involved_object: k8s_openapi::api::core::v1::ObjectReference {
                    kind: Some("Pod".to_owned()),
                    name: Some("cache-2f1a-uvwxy".to_owned()),
                    namespace: Some("demo".to_owned()),
                    uid: Some("pod-uid".to_owned()),
                    ..Default::default()
                },
                first_timestamp: Some(Time(
                    k8s_openapi::jiff::Timestamp::from_millisecond((now - 120_000) as i64)
                        .expect("timestamp"),
                )),
                last_timestamp: Some(Time(
                    k8s_openapi::jiff::Timestamp::from_millisecond((now - index * 30_000) as i64)
                        .expect("timestamp"),
                )),
                ..Event::default()
            })
            .collect();

        let rows = aggregated_event_rows(&events);
        assert_eq!(rows.len(), 1, "one cause, one row");
        assert_eq!(rows[0].count, 3);
        assert_eq!(rows[0].kind, "Warning");
        assert_eq!(rows[0].span, "2m", "first to last observation");
        assert_eq!(rows[0].age, "0s");
    }

    #[test]
    fn gauges_skip_kinds_the_cluster_does_not_report() {
        assert!(gauge("Pods", "0 / 0".to_owned(), 0.0, 0.0).is_none());
        let full = gauge("Pods", "9 / 10".to_owned(), 9.0, 10.0).expect("gauge");
        assert_eq!(full.pct, 90);
        assert_eq!(full.tone, "warn");
    }
}
