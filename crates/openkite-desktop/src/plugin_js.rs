//! JS plugin host — manifest, discovery, registry, and hot-reload watcher.
//!
//! Decision (see `docs/plugin-architecture.md`): external plugins are
//! JS bundles dropped into `~/.openkite/plugins/<name>/` as
//! `manifest.json` + an entry `.js`. The webview evaluates the bundle and the
//! plugin registers UI through the eval bridge. This module owns the
//! on-disk + registry half: manifest parsing/validation, directory discovery,
//! tracking loaded plugins, and the **hot-reload diff** — watching the plugins
//! dir and reconciling the registry so added/removed/updated plugins take
//! effect without a restart. The static Rust SDK registry (`plugin_host`) is
//! unchanged.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    /// Optional stylesheet, relative to the plugin dir (must end in
    /// `.css`). Scoped to the plugin's own subtree at load time — see
    /// `scope_css` and the isolation contract in `docs/plugin-api.md`.
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    /// Statically-declared sidebar entries. Plugins may also register UI at
    /// runtime through the eval bridge; these are the load-time set.
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
        if let Some(style) = &self.style {
            if !style.ends_with(".css") {
                return Err(format!("style '{style}' must be a .css file"));
            }
            if Path::new(style).is_absolute() || style.split(['/', '\\']).any(|c| c == "..") {
                return Err(format!(
                    "style '{style}' must be relative and inside the plugin dir"
                ));
            }
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

/// An eval-ready JS plugin bundle: plugin name, entry-file path, and its
/// already-scoped stylesheet (if the manifest declares one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsBundle {
    pub name: String,
    pub entry: PathBuf,
    /// CSS rewritten by [`scope_css`] so every selector is scoped under
    /// `[data-plugin="<name>"]`. `None` when the manifest has no `style`.
    pub style: Option<String>,
}

/// Discover plugins under `root` and keep the enabled ones' bundles.
///
/// Broken siblings never block discovery: manifests that fail validation
/// surface in `errors`; the enabled predicate runs on the parsed name. A
/// plugin whose `style` fails to load or to scope (see [`scope_css`]) is
/// dropped from the bundle list with its reason appended to `errors` too —
/// the same disabled-with-reason treatment as a broken manifest, never a
/// panic, and it never blocks its siblings.
pub fn collect_bundles(
    root: &Path,
    enabled: impl Fn(&str) -> bool,
) -> (Vec<JsBundle>, Vec<String>) {
    let (discovered, mut errors) = discover_plugins(root);
    let mut bundles = Vec::new();
    for (name, manifest) in discovered {
        if !enabled(&name) {
            continue;
        }
        let plugin_dir = root.join(&name);
        match load_scoped_style(&plugin_dir, &manifest) {
            Ok(style) => bundles.push(JsBundle {
                entry: plugin_dir.join(&manifest.entry),
                name,
                style,
            }),
            Err(reason) => errors.push(format!("plugin '{name}' disabled: {reason}")),
        }
    }
    (bundles, errors)
}

/// Load and scope a plugin's declared `style` file, if any.
fn load_scoped_style(
    plugin_dir: &Path,
    manifest: &PluginManifest,
) -> Result<Option<String>, String> {
    let Some(style) = &manifest.style else {
        return Ok(None);
    };
    let path = plugin_dir.join(style);
    let text = fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    scope_css(&text, &manifest.name).map(Some)
}

/// Read a bundle's source text for eval.
pub fn load_source(bundle: &JsBundle) -> Result<String, String> {
    fs::read_to_string(&bundle.entry)
        .map_err(|err| format!("read {}: {err}", bundle.entry.display()))
}

/// Rewrite a plugin's CSS so every selector is scoped under its
/// `[data-plugin="<name>"]` root — the isolation contract documented in
/// `docs/plugin-api.md`. Dependency-free: no CSS parser crate, just enough
/// hand-rolled splitting to find selectors and at-rule bodies.
///
/// Handled: plain selectors and comma-separated selector lists (prefixed
/// with the plugin root as a descendant), `:root` (mapped onto the plugin
/// root itself, so a plugin's custom properties apply only to its own
/// subtree), and `@media`/`@supports`/`@container` (the prelude is kept
/// verbatim, the rules nested inside are scoped recursively).
///
/// Rejected — returned as `Err(reason)`, never a panic, so the caller can
/// disable the plugin's CSS with that reason instead of pretending it is
/// isolated: `@font-face` and `@keyframes` (their names are global by spec;
/// nesting the rule under an attribute selector does not scope them, so two
/// plugins could still collide on a font or animation name — this rewriter
/// refuses rather than claim an isolation it cannot deliver), `@import`
/// (reaches outside the plugin's own files), and any selector whose first
/// simple selector is `html`/`body` (no attribute scope can make
/// `[data-plugin="x"] html` match anything).
pub fn scope_css(css: &str, plugin: &str) -> Result<String, String> {
    let root = format!("[data-plugin=\"{plugin}\"]");
    scope_rules(&strip_css_comments(css), &root, plugin)
}

