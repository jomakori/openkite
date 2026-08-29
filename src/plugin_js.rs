//! JS plugin host — manifest, discovery, registry, and hot-reload watcher.
//!
//! Decision (OKT-28, see `docs/plugin-architecture.md`): external plugins are
//! JS bundles dropped into `~/.openkite/plugins/<name>/` as
//! `manifest.json` + an entry `.js`. The webview evaluates the bundle and the
//! plugin registers UI through the eval bridge (OKT-46). This module owns the
//! on-disk + registry half: manifest parsing/validation, directory discovery,
//! tracking loaded plugins, and the **hot-reload diff** — watching the plugins
//! dir and reconciling the registry so added/removed/updated plugins take
//! effect without a restart. The static Rust SDK registry (`plugin_host`) is
//! unchanged.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

// `watch()`/`unwatch()` are inherent to the `Watcher` trait — must be in scope.
use notify::Watcher;

/// A plugin's `manifest.json` — the contract a JS plugin must declare.
///
/// Minimal example:
/// ```json
/// { "name": "argocd", "version": "0.1.0", "entry": "main.js" }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique, filesystem-safe id (`[a-zA-Z0-9-_]`). Also the plugin dir name.
    pub name: String,
    /// Semver string, e.g. `"0.1.0"`.
    pub version: String,
    /// JS entry point, relative to the plugin dir (must end in `.js`).
    pub entry: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    /// Statically-declared sidebar entries. Plugins may also register UI at
    /// runtime through the eval bridge (OKT-46); these are the load-time set.
    #[serde(default)]
    pub sidebar: Vec<SidebarEntry>,
}

/// A sidebar item declared in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarEntry {
    pub label: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub route: String,
}

impl PluginManifest {
    /// Validate the fields the host depends on before loading.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("name must not be empty".into());
        }
        if !self
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!(
                "name '{}' may only contain [a-zA-Z0-9-_]",
                self.name
            ));
        }
        if self.version.trim().is_empty() {
            return Err("version must not be empty".into());
        }
        if self.entry.trim().is_empty() {
            return Err("entry must not be empty".into());
        }
        if !self.entry.ends_with(".js") {
            return Err(format!("entry '{}' must be a .js file", self.entry));
        }
        if Path::new(&self.entry).is_absolute() || self.entry.split(['/', '\\']).any(|c| c == "..")
        {
            return Err(format!(
                "entry '{}' must be relative and inside the plugin dir",
                self.entry
            ));
        }
        Ok(())
    }
}

/// The plugins root: `~/.openkite/plugins`.
pub fn plugins_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openkite")
        .join("plugins")
}

/// Load + validate the manifest of one plugin directory.
pub fn load_manifest(plugin_dir: &Path) -> Result<PluginManifest, String> {
    let path = plugin_dir.join("manifest.json");
    let text = fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let manifest: PluginManifest =
        serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    manifest.validate()?;
    Ok(manifest)
}

/// Scan a plugins root and load every plugin directory with a valid manifest.
///
/// Returns `(loaded, errors)`; a broken plugin never blocks its siblings.
/// Plugins are sorted by name for deterministic load order.
pub fn discover_plugins(root: &Path) -> (Vec<(String, PluginManifest)>, Vec<String>) {
    let mut loaded = Vec::new();
    let mut errors = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) => {
            errors.push(format!("scan {}: {e}", root.display()));
            return (loaded, errors);
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        match load_manifest(&path) {
            Ok(manifest) => loaded.push((manifest.name.clone(), manifest)),
            Err(e) => errors.push(e),
        }
    }
    loaded.sort_by(|a, b| a.0.cmp(&b.0));
    (loaded, errors)
}

/// What happened to a plugin — the unit of the hot-reload diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginAction {
    Added,
    Removed,
    Changed,
}

/// A single plugin lifecycle change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginChange {
    pub name: String,
    pub action: PluginAction,
}

/// Map a `notify` event kind to the coarse action it implies.
///
/// Renames surface as `Modify(Name(..))` → `Changed`; the caller's rescan +
/// reconcile determines the authoritative Added/Removed.
pub fn action_from_kind(kind: &notify::EventKind) -> Option<PluginAction> {
    match kind {
        notify::EventKind::Create(_) => Some(PluginAction::Added),
        notify::EventKind::Remove(_) => Some(PluginAction::Removed),
        notify::EventKind::Modify(_) => Some(PluginAction::Changed),
        // Access and Any carry no plugin-meaningful signal.
        _ => None,
    }
}

