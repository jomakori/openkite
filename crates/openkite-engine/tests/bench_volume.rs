//! Deterministic volume fixture plus the harness that reproduces the design's
//! frame-rate and memory budgets.
//! Ignored by default (~20s wall clock to reproduce the real event pacing); the CI
//! benchmark job runs it explicitly, in release mode, against this in-process fixture.
use std::collections::BTreeMap;
use std::fs;
use std::time::{Duration, Instant};

use k8s_openapi::api::core::v1::{
    Container, ContainerState, ContainerStateRunning, ContainerStatus, Pod, PodSpec, PodStatus,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, ListParams};
use kube::core::{ApiResource, DynamicObject, GroupVersionKind};
use tokio::sync::mpsc;

use openkite_engine::coalesce::{self, CoalescerInput, Frame};
use openkite_engine::interest::InterestClass;
use openkite_engine::projection::{project_metadata_only, project_pod, Row, RowMeta, RowStatus};
use openkite_engine_testkit::{FakeApiserver, ResourceDef, Scenario};

/// Fixed seed so the fixture is byte-for-byte identical on every run.
const SEED: u64 = 0x0218_0218_0218_0218;

const POD_COUNT: usize = 5_000;
const TOTAL_OBJECTS: usize = 40_000;
const BURST_EVENTS: usize = 5_000;
const BURST_SECONDS: u64 = 20;

/// §5.3 claim: ~160 coalesced frames for the burst, ±20%.
const FRAME_RANGE: (usize, usize) = (128, 192);
/// §5.3 claim: ~1.5 MB held for 5,000 pod projections, ±20%.
const PROJECTION_BYTES_RANGE: (u64, u64) = (1_200_000, 1_800_000);
/// §5.3 claim: pinned/background layer for the non-visible kinds stays under ~40 MB (+20%).
const PINNED_LAYER_MAX_BYTES: u64 = 48 * 1024 * 1024;

/// The 24 kinds backing the "24 active informers" claim; Pod is the one visible kind.
const KINDS: [(&str, &str, &str, &str, bool); 24] = [
    ("", "v1", "Pod", "pods", true),
    ("apps", "v1", "Deployment", "deployments", true),
    ("apps", "v1", "ReplicaSet", "replicasets", true),
    ("apps", "v1", "DaemonSet", "daemonsets", true),
    ("apps", "v1", "StatefulSet", "statefulsets", true),
    ("", "v1", "Service", "services", true),
    ("", "v1", "ConfigMap", "configmaps", true),
    ("", "v1", "Secret", "secrets", true),
    ("", "v1", "Endpoints", "endpoints", true),
    ("", "v1", "Event", "events", true),
    ("", "v1", "Node", "nodes", false),
    ("", "v1", "PersistentVolume", "persistentvolumes", false),
    (
        "",
        "v1",
        "PersistentVolumeClaim",
        "persistentvolumeclaims",
        true,
    ),
    ("", "v1", "ServiceAccount", "serviceaccounts", true),
    ("batch", "v1", "Job", "jobs", true),
    ("batch", "v1", "CronJob", "cronjobs", true),
    ("networking.k8s.io", "v1", "Ingress", "ingresses", true),
    (
        "networking.k8s.io",
        "v1",
        "NetworkPolicy",
        "networkpolicies",
        true,
    ),
    ("rbac.authorization.k8s.io", "v1", "Role", "roles", true),
    (
        "rbac.authorization.k8s.io",
        "v1",
        "RoleBinding",
        "rolebindings",
        true,
    ),
    (
        "rbac.authorization.k8s.io",
        "v1",
        "ClusterRole",
        "clusterroles",
        false,
    ),
    (
        "rbac.authorization.k8s.io",
        "v1",
        "ClusterRoleBinding",
        "clusterrolebindings",
        false,
    ),
    (
        "autoscaling",
        "v2",
        "HorizontalPodAutoscaler",
        "horizontalpodautoscalers",
        true,
    ),
    (
        "policy",
        "v1",
        "PodDisruptionBudget",
        "poddisruptionbudgets",
        true,
    ),
];

/// Tiny deterministic PRNG (splitmix64); avoids adding a `rand` dependency for a seeded fixture.
struct Rng(u64);

