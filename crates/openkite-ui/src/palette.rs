//! Search and command palette.
//!
//! The index is built from projections only — the same kind/namespace/name/label set the list
//! views already hold — and never touches a cluster or a store. Searchable text lives in one
//! growing arena with spans, postings are item indices, and the token map is ordered so a prefix
//! query is one range scan. That keeps 40k objects inside the ticket's memory and latency budget
//! (see `tests/palette_budget.rs`, which measures real allocations).

use std::collections::BTreeMap;

/// Bytes per item above the arena: kind, namespace, two spans.
const ITEM_BYTES: usize = 16;

/// Results the palette shows for one query.
pub const RESULT_LIMIT: usize = 50;

/// One indexed object, as a projection hands it over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub kind: String,
    pub namespace: String,
    pub name: String,
    pub labels: Vec<String>,
}

impl Entry {
    /// An entry with no labels.
    pub fn new(kind: &str, namespace: &str, name: &str) -> Self {
        Self {
            kind: kind.to_string(),
            namespace: namespace.to_string(),
            name: name.to_string(),
            labels: Vec::new(),
        }
    }

    /// Adds `key=value` labels.
    pub fn with_labels(mut self, labels: &[&str]) -> Self {
        self.labels = labels.iter().map(|l| l.to_string()).collect();
        self
    }
}

/// A span into the arena, stored as `(offset, len)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Span {
    at: u32,
    len: u16,
}

impl Span {
    fn write(arena: &mut String, text: &str) -> Span {
        let at = arena.len() as u32;
        arena.push_str(text);
        Span {
            at,
            len: text.len().min(u16::MAX as usize) as u16,
        }
    }

    fn read<'a>(&self, arena: &'a str) -> &'a str {
        let start = self.at as usize;
        let end = (start + self.len as usize).min(arena.len());
        arena.get(start..end).unwrap_or("")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Item {
    kind: u8,
    namespace: u16,
    name: Span,
    /// Labels joined with `\u{1f}`, so a label prefix scan cannot run across two of them.
    labels: Span,
}

/// What kind of thing a hit is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hit {
    Object {
        kind: String,
        namespace: String,
        name: String,
        /// The label that matched the query, when the match came from a label.
        label: Option<String>,
    },
    Command {
        id: &'static str,
        title: String,
        hint: &'static str,
        keys: &'static str,
    },
}

impl Hit {
    /// Stable key for the view: identity for objects, id for commands.
    pub fn key(&self) -> String {
        match self {
            Hit::Object {
                kind,
                namespace,
                name,
                ..
            } => format!("{kind}/{namespace}/{name}"),
            Hit::Command { id, .. } => format!("cmd/{id}"),
        }
    }

    /// The line the palette shows in bold.
    pub fn title(&self) -> &str {
        match self {
            Hit::Object { name, .. } => name,
            Hit::Command { title, .. } => title,
        }
    }

    /// The dimmed line above or below the title.
    pub fn subtitle(&self) -> String {
        match self {
            Hit::Object {
                kind,
                namespace,
                label,
                ..
            } => match label {
                Some(label) => format!("{kind} · {namespace} · {label}"),
                None => format!("{kind} · {namespace}"),
            },
            Hit::Command { hint, .. } => (*hint).to_string(),
        }
    }
}

/// A palette command: navigation and the two things the keyboard can do from here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub id: &'static str,
    pub title: &'static str,
    pub hint: &'static str,
    pub keys: &'static str,
}

/// Every command the palette offers, in the order it shows them for an empty query.
pub const COMMANDS: &[Command] = &[
    Command {
        id: "overview",
        title: "Go to overview",
        hint: "Board, usage and attention",
        keys: "g o",
    },
    Command {
        id: "pods",
        title: "Go to pods",
        hint: "Every pod in the cluster",
        keys: "g p",
    },
    Command {
        id: "deployments",
        title: "Go to deployments",
        hint: "Rollouts and replicas",
        keys: "g d",
    },
    Command {
        id: "nodes",
        title: "Go to nodes",
        hint: "Capacity and pressure",
        keys: "g n",
    },
    Command {
        id: "events",
        title: "Go to events",
        hint: "What just happened, newest first",
        keys: "g e",
    },
    Command {
        id: "argo",
        title: "Go to Argo CD",
        hint: "Applications and sync state",
        keys: "g a",
    },
    Command {
        id: "namespaces",
        title: "Go to namespaces",
        hint: "Pick a namespace to filter by",
        keys: "g s",
    },
    Command {
        id: "logs",
        title: "Toggle the log dock",
        hint: "Follow logs without leaving the list",
        keys: "ctrl `",
    },
    Command {
        id: "customize",
        title: "Customize the overview",
        hint: "Move, resize and hide widgets",
        keys: "g c",
    },
    Command {
        id: "settings",
        title: "Open settings",
        hint: "Theme, density and preferences",
        keys: "g ,",
    },
];

