//! Catalogue model: one entry per served GVK, and its TTL-cached, single-flight refresh.
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use kube::core::discovery::v2::GroupVersionKind;
use kube::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AsyncMutex;

use crate::discovery;
use crate::error::EngineError;

/// The TTL a cached catalogue is trusted for before `get_or_refresh` fetches again.
pub const DEFAULT_TTL: Duration = Duration::from_secs(600);

/// One `additionalPrinterColumns` entry: display name, type, JSONPath and priority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub column_type: String,
    pub json_path: String,
    pub priority: i32,
}

/// One served GVK: its identity, scope, verbs, and display metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueEntry {
    pub group: String,
    pub version: String,
    pub kind: String,
    pub plural: String,
    pub singular: String,
    pub namespaced: bool,
    pub verbs: Vec<String>,
    pub short_names: Vec<String>,
    pub categories: Vec<String>,
    pub additional_printer_columns: Vec<PrinterColumn>,
}

/// The served API surface: one `CatalogueEntry` per GVK.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Catalogue {
    pub entries: Vec<CatalogueEntry>,
}

impl Catalogue {
    /// The entry matching a group/version/kind triple, if the server serves it.
    pub fn find(&self, group: &str, version: &str, kind: &str) -> Option<&CatalogueEntry> {
        self.entries
            .iter()
            .find(|entry| entry.group == group && entry.version == version && entry.kind == kind)
    }
}

/// Printer columns for a GVK: the server's own columns, or a name/namespace/age fallback.
pub fn columns_for(catalogue: &Catalogue, gvk: &GroupVersionKind) -> Vec<PrinterColumn> {
    let group = gvk.group.as_deref().unwrap_or_default();
    let version = gvk.version.as_deref().unwrap_or_default();
    let kind = gvk.kind.as_deref().unwrap_or_default();
    match catalogue.find(group, version, kind) {
        Some(entry) if !entry.additional_printer_columns.is_empty() => {
            entry.additional_printer_columns.clone()
        }
        Some(entry) => fallback_columns(entry.namespaced),
        None => fallback_columns(true),
    }
}

fn fallback_columns(namespaced: bool) -> Vec<PrinterColumn> {
    let mut columns = vec![PrinterColumn {
        name: "Name".to_owned(),
        column_type: "string".to_owned(),
        json_path: ".metadata.name".to_owned(),
        priority: 0,
    }];
    if namespaced {
        columns.push(PrinterColumn {
            name: "Namespace".to_owned(),
            column_type: "string".to_owned(),
            json_path: ".metadata.namespace".to_owned(),
            priority: 0,
        });
    }
    columns.push(PrinterColumn {
        name: "Age".to_owned(),
        column_type: "date".to_owned(),
        json_path: ".metadata.creationTimestamp".to_owned(),
        priority: 0,
    });
    columns
}

/// A cached catalogue, the refresh generation it came from, and when it was fetched.
pub struct CatalogueSnapshot {
    pub catalogue: Arc<Catalogue>,
    pub generation: u64,
    fetched_at: Instant,
}

impl CatalogueSnapshot {
    /// Time elapsed since this snapshot was fetched, for UI staleness display.
    pub fn age(&self) -> Duration {
        self.fetched_at.elapsed()
    }
}

#[derive(Default)]
struct Slot {
    snapshot: Option<Arc<CatalogueSnapshot>>,
    generation: u64,
}

/// TTL cache of catalogues keyed by cluster identity, with single-flight concurrent refresh.
pub struct CatalogueCache {
    ttl: Duration,
    slots: StdMutex<HashMap<String, Arc<AsyncMutex<Slot>>>>,
}

impl Default for CatalogueCache {
    fn default() -> Self {
        Self::new()
    }
}

impl CatalogueCache {
    /// A cache using the default 10 minute TTL.
    pub fn new() -> Self {
        Self::with_ttl(DEFAULT_TTL)
    }