/// Strip `/* ... */` comments (string-unaware: plugin CSS is not expected to
/// carry `*/` inside a string literal, and stripping comments first keeps
/// the brace scanner below simple).
fn strip_css_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut chars = css.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(c) = chars.next() {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Split `css` into top-level `(prelude, body)` rules, where `body` is the
/// text between a rule's `{` and matching `}`. Depth-aware over `()`/`[]`
/// and string quotes, so a prelude like `:not(.a, .b)` or a `url("{")` never
/// confuses the brace match. A top-level `;` before any `{` (e.g.
/// `@import "x";`) is rejected rather than silently dropped.
fn split_rules(css: &str) -> Result<Vec<(String, String)>, String> {
    let chars: Vec<char> = css.chars().collect();
    let mut rules = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let start = i;
        let mut depth = 0i32;
        let mut quote: Option<char> = None;
        while i < chars.len() {
            let c = chars[i];
            if let Some(q) = quote {
                if c == q && chars[i - 1] != '\\' {
                    quote = None;
                }
            } else {
                match c {
                    '"' | '\'' => quote = Some(c),
                    '(' | '[' => depth += 1,
                    ')' | ']' => depth -= 1,
                    '{' if depth == 0 => break,
                    ';' if depth == 0 => {
                        let stmt: String = chars[start..i].iter().collect();
                        return Err(format!(
                            "unsupported statement in plugin stylesheet: '{}'",
                            stmt.trim()
                        ));
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        if i >= chars.len() {
            let rest: String = chars[start..].iter().collect();
            if rest.trim().is_empty() {
                break;
            }
            return Err(format!("unterminated rule: '{}'", rest.trim()));
        }
        let prelude: String = chars[start..i].iter().collect();
        let body_start = i + 1;
        let mut body_depth = 1i32;
        let mut j = body_start;
        let mut quote: Option<char> = None;
        while j < chars.len() && body_depth > 0 {
            let c = chars[j];
            if let Some(q) = quote {
                if c == q && chars[j - 1] != '\\' {
                    quote = None;
                }
            } else {
                match c {
                    '"' | '\'' => quote = Some(c),
                    '{' => body_depth += 1,
                    '}' => body_depth -= 1,
                    _ => {}
                }
            }
            j += 1;
        }
        if body_depth != 0 {
            return Err(format!("unterminated block for '{}'", prelude.trim()));
        }
        let body: String = chars[body_start..j - 1].iter().collect();
        rules.push((prelude.trim().to_string(), body));
        i = j;
    }
    Ok(rules)
}

/// Scope every rule in `css`, recursing into the rule-container at-rules.
fn scope_rules(css: &str, root: &str, plugin: &str) -> Result<String, String> {
    let mut out = String::new();
    for (prelude, body) in split_rules(css)? {
        if let Some(rest) = prelude.strip_prefix('@') {
            let (keyword, params) = split_at_rule(rest);
            match keyword.to_ascii_lowercase().as_str() {
                "media" | "supports" | "container" => {
                    let inner = scope_rules(&body, root, plugin)?;
                    out.push_str(&format!("@{keyword} {params} {{\n{inner}}}\n"));
                }
                "font-face" => {
                    return Err(format!(
                        "@font-face is global and cannot be scoped to plugin '{plugin}'s subtree"
                    ));
                }
                "keyframes" | "-webkit-keyframes" | "-moz-keyframes" => {
                    return Err(format!(
                        "@{keyword} names are global and cannot be scoped to plugin '{plugin}'s subtree"
                    ));
                }
                "import" => {
                    return Err(format!(
                        "@import reaches outside plugin '{plugin}'s own files and is not supported"
                    ));
                }
                other => {
                    return Err(format!(
                        "unsupported at-rule '@{other}' in plugin '{plugin}'s stylesheet"
                    ));
                }
            }
        } else {
            let selector = scope_selector_list(&prelude, root, plugin)?;
            out.push_str(&format!("{selector} {{{body}}}\n"));
        }
    }
    Ok(out)
}

/// Split an at-rule prelude (after the `@`) into its keyword and the rest
/// (the part between the keyword and the `{`), e.g. `media (min-width: 1px)`
/// → `("media", "(min-width: 1px)")`.
fn split_at_rule(s: &str) -> (&str, &str) {
    match s.find(|c: char| c.is_whitespace() || c == '(') {
        Some(idx) => (&s[..idx], s[idx..].trim()),
        None => (s, ""),
    }
}

/// Scope a comma-separated selector list under `root`.
fn scope_selector_list(selector_list: &str, root: &str, plugin: &str) -> Result<String, String> {
    let mut scoped = Vec::new();
    for part in split_top_level_commas(selector_list) {
        let sel = part.trim();
        if sel.is_empty() {
            continue;
        }
        if sel == ":root" {
            scoped.push(root.to_string());
            continue;
        }
        let first = sel
            .split(|c: char| c.is_whitespace() || c == '>' || c == '+' || c == '~')
            .next()
            .unwrap_or("")
            .trim_start_matches('*');
        let targets_host = |tag: &str| {
            let lower = first.to_ascii_lowercase();
            let tag_lower = tag.to_ascii_lowercase();
            lower == tag_lower
                || lower.starts_with(&format!("{tag_lower}."))
                || lower.starts_with(&format!("{tag_lower}#"))
                || lower.starts_with(&format!("{tag_lower}:"))
        };
        if targets_host("html") || targets_host("body") {
            return Err(format!(
                "selector '{sel}' in plugin '{plugin}'s stylesheet targets the host document; no plugin scope can reach it"
            ));
        }
        scoped.push(format!("{root} {sel}"));
    }
    Ok(scoped.join(", "))
}

/// Split a selector list on top-level commas (depth-aware over `()`/`[]`, so
/// `:not(.a, .b)` is not mistaken for two selectors).
fn split_top_level_commas(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for c in s.chars() {
        match c {
            '(' | '[' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    parts.push(current);
    parts
}