/// The index plus the queries over it.
#[derive(Debug, Default, Clone)]
pub struct Palette {
    arena: String,
    items: Vec<Item>,
    kinds: Vec<String>,
    namespaces: Vec<String>,
    tokens: BTreeMap<Box<str>, Vec<u32>>,
}

impl Palette {
    /// Builds the index. One pass over the entries; no cluster access.
    pub fn build(entries: impl IntoIterator<Item = Entry>) -> Self {
        let mut palette = Self::default();
        for entry in entries {
            palette.push(entry);
        }
        palette
    }

    /// Adds one entry, returning its index.
    pub fn push(&mut self, entry: Entry) -> usize {
        let kind = intern(&mut self.kinds, &entry.kind) as u8;
        let namespace = intern(&mut self.namespaces, &entry.namespace) as u16;
        let name = Span::write(&mut self.arena, &entry.name);
        let labels = Span::write(&mut self.arena, &entry.labels.join("\u{1f}"));
        let at = self.items.len() as u32;
        self.items.push(Item {
            kind,
            namespace,
            name,
            labels,
        });
        for token in tokens(&entry.name) {
            self.post(&token, at);
        }
        for label in &entry.labels {
            // The pair itself (`app=api`) and its halves (`app`, `api`) are both searchable.
            self.post(&label.to_lowercase(), at);
            for token in tokens(label) {
                self.post(&token, at);
            }
        }
        self.items.len() - 1
    }

    fn post(&mut self, token: &str, at: u32) {
        match self.tokens.get_mut(token) {
            Some(list) => {
                if list.last() != Some(&at) {
                    list.push(at);
                }
            }
            None => {
                self.tokens.insert(token.into(), vec![at]);
            }
        }
    }

    /// Objects in the index.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the index holds nothing.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Bytes the index occupies, arena and postings included.
    ///
    /// The budget test measures real allocations as well; this is the cheap estimate the
    /// palette can report without walking its own tables.
    pub fn heap_bytes(&self) -> usize {
        let postings: usize = self
            .tokens
            .values()
            .map(|list| {
                list.capacity() * std::mem::size_of::<u32>() + std::mem::size_of::<Vec<u32>>()
            })
            .sum();
        let keys: usize = self.tokens.keys().map(|k| k.len() + 24).sum();
        self.arena.len()
            + keys
            + postings
            + self.items.capacity() * ITEM_BYTES
            + self.kinds.iter().map(|k| k.len() + 24).sum::<usize>()
            + self.namespaces.iter().map(|n| n.len() + 24).sum::<usize>()
    }

    /// Runs a query: `>` alone lists commands, otherwise objects first then matching commands.
    pub fn query(&self, query: &str) -> Vec<Hit> {
        let trimmed = query.trim();
        if let Some(rest) = trimmed.strip_prefix('>') {
            return self.commands(rest);
        }
        let terms = self.parse_query(trimmed);
        if terms.tokens.is_empty() && terms.labels.is_empty() && trimmed.is_empty() {
            return self.commands("");
        }

        let mut hits = self.objects(&terms);
        hits.extend(self.commands(trimmed));
        hits.truncate(RESULT_LIMIT);
        hits
    }

    /// Splits the query into tokens, labels and filters, recognising a bare namespace or kind
    /// name as the filter it is.
    fn parse_query(&self, query: &str) -> Terms {
        let mut terms = Terms {
            joined: query.trim().to_lowercase(),
            ..Terms::default()
        };
        for word in query.split_whitespace() {
            let lower = word.to_lowercase();
            if let Some(value) = lower.strip_prefix("ns=") {
                terms.namespace = Some(value.to_string());
            } else if let Some(value) = lower.strip_prefix("kind=") {
                terms.kind = Some(singular(value));
            } else if lower.contains('=') {
                terms.labels.push(lower);
            } else if self.namespaces.iter().any(|n| n == &lower) {
                terms.namespace = Some(lower);
            } else if self.kinds.iter().any(|k| k == &singular(&lower)) {
                terms.kind = Some(singular(&lower));
            } else {
                // A pasted name arrives hyphenated; its pieces are the index tokens.
                terms.tokens.extend(tokens(&lower));
            }
        }
        terms
    }

    fn commands(&self, query: &str) -> Vec<Hit> {
        let needle = query.trim().to_lowercase();
        COMMANDS
            .iter()
            .filter(|cmd| {
                needle.is_empty()
                    || cmd.title.to_lowercase().contains(&needle)
                    || cmd.hint.to_lowercase().contains(&needle)
            })
            .map(|cmd| Hit::Command {
                id: cmd.id,
                title: cmd.title.to_string(),
                hint: cmd.hint,
                keys: cmd.keys,
            })
            .collect()
    }