/// Collapse a burst of watcher events into one change per plugin (last wins).
///
/// A plugin added and then removed within one burst nets out to Removed.
pub fn coalesce(changes: Vec<PluginChange>) -> Vec<PluginChange> {
    let mut last = BTreeMap::new();
    for change in changes {
        last.insert(change.name, change.action);
    }
    let mut out: Vec<PluginChange> = last
        .into_iter()
        .map(|(name, action)| PluginChange { name, action })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Runtime registry of discovered JS plugins: `name → (manifest, enabled)`.
#[derive(Debug, Default)]
pub struct JsPluginRegistry {
    plugins: BTreeMap<String, (PluginManifest, bool)>,
}

impl JsPluginRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a plugin, keeping its enabled state on replace.
    pub fn upsert(&mut self, manifest: PluginManifest, enabled: bool) {
        self.plugins
            .insert(manifest.name.clone(), (manifest, enabled));
    }

    /// Remove a plugin; returns whether it was present.
    pub fn remove(&mut self, name: &str) -> bool {
        self.plugins.remove(name).is_some()
    }

    /// Set enable/disable; returns whether the plugin is known.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> bool {
        match self.plugins.get_mut(name) {
            Some((_, state)) => {
                *state = enabled;
                true
            }
            None => false,
        }
    }

    /// Whether a known plugin is enabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.plugins.get(name).map(|(_, e)| *e).unwrap_or(false)
    }

    /// Names of all discovered plugins, sorted.
    pub fn names(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// Number of discovered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Whether no plugins are discovered.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Diff a fresh discovery scan against the registry and apply it.
    ///
    /// Returns the changes (Added/Removed/Changed) so the host can re-evaluate
    /// affected bundles — this is the hot-reload reconciliation step. Enabled
    /// state survives a Changed (reload) and is set to `default_enabled` for
    /// newly Added plugins.
    pub fn reconcile(
        &mut self,
        discovered: Vec<(String, PluginManifest)>,
        default_enabled: bool,
    ) -> Vec<PluginChange> {
        let mut changes = Vec::new();

        for name in self.names() {
            if !discovered.iter().any(|(n, _)| n == &name) {
                self.remove(&name);
                changes.push(PluginChange {
                    name,
                    action: PluginAction::Removed,
                });
            }
        }

        for (name, manifest) in discovered {
            match self.plugins.get(&name) {
                Some((existing, enabled)) => {
                    if existing != &manifest {
                        self.upsert(manifest, *enabled);
                        changes.push(PluginChange {
                            name,
                            action: PluginAction::Changed,
                        });
                    }
                }
                None => {
                    self.upsert(manifest, default_enabled);
                    changes.push(PluginChange {
                        name,
                        action: PluginAction::Added,
                    });
                }
            }
        }

        changes.sort_by(|a, b| a.name.cmp(&b.name));
        changes
    }
}

/// Scan the plugins root and reconcile the registry in one call.
///
/// This is the entry point the host uses at startup and on every watcher tick.
/// Returns the changes plus any discovery errors (broken manifests etc.).
pub fn scan_and_reconcile(
    root: &Path,
    registry: &mut JsPluginRegistry,
    default_enabled: bool,
) -> (Vec<PluginChange>, Vec<String>) {
    let (discovered, errors) = discover_plugins(root);
    let changes = registry.reconcile(discovered, default_enabled);
    (changes, errors)
}

/// Watch a plugins root recursively, forwarding one `PluginChange` per
/// affected plugin (coarse; callers rescan + reconcile to get the truth).
pub fn watch_plugins(
    root: &Path,
    tx: std::sync::mpsc::Sender<PluginChange>,
) -> Result<notify::RecommendedWatcher, notify::Error> {
    // The notify callback is `'static` — capture an owned copy of the root.
    let root_owned = root.to_path_buf();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else {
            return;
        };
        for path in event.paths {
            let Ok(rel) = path.strip_prefix(&root_owned) else {
                continue;
            };
            let Some(name) = rel.components().next().and_then(|c| c.as_os_str().to_str()) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            if let Some(action) = action_from_kind(&event.kind) {
                let _ = tx.send(PluginChange {
                    name: name.to_string(),
                    action,
                });
            }
        }
    })?;
    watcher.watch(root, notify::RecursiveMode::Recursive)?;
    Ok(watcher)
}

/// An eval-ready JS plugin bundle (OKT-31): plugin name + entry-file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsBundle {
    pub name: String,
    pub entry: PathBuf,
}

/// Discover plugins under `root` and keep the enabled ones' bundles.
///
/// Broken siblings never block discovery: manifests that fail validation
/// surface in `errors`; the enabled predicate runs on the parsed name.
pub fn collect_bundles(
    root: &Path,
    enabled: impl Fn(&str) -> bool,
) -> (Vec<JsBundle>, Vec<String>) {
    let (discovered, errors) = discover_plugins(root);
    let bundles = discovered
        .into_iter()
        .filter(|(name, _)| enabled(name))
        .map(|(name, manifest)| JsBundle {
            entry: root.join(&name).join(&manifest.entry),
            name,
        })
        .collect();
    (bundles, errors)
}

/// Read a bundle's source text for eval.
pub fn load_source(bundle: &JsBundle) -> Result<String, String> {
    fs::read_to_string(&bundle.entry)
        .map_err(|err| format!("read {}: {err}", bundle.entry.display()))
}
