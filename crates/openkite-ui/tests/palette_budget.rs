//! The ticket's budget: 40,000 objects must fit in under 80 MB and answer in under 80 ms.
//!
//! Memory is measured by a counting allocator, not estimated: the delta around `Palette::build`
//! is what the index really costs, arena, token map and postings included.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use openkite_ui::palette::{Entry, Palette};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let out = unsafe { System.realloc(ptr, layout, new_size) };
        if !out.is_null() {
            LIVE.fetch_add(new_size, Ordering::Relaxed);
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        out
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

const OBJECTS: usize = 40_000;
const MEMORY_LIMIT: usize = 80 * 1024 * 1024;
const LATENCY_LIMIT: Duration = Duration::from_millis(80);

/// A cluster-shaped fixture: mostly pods, a long tail of deployments, nodes and events, spread
/// over namespaces, each with the labels a real object carries.
fn fixture() -> Vec<Entry> {
    let namespaces = [
        "prod",
        "staging",
        "kube-system",
        "ingress-nginx",
        "cert-manager",
        "observability",
        "data",
        "batch",
        "argo",
        "monitoring",
        "platform",
        "dev",
    ];
    let apps = [
        "api",
        "worker",
        "gateway",
        "checkout",
        "catalog",
        "identity",
        "billing",
        "search",
        "notify",
        "ledger",
        "export",
        "scheduler",
    ];
    let hashes = [
        "7d9c4f8b6",
        "6b8f7d5c4",
        "5d78c9869d",
        "849c9b7b8",
        "7f4c8d6b9",
    ];

    let mut entries = Vec::with_capacity(OBJECTS);
    for i in 0..OBJECTS {
        let ns = namespaces[i % namespaces.len()];
        let app = apps[(i / 7) % apps.len()];
        let hash = hashes[i % hashes.len()];
        let tail = format!("{:05x}", i * 2654435761u64 as usize % 0xff_ffff);
        if i % 20 == 0 {
            entries.push(Entry::new("deployment", ns, app).with_labels(&[
                &format!("app={app}"),
                &format!("tier={}", if i % 40 == 0 { "web" } else { "batch" }),
                "managed-by=argo",
            ]));
        } else if i % 97 == 0 {
            entries.push(
                Entry::new(
                    "node",
                    "",
                    &format!("ip-10-0-{}-{}.eu-west-1.compute.internal", i % 256, i % 17),
                )
                .with_labels(&["role=worker", "pool=general"]),
            );
        } else if i % 31 == 0 {
            entries.push(
                Entry::new(
                    "event",
                    ns,
                    &format!("BackOff {app}-{hash}-{tail} restarting container"),
                )
                .with_labels(&[&format!(
                    "reason={}",
                    if i % 62 == 0 { "BackOff" } else { "Pulled" }
                )]),
            );
        } else {
            entries.push(
                Entry::new("pod", ns, &format!("{app}-{hash}-{tail}")).with_labels(&[
                    &format!("app={app}"),
                    &format!("pod-template-hash={hash}"),
                    &format!("team={}", apps[(i / 11) % apps.len()]),
                ]),
            );
        }
    }
    entries
}

#[test]
fn forty_thousand_objects_fit_the_memory_and_latency_budget() {
    let entries = fixture();
    assert_eq!(entries.len(), OBJECTS);

    // Warm the allocator so the delta is the index, not the first-touch bookkeeping.
    drop(Palette::build(entries.iter().take(100).cloned()));

    let before = LIVE.load(Ordering::Relaxed);
    let built = Instant::now();
    let palette = Palette::build(entries);
    let build_time = built.elapsed();
    let after = LIVE.load(Ordering::Relaxed);
    let index_bytes = after.saturating_sub(before);

    assert_eq!(palette.len(), OBJECTS, "every object is indexed");

    let queries = [
        "api",
        "worker-6b8f",
        "7d9c4f8b6",
        "app=api",
        "team=billing",
        "ns=prod api",
        "kind=deployment",
        "catalog",
        "ip-10-0",
        "prod",
        ">pods",
        "nothing-matches-this",
    ];

    // Every prefix length of a realistic name, which is the worst case for a range scan.
    let mut worst = Duration::ZERO;
    let mut worst_query = "";
    for q in queries {
        let started = Instant::now();
        let hits = palette.query(q);
        let took = started.elapsed();
        if took > worst {
            worst = took;
            worst_query = q;
        }
        assert!(took < LATENCY_LIMIT, "{q:?} took {took:?}");
        if q != "nothing-matches-this" && !q.starts_with('>') {
            assert!(!hits.is_empty(), "{q:?} matched nothing");
        }
    }
    for cut in 1..=8 {
        let q = &"worker-6b8f7d5c4"[..cut];
        let started = Instant::now();
        let hits = palette.query(q);
        let took = started.elapsed();
        if took > worst {
            worst = took;
            worst_query = q;
        }
        assert!(!hits.is_empty(), "prefix {q:?} matched nothing");
    }

    println!(
        "objects={OBJECTS} index={} KiB build={build_time:?} worst_query={worst:?} ({worst_query})",
        index_bytes / 1024
    );

    assert!(
        index_bytes < MEMORY_LIMIT,
        "index cost {} MiB, budget {} MiB",
        index_bytes / (1024 * 1024),
        MEMORY_LIMIT / (1024 * 1024)
    );
    assert!(
        index_bytes < palette.heap_bytes() * 2,
        "measured {} B vs estimate {} B — the estimate is not tracking the real cost",
        index_bytes,
        palette.heap_bytes()
    );
    assert!(
        worst < LATENCY_LIMIT,
        "worst query {worst_query:?} took {worst:?}"
    );
}