    fn objects(&self, terms: &Terms) -> Vec<Hit> {
        let mut candidates: Option<Vec<u32>> = None;
        for term in terms.tokens.iter().chain(&terms.labels) {
            let set = self.match_token(term);
            candidates = Some(match candidates {
                Some(prev) => intersect(&prev, &set),
                None => set,
            });
            if candidates.as_ref().is_some_and(Vec::is_empty) {
                return Vec::new();
            }
        }

        let ats = match candidates {
            Some(ats) => ats
                .into_iter()
                .filter(|at| self.matches_filter(*at, terms))
                .collect(),
            None => self.filtered(terms),
        };

        let needle = terms.joined.clone();
        let mut ranked: Vec<(bool, usize, &str, u32)> = ats
            .into_iter()
            .map(|at| {
                let name = self.items[at as usize].name.read(&self.arena);
                (name.eq_ignore_ascii_case(&needle), name.len(), name, at)
            })
            .collect();
        ranked.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then(a.1.cmp(&b.1))
                .then(a.2.cmp(b.2))
                .then(a.3.cmp(&b.3))
        });

        ranked
            .into_iter()
            .take(RESULT_LIMIT)
            .map(|(_, _, _, at)| self.hit(at, terms))
            .collect()
    }

    /// Whether an item survives the `ns=` / `kind=` filters.
    fn matches_filter(&self, at: u32, terms: &Terms) -> bool {
        let item = &self.items[at as usize];
        terms
            .namespace
            .as_deref()
            .is_none_or(|want| self.namespaces[item.namespace as usize] == want)
            && terms
                .kind
                .as_deref()
                .is_none_or(|want| self.kinds[item.kind as usize] == want)
    }

    /// Items selected by namespace or kind alone (`ns=prod`, `kind=pod`, or a bare name that
    /// happens to be a namespace or kind).
    fn filtered(&self, terms: &Terms) -> Vec<u32> {
        self.items
            .iter()
            .enumerate()
            .filter(|(at, _)| self.matches_filter(*at as u32, terms))
            .map(|(at, _)| at as u32)
            .collect()
    }

    /// Items whose name, label, namespace or kind token matches `token` by prefix.
    fn match_token(&self, token: &str) -> Vec<u32> {
        let mut out: Vec<u32> = Vec::new();
        for (key, list) in self.tokens.range(Box::<str>::from(token)..) {
            if !key.starts_with(token) {
                break;
            }
            out.extend_from_slice(list);
        }
        if out.is_empty() {
            // A bare namespace or kind is a filter, not a name fragment.
            if let Some(ns) = self.namespaces.iter().position(|n| n == token) {
                out.extend(
                    self.items
                        .iter()
                        .enumerate()
                        .filter(|(_, item)| item.namespace as usize == ns)
                        .map(|(at, _)| at as u32),
                );
            }
            if let Some(kind) = self.kinds.iter().position(|k| k == token) {
                out.extend(
                    self.items
                        .iter()
                        .enumerate()
                        .filter(|(_, item)| item.kind as usize == kind)
                        .map(|(at, _)| at as u32),
                );
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    fn hit(&self, at: u32, terms: &Terms) -> Hit {
        let item = &self.items[at as usize];
        // Only a label that the query actually named is worth showing.
        let label = item
            .labels
            .read(&self.arena)
            .split('\u{1f}')
            .find(|label| {
                let lower = label.to_lowercase();
                terms
                    .labels
                    .iter()
                    .any(|want| lower.starts_with(want.as_str()))
            })
            .map(str::to_string);
        Hit::Object {
            kind: self.kinds[item.kind as usize].clone(),
            namespace: self.namespaces[item.namespace as usize].clone(),
            name: item.name.read(&self.arena).to_string(),
            label,
        }
    }
}

/// Intersects two sorted posting lists in one pass.
fn intersect(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len().min(b.len()));
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                out.push(a[i]);
                i += 1;
                j += 1;
            }
        }
    }
    out
}

fn intern(table: &mut Vec<String>, value: &str) -> usize {
    match table.iter().position(|v| v == value) {
        Some(at) => at,
        None => {
            table.push(value.to_string());
            table.len() - 1
        }
    }
}

/// Lowercased alphanumeric runs.
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            current.extend(ch.to_lowercase());
        } else if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// A parsed query: name tokens, label pairs, and the namespace/kind filters.
#[derive(Debug, Default)]
struct Terms {
    tokens: Vec<String>,
    labels: Vec<String>,
    namespace: Option<String>,
    kind: Option<String>,
    /// The whole query lowercased, for the exact-name check.
    joined: String,
}

/// `pods` filters like `pod`.
fn singular(word: &str) -> String {
    word.strip_suffix('s').unwrap_or(word).to_string()
}
