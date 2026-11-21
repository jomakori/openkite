//! Live cluster adapter: turns `Engine` queries into the view-models `openkite-ui` renders.
//!
//! Every list call gets its own timeout and turns a failure into an empty result for that
//! slice, so one flaky query degrades a widget instead of failing the whole page.

use std::sync::Arc;
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
    /// and serves an empty console instead of refusing to boot without a cluster.
    pub async fn connect() -> Self {
        match Engine::connect(None).await {
            Ok(engine) => Live::Connected(Arc::new(engine)),
            Err(error) => {
                tracing::warn!(%error, "engine connect failed; serving an empty console");
                Live::Unavailable
            }
        }
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

    /// Board data for `/`. See module docs for the per-field timeout/fallback behaviour.
    pub async fn overview(&self) -> OverviewData {
        let Live::Connected(engine) = self else {
            return OverviewData::from_counts("unavailable", &Counts::new());
        };
        let client = engine.client();

        // One round-trip window for all six lists: `join!` keeps a slow kind from
        // stacking its timeout on top of the others.
        let (pods, nodes_raw, namespaces_raw, events_raw, deployments, argo) = tokio::join!(
            timed(list_all::<Pod>(client.clone())),
            timed(list_all::<Node>(client.clone())),
            timed(list_all::<Namespace>(client.clone())),
            timed(list_all::<Event>(client.clone())),
            timed(list_all::<Deployment>(client.clone())),
            timed(fetch_argo_apps(client.clone())),
        );
        let pods = pods.unwrap_or_default();
        let nodes_raw = nodes_raw.unwrap_or_default();
        let namespaces_raw = namespaces_raw.unwrap_or_default();
        let events_raw = events_raw.unwrap_or_default();
        let deployments = deployments.unwrap_or_default();
        let argo = argo.unwrap_or_default();

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
            .with_pods(pod_rows(&pods), threshold);
        data.nodes = node_rows(&nodes_raw, &pods);
        data.argo = argo;
        data.events = event_rows(&events_raw);
        data.namespaces = namespace_counts(&namespaces_raw, &pods);
        data.capacity = pressure.gauges(pods.len());
        data
    }

    /// One row per pod for `/table`.
    pub async fn table(&self) -> Vec<Row> {
        let Live::Connected(engine) = self else {
            return Vec::new();
        };
        let pods = timed(list_all::<Pod>(engine.client()))
            .await
            .unwrap_or_default();
        pod_rows(&pods)
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
fn dynamic_str(item: &DynamicObject, path: &[&str]) -> Option<String> {
    let mut value = &item.data;
    for key in path {
        value = value.get(key)?;
    }
    value.as_str().map(str::to_owned)
}

/// Pod rows for both the overview board and the pods table. Columns mirror
/// `crates/openkite-ui/src/table_columns.rs`'s `pods` entry; no new columns invented.
fn pod_rows(pods: &[Pod]) -> Vec<Row> {
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

            Row::new(format!("pod/{namespace}/{name}"), name.clone())
                .with_namespace(namespace)
                .with_cell("status", status)
                .with_cell("ready", format!("{ready_count}/{total}"))
                .with_cell("restarts", restarts.to_string())
                .with_cell("ctrl", controller)
                .with_cell("node", node)
                .with_cell("qos", qos)
                .with_cell("age", age_of(pod.metadata.creation_timestamp.as_ref()))
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
    let delta = now.saturating_sub(then).max(0);
    match delta {
        0..=59 => format!("{delta}s"),
        60..=3599 => format!("{}m", delta / 60),
        3600..=86_399 => format!("{}h", delta / 3600),
        86_400..=31_535_999 => format!("{}d", delta / 86_400),
        _ => format!("{}y", delta / 31_536_000),
    }
}

/// Cluster-wide pressure, measured as *scheduled requests* against node allocatable.
///
/// This is the accounting the scheduler itself uses, and it comes from the pod and node
/// lists already in hand — no `metrics-server` round trip. Live CPU/memory *usage* would
/// need `metrics.k8s.io`, which this slice deliberately does not query (see the ticket
/// report), so the usage meters and capacity gauges both speak in requests.
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
    fn gauges_skip_kinds_the_cluster_does_not_report() {
        assert!(gauge("Pods", "0 / 0".to_owned(), 0.0, 0.0).is_none());
        let full = gauge("Pods", "9 / 10".to_owned(), 9.0, 10.0).expect("gauge");
        assert_eq!(full.pct, 90);
        assert_eq!(full.tone, "warn");
    }
}