    /// A cache with an explicit TTL.
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            ttl,
            slots: StdMutex::new(HashMap::new()),
        }
    }

    fn slot_for(&self, key: &str) -> Arc<AsyncMutex<Slot>> {
        self.slots
            .lock()
            .expect("catalogue cache mutex poisoned")
            .entry(key.to_owned())
            .or_insert_with(|| Arc::new(AsyncMutex::new(Slot::default())))
            .clone()
    }

    /// The cached catalogue for `key`, refreshing through `client` when stale; concurrent callers share one fetch.
    pub async fn get_or_refresh(
        &self,
        key: &str,
        client: &Client,
    ) -> Result<Arc<CatalogueSnapshot>, EngineError> {
        self.get_or_refresh_with(key, || discovery::fetch(client))
            .await
    }

    /// `get_or_refresh` over an arbitrary fetch, so the cache is testable without a cluster.
    async fn get_or_refresh_with<F, Fut>(
        &self,
        key: &str,
        fetch: F,
    ) -> Result<Arc<CatalogueSnapshot>, EngineError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<Catalogue, EngineError>>,
    {
        let slot = self.slot_for(key);
        let mut guard = slot.lock().await;
        if let Some(snapshot) = &guard.snapshot {
            if snapshot.age() < self.ttl {
                return Ok(snapshot.clone());
            }
        }
        let catalogue = fetch().await?;
        guard.generation += 1;
        let snapshot = Arc::new(CatalogueSnapshot {
            catalogue: Arc::new(catalogue),
            generation: guard.generation,
            fetched_at: Instant::now(),
        });
        guard.snapshot = Some(snapshot.clone());
        Ok(snapshot)
    }

    /// Drop the cached entry for `key` so the next `get_or_refresh` fetches again.
    pub async fn invalidate(&self, key: &str) {
        let slot = self.slot_for(key);
        let mut guard = slot.lock().await;
        guard.snapshot = None;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn entry(plural: &str, kind: &str, namespaced: bool) -> CatalogueEntry {
        CatalogueEntry {
            group: String::new(),
            version: "v1".to_owned(),
            kind: kind.to_owned(),
            plural: plural.to_owned(),
            singular: "pod".to_owned(),
            namespaced,
            verbs: vec!["get".to_owned(), "list".to_owned()],
            short_names: vec!["po".to_owned()],
            categories: Vec::new(),
            additional_printer_columns: Vec::new(),
        }
    }

    fn catalogue(plural: &str) -> Catalogue {
        Catalogue {
            entries: vec![entry(plural, "Pod", true)],
        }
    }

    fn gvk(kind: &str) -> GroupVersionKind {
        GroupVersionKind {
            version: Some("v1".to_owned()),
            kind: Some(kind.to_owned()),
            ..Default::default()
        }
    }

    #[test]
    fn columns_fall_back_to_name_namespace_age() {
        let columns = columns_for(&catalogue("pods"), &gvk("Pod"));
        let names: Vec<_> = columns.iter().map(|column| column.name.as_str()).collect();
        assert_eq!(names, ["Name", "Namespace", "Age"]);
    }

    #[test]
    fn columns_omit_namespace_for_cluster_scope() {
        let scoped = Catalogue {
            entries: vec![entry("nodes", "Node", false)],
        };
        let columns = columns_for(&scoped, &gvk("Node"));
        let names: Vec<_> = columns.iter().map(|column| column.name.as_str()).collect();
        assert_eq!(names, ["Name", "Age"]);
    }

    #[test]
    fn columns_prefer_the_served_set() {
        let mut served = catalogue("pods");
        served.entries[0].additional_printer_columns = vec![PrinterColumn {
            name: "Ready".to_owned(),
            column_type: "string".to_owned(),
            json_path: ".status.phase".to_owned(),
            priority: 0,
        }];
        let columns = columns_for(&served, &gvk("Pod"));
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].name, "Ready");
    }

    #[tokio::test]
    async fn a_fresh_snapshot_is_served_without_refetching() {
        let cache = CatalogueCache::new();
        let fetches = AtomicUsize::new(0);
        let first = cache
            .get_or_refresh_with("ctx", || async {
                fetches.fetch_add(1, Ordering::SeqCst);
                Ok(catalogue("pods"))
            })
            .await
            .expect("first fetch");
        let second = cache
            .get_or_refresh_with("ctx", || async {
                fetches.fetch_add(1, Ordering::SeqCst);
                Ok(catalogue("pods"))
            })
            .await
            .expect("second fetch");

        assert_eq!(fetches.load(Ordering::SeqCst), 1);
        assert_eq!(first.generation, 1);
        assert!(Arc::ptr_eq(&first.catalogue, &second.catalogue));
    }

    #[tokio::test]
    async fn an_expired_snapshot_is_refetched() {
        let cache = CatalogueCache::with_ttl(Duration::ZERO);
        let fetches = AtomicUsize::new(0);
        for _ in 0..2 {
            cache
                .get_or_refresh_with("ctx", || async {
                    fetches.fetch_add(1, Ordering::SeqCst);
                    Ok(catalogue("pods"))
                })
                .await
                .expect("fetch");
        }
        assert_eq!(fetches.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn invalidate_forces_the_next_fetch() {
        let cache = CatalogueCache::new();
        let fetches = AtomicUsize::new(0);
        let fetch = || async {
            fetches.fetch_add(1, Ordering::SeqCst);
            Ok(catalogue("pods"))
        };
        cache
            .get_or_refresh_with("ctx", fetch)
            .await
            .expect("first");
        cache.invalidate("ctx").await;
        cache
            .get_or_refresh_with("ctx", fetch)
            .await
            .expect("second");
        assert_eq!(fetches.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn concurrent_callers_share_one_fetch() {
        let cache = Arc::new(CatalogueCache::new());
        let fetches = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let cache = cache.clone();
            let fetches = fetches.clone();
            handles.push(tokio::spawn(async move {
                cache
                    .get_or_refresh_with("ctx", || async {
                        fetches.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(25)).await;
                        Ok(catalogue("pods"))
                    })
                    .await
                    .expect("fetch")
            }));
        }
        let mut generations = Vec::new();
        for handle in handles {
            generations.push(handle.await.expect("join").generation);
        }
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
        assert!(generations.iter().all(|generation| *generation == 1));
    }

    #[test]
    fn find_matches_group_version_kind() {
        let served = catalogue("pods");
        assert!(served.find("", "v1", "Pod").is_some());
        assert!(served.find("apps", "v1", "Pod").is_none());
    }
}