impl Rng {
    /// Next pseudo-random 64-bit value.
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A pseudo-random index in `[0, bound)`.
    fn next_index(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

/// Builds a full Pod object padded with a last-applied-configuration blob, like a real one.
fn build_pod(index: usize, rng: &mut Rng) -> Pod {
    let mut annotations = BTreeMap::new();
    annotations.insert(
        "kubectl.kubernetes.io/last-applied-configuration".to_owned(),
        format!("managed-fields-simulated-{index}-").repeat(400),
    );
    Pod {
        metadata: ObjectMeta {
            uid: Some(format!("pod-uid-{index}")),
            name: Some(format!("pod-{index}")),
            namespace: Some(format!("ns-{}", rng.next_index(50))),
            annotations: Some(annotations),
            ..Default::default()
        },
        spec: Some(PodSpec {
            node_name: Some(format!("node-{}", rng.next_index(200))),
            containers: vec![Container {
                name: "app".to_owned(),
                image: Some("registry.example/app:bench".to_owned()),
                ..Default::default()
            }],
            ..Default::default()
        }),
        status: Some(PodStatus {
            phase: Some("Running".to_owned()),
            container_statuses: Some(vec![ContainerStatus {
                name: "app".to_owned(),
                image: "registry.example/app:bench".to_owned(),
                image_id: String::new(),
                ready: true,
                restart_count: 0,
                state: Some(ContainerState {
                    running: Some(ContainerStateRunning { started_at: None }),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        }),
    }
}

/// Builds a metadata-only object for one of the 23 pinned/background kinds.
fn build_other_object(
    kind_idx: usize,
    index: usize,
    rng: &mut Rng,
) -> (ObjectMeta, serde_json::Value) {
    let (group, version, kind, _plural, namespaced) = KINDS[kind_idx];
    let mut labels = BTreeMap::new();
    labels.insert("app".to_owned(), format!("bench-{}", rng.next_index(20)));
    let meta = ObjectMeta {
        uid: Some(format!("{kind}-uid-{index}")),
        name: Some(format!("{}-{index}", kind.to_lowercase())),
        namespace: if namespaced {
            Some(format!("ns-{}", rng.next_index(50)))
        } else {
            None
        },
        labels: Some(labels),
        ..Default::default()
    };
    let api_version = if group.is_empty() {
        version.to_owned()
    } else {
        format!("{group}/{version}")
    };
    let value = serde_json::json!({
        "apiVersion": api_version,
        "kind": kind,
        "metadata": meta,
    });
    (meta, value)
}

/// Seeds a `Scenario` with every pod and background object, driving the real store below.
fn build_scenario(
    pods: &[Pod],
    others: &[(usize, ObjectMeta, serde_json::Value)],
) -> (Scenario, Vec<ResourceDef>) {
    let defs: Vec<ResourceDef> = KINDS
        .iter()
        .map(|(group, version, kind, plural, namespaced)| {
            ResourceDef::new(group, version, kind, plural, *namespaced)
        })
        .collect();
    let mut scenario = Scenario::new();
    for def in &defs {
        scenario = scenario.with_resource(def.clone());
    }
    for pod in pods {
        scenario = scenario.add(&defs[0], pod.clone());
    }
    for (kind_idx, _meta, value) in others {
        scenario = scenario.add(&defs[*kind_idx], value.clone());
    }
    (scenario, defs)
}

/// Lists every seeded kind back over real HTTP against the fake apiserver and sums the count.
async fn count_objects_via_store(server: &FakeApiserver, defs: &[ResourceDef]) -> usize {
    let client = server.client();
    let mut total = 0usize;
    for def in defs {
        let gvk = GroupVersionKind {
            group: def.group.clone(),
            version: def.version.clone(),
            kind: def.kind.clone(),
        };
        let resource = ApiResource::from_gvk_with_plural(&gvk, &def.plural);
        let api: Api<DynamicObject> = Api::all_with(client.clone(), &resource);
        let mut params = ListParams::default();
        loop {
            let list = api
                .list(&params)
                .await
                .unwrap_or_else(|err| panic!("list {} via fake apiserver: {err}", def.kind));
            let page_len = list.items.len();
            total += page_len;
            if page_len == 0 {
                break;
            }
            match list.metadata.continue_.as_deref() {
                Some(token) if !token.is_empty() => {
                    params = ListParams::default().continue_token(token);
                }
                _ => break,
            }
        }
    }
    total
}

/// A minimal upsert row carrying only the burst index needed to pair it with its push time.
fn burst_row(index: usize) -> Row {
    Row {
        meta: RowMeta {
            uid: format!("burst-{index}"),
            name: format!("burst-{index}"),
            namespace: None,
            kind: "Pod".to_owned(),
            created: None,
            labels_hash: 0,
            resource_version: None,
        },
        status: RowStatus::Metadata,
    }
}

/// Recovers the burst index from a row uid produced by `burst_row`.
fn parse_burst_uid(uid: &str) -> Option<usize> {
    uid.strip_prefix("burst-")
        .and_then(|rest| rest.parse().ok())
}

/// The 99th-percentile sample, or zero when there were none.
fn percentile_99(samples: &mut [Duration]) -> Duration {
    if samples.is_empty() {
        return Duration::ZERO;
    }
    samples.sort();
    let index = ((samples.len() as f64) * 0.99).ceil() as usize;
    samples[index.saturating_sub(1).min(samples.len() - 1)]
}

/// Paces 5,000 unique upserts over `BURST_SECONDS` (the §5.3 rollout rate) through the real
/// coalescer, draining frames as they arrive, and returns the frame count and event-to-frame p99.
async fn push_and_drain(
    input: CoalescerInput,
    mut frame_rx: mpsc::Receiver<Frame>,
) -> (usize, Duration) {
    let start = Instant::now();
    let interval = Duration::from_secs(BURST_SECONDS) / BURST_EVENTS as u32;
    let safety_deadline = Duration::from_secs(BURST_SECONDS + 15);

    let mut pushed_at = Vec::with_capacity(BURST_EVENTS);
    let mut latencies = Vec::with_capacity(BURST_EVENTS);
    let mut frames = Vec::new();
    let mut pushed = 0usize;
    let mut next_push_at = Duration::ZERO;

    while start.elapsed() < safety_deadline {
        if pushed < BURST_EVENTS && start.elapsed() >= next_push_at {
            pushed_at.push(Instant::now());
            input.push(coalesce::CoalesceInput::Upsert(Box::new(burst_row(pushed))));
            pushed += 1;
            next_push_at += interval;
            continue;
        }
        if pushed == BURST_EVENTS && latencies.len() >= BURST_EVENTS {
            break;
        }
        let wait = if pushed < BURST_EVENTS {
            next_push_at
                .saturating_sub(start.elapsed())
                .max(Duration::from_millis(1))
        } else {
            Duration::from_millis(200)
        };
        match tokio::time::timeout(wait, frame_rx.recv()).await {
            Ok(Some(frame)) => {
                for row in &frame.upsert {
                    if let Some(index) = parse_burst_uid(&row.meta.uid) {
                        latencies.push(Instant::now().duration_since(pushed_at[index]));
                    }
                }
                frames.push(frame);
            }
            Ok(None) => break,
            Err(_) => {}
        }
    }

    (frames.len(), percentile_99(&mut latencies))
}

/// Simulated "input latency": scheduler responsiveness to a periodic 1ms tick while the
/// burst above runs concurrently. §5.3 states no number for this; it is printed, never asserted.
async fn ping_during(duration: Duration) -> Duration {
    let start = Instant::now();
    let mut jitters = Vec::new();
    while start.elapsed() < duration {
        let before = Instant::now();
        tokio::time::sleep(Duration::from_millis(1)).await;
        jitters.push(before.elapsed().saturating_sub(Duration::from_millis(1)));
        tokio::time::sleep(Duration::from_millis(49)).await;
    }
    percentile_99(&mut jitters)
}

/// Runs the burst and the input-latency proxy concurrently; returns (frames, p99, input p99).
async fn run_burst() -> (usize, Duration, Duration) {
    let (input, frame_rx) = coalesce::spawn(InterestClass::Visible);
    let (burst_result, input_p99) = tokio::join!(
        push_and_drain(input, frame_rx),
        ping_during(Duration::from_secs(BURST_SECONDS))
    );
    (burst_result.0, burst_result.1, input_p99)
}

/// Process RSS in bytes, read from `/proc/self/statm`; assumes the common 4 KiB Linux page size.
fn read_rss_bytes() -> u64 {
    const PAGE_SIZE: u64 = 4096;
    fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|statm| statm.split_whitespace().nth(1).map(str::to_owned))
        .and_then(|pages| pages.parse::<u64>().ok())
        .map(|pages| pages * PAGE_SIZE)
        .unwrap_or(0)
}

fn pass_str(pass: bool) -> &'static str {
    if pass {
        "PASS"
    } else {
        "FAIL"
    }
}

#[tokio::test]
#[ignore = "reproduces real 250 events/sec pacing; takes ~20s, run explicitly in CI"]
async fn volume_benchmark_reproduces_section_5_3() {
    let mut rng = Rng(SEED);
    let pods: Vec<Pod> = (0..POD_COUNT)
        .map(|index| build_pod(index, &mut rng))
        .collect();

    let other_kind_count = KINDS.len() - 1;
    let remaining = TOTAL_OBJECTS - POD_COUNT;
    let base = remaining / other_kind_count;
    let extra = remaining % other_kind_count;
    let mut others = Vec::with_capacity(remaining);
    let mut global_index = 0usize;
    for kind_idx in 1..KINDS.len() {
        let count = base + usize::from((kind_idx - 1) < extra);
        for _ in 0..count {
            let (meta, value) = build_other_object(kind_idx, global_index, &mut rng);
            others.push((kind_idx, meta, value));
            global_index += 1;
        }
    }
    assert_eq!(
        pods.len() + others.len(),
        TOTAL_OBJECTS,
        "fixture must total 40,000 objects"
    );

    let rss_before = read_rss_bytes();

    let (scenario, defs) = build_scenario(&pods, &others);
    let server = FakeApiserver::start(scenario).await;
    let stored_total = count_objects_via_store(&server, &defs).await;
    server.shutdown().await;
    assert_eq!(
        stored_total, TOTAL_OBJECTS,
        "the real store must serve back every seeded object"
    );

    let projection_bytes: u64 = pods
        .iter()
        .map(|pod| {
            serde_json::to_vec(&project_pod(pod))
                .expect("row serializes")
                .len() as u64
        })
        .sum();
    let full_object_bytes: u64 = pods
        .iter()
        .map(|pod| serde_json::to_vec(pod).expect("pod serializes").len() as u64)
        .sum();

    let pinned_layer_bytes: u64 = others
        .iter()
        .map(|(kind_idx, meta, _)| {
            let kind = KINDS[*kind_idx].2;
            serde_json::to_vec(&project_metadata_only(meta, kind))
                .expect("row serializes")
                .len() as u64
        })
        .sum();

    let (frame_count, event_to_frame_p99, input_latency_p99) = run_burst().await;

    let rss_after = read_rss_bytes();

    let frame_pass = (FRAME_RANGE.0..=FRAME_RANGE.1).contains(&frame_count);
    let projection_pass =
        (PROJECTION_BYTES_RANGE.0..=PROJECTION_BYTES_RANGE.1).contains(&projection_bytes);
    let pinned_pass = pinned_layer_bytes < PINNED_LAYER_MAX_BYTES;

    println!("== volume benchmark (fixed seed {SEED:#x}) ==");
    println!(
        "burst frames:        {frame_count} (in-range {}-{}: {})",
        FRAME_RANGE.0,
        FRAME_RANGE.1,
        pass_str(frame_pass)
    );
    println!(
        "pod projections:     {projection_bytes} bytes (in-range {}-{}: {}); full objects (not asserted): {full_object_bytes} bytes",
        PROJECTION_BYTES_RANGE.0,
        PROJECTION_BYTES_RANGE.1,
        pass_str(projection_pass)
    );
    println!(
        "pinned/background:   {pinned_layer_bytes} bytes (under {PINNED_LAYER_MAX_BYTES}: {})",
        pass_str(pinned_pass)
    );
    println!("event-to-frame p99:  {event_to_frame_p99:?} (no §5.3 threshold; printed only)");
    println!(
        "input latency p99:   {input_latency_p99:?} (simulated scheduler proxy; no §5.3 threshold; printed only)"
    );
    println!("process RSS before:  {rss_before} bytes");
    println!("process RSS after:   {rss_after} bytes (diagnostic only; not used for the §5.3 byte claims above)");

    assert!(
        frame_pass,
        "frame count {frame_count} outside {FRAME_RANGE:?}"
    );
    assert!(
        projection_pass,
        "projection bytes {projection_bytes} outside {PROJECTION_BYTES_RANGE:?}"
    );
    assert!(
        pinned_pass,
        "pinned/background bytes {pinned_layer_bytes} >= {PINNED_LAYER_MAX_BYTES}"
    );
}
