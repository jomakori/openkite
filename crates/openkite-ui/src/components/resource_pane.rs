//! The resource detail side pane (Lens parity): every kind opens at the same
//! stop, and a second selection replaces the content in place.
//!
//! The pane is one component in one slot. It renders from a single selection
//! ([`crate::runtime::RESOURCE_SELECTION`]) rather than from per-kind chrome,
//! so a pod, a deployment, a service, a node, a configmap and an Argo CD
//! application all paint the same `.inspector` markup at the same right edge:
//! the stylesheet owns the position, and nothing about a kind changes it. A
//! selection never pushes a second pane — it overwrites the slot — and the pane
//! does not unmount the route, so the list keeps its scroll and its selected
//! row.
//!
//! What the pane shows is [`ResourceDetail`]: identity, status, the metadata a
//! Lens user expects, the controller owner and the pane's own actions. It is
//! built either from a row the table already has (`from_row` — the same
//! `ResourceRow` the row-selection path hands over) or from a bare reference
//! (`from_ref`, what a deep link alone can restore).
//!
//! Three behaviours need a document, so they ride the crate's existing
//! `document::eval` seam (the same one the log viewer, the terminal and the
//! secret detail use) and are installed by one script:
//!
//! - **Escape** clicks the pane's own close button, so the close travels the
//!   normal Dioxus event path on both hosts.
//! - **Resize**: the left edge drags; the width is clamped to
//!   [`PANE_MIN_WIDTH`]..[`PANE_MAX_WIDTH`] and persisted under
//!   [`PANE_WIDTH_KEY`] in the host's `localStorage` — the same browser storage
//!   seam the console used for its settings — so it survives a reload.
//! - **Focus trap**: while focus is inside the pane, Tab cycles within it. The
//!   pane never focuses itself, so opening it cannot steal focus from the dock.
//!
//! At ≤767px the stylesheet turns the pane into the reference's full-width
//! bottom sheet and the `.inspector-scrim` becomes visible; above that width
//! the scrim stays inert so clicking another row still replaces the content.
//!
//! # What is wired, and what is still owed
//!
//! The row-selection path is [`toggle_resource_row`], which a host wires to
//! `ResourceTable::on_row_click`: a second click on the same row clears the
//! slot, a click on another row overwrites it, and the shell
//! ([`crate::components::shell::AppShell`]) mounts the pane once so both hosts
//! paint it at the same stop for every kind without a per-host copy.
//!
//! Three acceptance clauses depend on surfaces outside this ticket, and this
//! module does not pretend otherwise:
//!
//! - **A click in a served app** needs the inventory list that supplies the
//!   rows (ticket T5, pod inventory). Until that lands the row click is proven
//!   by the crate's mount tests, not by a click in a running app.
//! - **The interactive behaviours** (resize + persisted width, Escape,
//!   backdrop, Tab trap, URL sync) ride `document::eval`, so they run only on a
//!   host with a Dioxus client. The browser host still serves
//!   `RenderOptions::ssr_only()` and emits no client script; the desktop
//!   webview runs them.
//! - **The pane's own actions** render disabled until a host passes
//!   `on_action`. The reference mockup ships them inert as well — its
//!   `.inspector-actions` buttons are `data-toast` stubs — so an inert action
//!   is the reference's own state, not a regression introduced here.

#![allow(dead_code)]
#![allow(non_snake_case)]

use dioxus::prelude::*;

use crate::components::resource_table::{ColumnDef, ResourceRow};
use crate::components::status_badge::{StatusKind, StatusPill};
use crate::runtime::{
    clear_resource_selection, select_resource, selected_resource, RESOURCE_SELECTION,
};

/// Query parameter carrying the resource kind.
pub const KIND_PARAM: &str = "kind";
/// Query parameter carrying the namespace (absent for cluster-scoped kinds).
pub const NAMESPACE_PARAM: &str = "ns";
/// Query parameter carrying the resource name.
pub const NAME_PARAM: &str = "name";

/// The narrowest the pane can be dragged to, in pixels.
pub const PANE_MIN_WIDTH: u32 = 280;
/// The widest the pane can be dragged to, in pixels. The reference's own
/// default — 420px, the stylesheet's fallback — sits inside this range.
pub const PANE_MAX_WIDTH: u32 = 720;

/// Where the drag handle persists the pane's width. Browser storage is the
/// seam the console already persisted per-browser preferences in.
pub const PANE_WIDTH_KEY: &str = "openkite.pane.width";

/// Clamp a dragged width into the pane's allowed range.
pub fn clamp_pane_width(px: f64) -> u32 {
    if !px.is_finite() {
        return PANE_MAX_WIDTH;
    }
    (px.round() as i64).clamp(PANE_MIN_WIDTH as i64, PANE_MAX_WIDTH as i64) as u32
}

/// Parse a persisted width. Empty, non-numeric and out-of-range values are
/// ignored (`None`) rather than clamped: a corrupt entry falls back to the
/// stylesheet's default instead of silently rewriting the user's choice.
pub fn parse_pane_width(raw: &str) -> Option<u32> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let px: f64 = trimmed.parse().ok()?;
    if !px.is_finite() || px < PANE_MIN_WIDTH as f64 || px > PANE_MAX_WIDTH as f64 {
        return None;
    }
    Some(clamp_pane_width(px))
}

/// The identity half of a selection: what the pane is showing, and what the
/// address bar carries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResourceRef {
    /// Resource kind, in the table's own vocabulary (`Pod`, `Deployment`, …).
    pub kind: String,
    /// Namespace, absent for cluster-scoped kinds (nodes, Argo CD applications).
    pub namespace: Option<String>,
    /// Resource name.
    pub name: String,
}

impl ResourceRef {
    /// Build a reference.
    pub fn new(
        kind: impl Into<String>,
        namespace: Option<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            namespace,
            name: name.into(),
        }
    }

    /// Stable identity of the selection: two references with the same key are
    /// the same row, which is what makes a re-click a close.
    pub fn key(&self) -> String {
        format!(
            "{}/{}/{}",
            self.kind.to_lowercase(),
            self.namespace.as_deref().unwrap_or(""),
            self.name
        )
    }

    /// The selection as a query string, e.g. `kind=pod&ns=default&name=web-1`.
    pub fn query(&self) -> String {
        let mut out = String::new();
        push_param(&mut out, KIND_PARAM, &self.kind);
        if let Some(namespace) = &self.namespace {
            push_param(&mut out, NAMESPACE_PARAM, namespace);
        }
        push_param(&mut out, NAME_PARAM, &self.name);
        out
    }

    /// Restore a selection from a query string. Unknown parameters are ignored
    /// — the console's routes carry other state (filters, namespaces) — and a
    /// query without both a kind and a name selects nothing.
    pub fn from_query(query: &str) -> Option<Self> {
        let mut kind = None;
        let mut namespace = None;
        let mut name = None;
        for pair in query.trim_start_matches('?').split('&') {
            if pair.is_empty() {
                continue;
            }
            let (raw_key, raw_value) = match pair.split_once('=') {
                Some((key, value)) => (key, value),
                None => (pair, ""),
            };
            let value = decode_component(raw_value);
            match decode_component(raw_key).as_str() {
                KIND_PARAM => kind = Some(value),
                NAMESPACE_PARAM => namespace = Some(value),
                NAME_PARAM => name = Some(value),
                _ => {}
            }
        }
        let kind = kind.filter(|value| !value.is_empty())?;
        let name = name.filter(|value| !value.is_empty())?;
        Some(Self {
            kind,
            namespace: namespace.filter(|value| !value.is_empty()),
            name,
        })
    }

    /// The selection as an address on `path`, e.g.
    /// `/workloads?kind=pod&ns=default&name=web-1`.
    pub fn address(&self, path: &str) -> String {
        format!("{}?{}", path.trim_end_matches('?'), self.query())
    }
}

/// Append one percent-encoded `key=value` pair.
fn push_param(out: &mut String, key: &str, value: &str) {
    if !out.is_empty() {
        out.push('&');
    }
    out.push_str(key);
    out.push('=');
    out.push_str(&encode_component(value));
}

/// Percent-encode everything but the query-string unreserved set.
fn encode_component(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Decode one percent-encoded component (`+` reads as a space, as it does in
/// every form-encoded query the console sees).
fn decode_component(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 3 <= bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                    .ok()
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok());
                match hex {
                    Some(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    None => {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One of the pane's own actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneAction {
    /// Open the resource's log stream in the dock.
    ViewLogs,
    /// Attach a terminal / exec session.
    Exec,
    /// Open the resource's YAML in the editor.
    EditYaml,
    /// Delete the resource.
    Delete,
}

impl PaneAction {
    /// Every action the pane offers, in render order.
    pub const ALL: [PaneAction; 4] = [
        PaneAction::ViewLogs,
        PaneAction::Exec,
        PaneAction::EditYaml,
        PaneAction::Delete,
    ];

    /// Stable id, carried to the handler and rendered as `data-action`.
    pub fn id(self) -> &'static str {
        match self {
            PaneAction::ViewLogs => "view-logs",
            PaneAction::Exec => "exec",
            PaneAction::EditYaml => "edit-yaml",
            PaneAction::Delete => "delete",
        }
    }

    /// The action's label.
    pub fn label(self) -> &'static str {
        match self {
            PaneAction::ViewLogs => "View logs",
            PaneAction::Exec => "Exec",
            PaneAction::EditYaml => "Edit YAML",
            PaneAction::Delete => "Delete",
        }
    }

    /// Why the action cannot run when no host handler is wired. The console's
    /// rule is that a control either performs an action or says why it cannot.
    pub fn reason(self) -> &'static str {
        match self {
            PaneAction::ViewLogs => "log streaming opens in the dock",
            PaneAction::Exec => "exec needs a terminal host",
            PaneAction::EditYaml => "the editor host supplies the document",
            PaneAction::Delete => "delete needs a mutation host",
        }
    }

    /// The action's button styling: the destructive one is the only primary
    /// weight the pane carries.
    pub fn class(self) -> &'static str {
        match self {
            PaneAction::Delete => "btn btn-danger",
            _ => "btn btn-secondary",
        }
    }
}

/// Everything the pane paints: identity, status, metadata, controller owner
/// and the pane's own actions.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceDetail {
    /// The resource this pane shows.
    pub identity: ResourceRef,
    /// Status label plus the semantic kind its pill paints.
    pub status: Option<(String, StatusKind)>,
    /// The metadata rows a Lens user expects, in render order.
    pub metadata: Vec<(String, String)>,
    /// Controller owner (`Deployment/checkout-api`), when the resource has one.
    pub owner: Option<String>,
    /// The pane's own actions.
    pub actions: Vec<PaneAction>,
}

impl ResourceDetail {
    /// A detail with identity only — what an address alone can restore.
    pub fn from_ref(identity: ResourceRef) -> Self {
        Self {
            identity,
            status: None,
            metadata: Vec::new(),
            owner: None,
            actions: PaneAction::ALL.to_vec(),
        }
    }

    /// Build the pane's content from a table row: the row the row-selection
    /// path already has. The row's own cells supply the metadata, so the pane
    /// shows what the list already knows without a second fetch, and a kind
    /// with no extra columns simply renders fewer rows.
    pub fn from_row(kind: &str, columns: &[ColumnDef], row: &ResourceRow) -> Self {
        let mut detail = Self::from_ref(ResourceRef::new(
            kind,
            row.namespace.clone(),
            row.id.clone(),
        ));

        for (index, column) in columns.iter().enumerate().skip(1) {
            let Some(cell) = row.cells.get(index) else {
                continue;
            };
            if cell.text.is_empty() || cell.text == "—" {
                continue;
            }
            if detail.status.is_none() {
                if let Some(status) = cell.status {
                    detail.status = Some((cell.text.clone(), status));
                    continue;
                }
            }
            if is_owner_column(column) {
                detail.owner = Some(cell.text.clone());
                continue;
            }
            detail
                .metadata
                .push((column.label.to_string(), cell.text.clone()));
        }

        detail
    }

    /// The selection this detail addresses.
    pub fn query(&self) -> String {
        self.identity.query()
    }

    /// Identity key (see [`ResourceRef::key`]).
    pub fn key(&self) -> String {
        self.identity.key()
    }

    /// Add one metadata row.
    pub fn with_metadata(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.push((label.into(), value.into()));
        self
    }

    /// Set the status row.
    pub fn with_status(mut self, label: impl Into<String>, kind: StatusKind) -> Self {
        self.status = Some((label.into(), kind));
        self
    }

    /// Set the controller owner.
    pub fn with_owner(mut self, owner: impl Into<String>) -> Self {
        self.owner = Some(owner.into());
        self
    }
}

/// True for the column whose cell carries the controller owner.
fn is_owner_column(column: &ColumnDef) -> bool {
    column.key.eq_ignore_ascii_case("controller")
        || column.label.to_lowercase().contains("controller")
}

/// Open the pane for `detail`. One slot: this REPLACES whatever the pane was
/// showing (a second click never stacks), and it is the row path every kind
/// goes through.
pub fn open_resource(detail: ResourceDetail) {
    select_resource(detail);
}

/// Open the pane for a row, or close it when the same row is clicked again.
pub fn toggle_resource(detail: ResourceDetail) {
    let same = selected_resource()
        .map(|current| current.key() == detail.key())
        .unwrap_or(false);
    if same {
        clear_resource_selection();
    } else {
        select_resource(detail);
    }
}

/// The row-selection path: click a table row and the pane follows. Wire it to
/// `ResourceTable`'s `on_row_click`:
///
/// ```ignore
/// on_row_click: move |row: ResourceRow| toggle_resource_row("Pod", &columns, &row),
/// ```
pub fn toggle_resource_row(kind: &str, columns: &[ColumnDef], row: &ResourceRow) {
    toggle_resource(ResourceDetail::from_row(kind, columns, row));
}

/// The width-restore, drag, Escape and focus-trap script. One script installs
/// every behaviour the pane needs from a document; it is a no-op for a closed
/// pane and re-applies the stored width each time the pane mounts.
fn pane_script() -> String {
    let key = serde_json::to_string(PANE_WIDTH_KEY).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(function () {{
  var pane = document.querySelector('.inspector[data-pane="resource"]');
  if (!pane) return;
  var min = parseInt(pane.dataset.paneMin || '{min}', 10);
  var max = parseInt(pane.dataset.paneMax || '{max}', 10);
  var key = {key};
  var clamp = function (px) {{ return Math.min(max, Math.max(min, px)); }};
  var stored = null;
  try {{ stored = localStorage.getItem(key); }} catch (e) {{ stored = null; }}
  var px = parseInt(stored, 10);
  if (!isNaN(px)) {{ pane.style.setProperty('--pane-width', clamp(px) + 'px'); }}

  var handle = pane.querySelector('.inspector-resize');
  if (handle && !handle.dataset.bound) {{
    handle.dataset.bound = '1';
    handle.addEventListener('pointerdown', function (down) {{
      down.preventDefault();
      var startX = down.clientX;
      var startWidth = pane.getBoundingClientRect().width;
      var move = function (event) {{
        pane.style.setProperty('--pane-width', clamp(startWidth + (startX - event.clientX)) + 'px');
      }};
      var up = function () {{
        window.removeEventListener('pointermove', move);
        window.removeEventListener('pointerup', up);
        var final = Math.round(pane.getBoundingClientRect().width);
        try {{ localStorage.setItem(key, String(final)); }} catch (e) {{}}
      }};
      window.addEventListener('pointermove', move);
      window.addEventListener('pointerup', up);
    }});
  }}

  if (window.__openkite_pane_installed) return;
  window.__openkite_pane_installed = true;

  var openPane = function () {{ return document.querySelector('.inspector[data-pane="resource"]'); }};

  document.addEventListener('keydown', function (event) {{
    var open = openPane();
    if (!open || event.key !== 'Escape') return;
    var close = open.querySelector('.inspector-close');
    if (close) close.click();
  }});

  document.addEventListener('keydown', function (event) {{
    if (event.key !== 'Tab') return;
    var open = openPane();
    if (!open) return;
    var active = document.activeElement;
    if (!active || !open.contains(active)) return;
    var focusable = open.querySelectorAll('button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])');
    if (!focusable.length) return;
    var first = focusable[0];
    var last = focusable[focusable.length - 1];
    if (event.shiftKey && active === first) {{ event.preventDefault(); last.focus(); }}
    else if (!event.shiftKey && active === last) {{ event.preventDefault(); first.focus(); }}
  }});
}})();"#,
        min = PANE_MIN_WIDTH,
        max = PANE_MAX_WIDTH,
        key = key,
    )
}

/// Keep the address in step with the selection, so a reload or a shared link
/// reopens the same pane. A closed pane drops the three parameters.
fn url_sync_script(query: Option<&str>) -> String {
    match query {
        Some(query) => {
            let query = serde_json::to_string(query).unwrap_or_else(|_| "\"\"".into());
            format!(
                r#"(function () {{
  try {{
    var url = new URL(window.location.href);
    new URLSearchParams({query}).forEach(function (value, name) {{ url.searchParams.set(name, value); }});
    history.replaceState(null, '', url.pathname + (url.search || '') + url.hash);
  }} catch (e) {{}}
}})();"#,
                query = query
            )
        }
        None => format!(
            r#"(function () {{
  try {{
    var url = new URL(window.location.href);
    ['{kind}', '{ns}', '{name}'].forEach(function (name) {{ url.searchParams.delete(name); }});
    history.replaceState(null, '', url.pathname + (url.search || '') + url.hash);
  }} catch (e) {{}}
}})();"#,
            kind = KIND_PARAM,
            ns = NAMESPACE_PARAM,
            name = NAME_PARAM,
        ),
    }
}

/// The pane itself. Mounted once, by the shell, in the same slot on every
/// route: it renders nothing while nothing is selected.
#[component]
pub fn ResourcePane(#[props(default)] on_action: Option<EventHandler<PaneAction>>) -> Element {
    // The document-owned behaviours. Both effects read the selection so they
    // re-run when it changes (and are no-ops while the pane is closed).
    use_effect(move || {
        if RESOURCE_SELECTION.read().is_some() {
            document::eval(&pane_script());
        }
    });

    use_effect(move || {
        let query = RESOURCE_SELECTION
            .read()
            .as_ref()
            .map(|detail| detail.query());
        document::eval(&url_sync_script(query.as_deref()));
    });

    let Some(detail) = RESOURCE_SELECTION.read().clone() else {
        return rsx! {};
    };

    let identity = detail.identity.clone();
    let namespace = identity
        .namespace
        .clone()
        .unwrap_or_else(|| "cluster-scoped".to_string());
    let title = identity.name.clone();
    let kind = identity.kind.clone();
    let status = detail.status.clone();
    let owner = detail.owner.clone();
    let metadata = detail.metadata.clone();
    let actions = detail.actions.clone();
    let wired = on_action.is_some();

    rsx! {
        // Touch only: the stylesheet shows this scrim at ≤767px, where the pane
        // is the full-width bottom sheet and the list behind it must stay
        // discoverable. Above that width it is inert, so clicking another row
        // still replaces the pane's content in place.
        button {
            class: "inspector-scrim show",
            r#type: "button",
            "aria-label": "Close resource details",
            "data-scrim": "resource-pane",
            onclick: move |_| clear_resource_selection(),
        }
        aside {
            class: "inspector open",
            "data-pane": "resource",
            "data-kind": "{kind}",
            "data-pane-min": "{PANE_MIN_WIDTH}",
            "data-pane-max": "{PANE_MAX_WIDTH}",
            "aria-label": "Resource details",
            div { class: "inspector-resize",
                role: "separator",
                "aria-label": "Resize resource details",
                "aria-orientation": "vertical",
                "data-resize": "resource-pane",
            }
            div { class: "inspector-header",
                div { class: "inspector-title",
                    h2 { "{title}" }
                    span { class: "resource-kind", "{kind}" }
                    button {
                        class: "icon-btn inspector-close",
                        r#type: "button",
                        "aria-label": "Close resource details",
                        "data-close": "resource-pane",
                        onclick: move |_| clear_resource_selection(),
                        svg { class: "icon", "viewBox": "0 0 24 24",
                            path { d: "M6 6l12 12M18 6L6 18" }
                        }
                    }
                }
                div { class: "inspector-meta",
                    span { "namespace: {namespace}" }
                }
            }
            div { class: "inspector-body",
                div { class: "inspector-eyebrow", "Resource summary" }
                dl { class: "kv-list",
                    div { class: "kv-row", dt { "Kind" }, dd { "{kind}" } }
                    div { class: "kv-row", dt { "Namespace" }, dd { "{namespace}" } }
                    div { class: "kv-row", dt { "Status" },
                        dd {
                            match status {
                                Some((_, kind)) => rsx! { StatusPill { status: kind } },
                                None => rsx! { "—" },
                            }
                        }
                    }
                    if let Some(owner) = owner.clone() {
                        div { class: "kv-row", dt { "Owner" }, dd { "{owner}" } }
                    } else {
                        div { class: "kv-row", dt { "Owner" }, dd { "—" } }
                    }
                    for (label, value) in metadata.iter() {
                        div { key: "{label}", class: "kv-row",
                            dt { "{label}" }
                            dd { "{value}" }
                        }
                    }
                }
                div { class: "inspector-actions",
                    for action in actions.iter() {
                        {
                            let action = *action;
                            let handler = on_action;
                            let id = action.id();
                            rsx! {
                                button {
                                    key: "{id}",
                                    class: "{action.class()}",
                                    r#type: "button",
                                    "data-action": "{id}",
                                    disabled: !wired,
                                    title: if wired { "" } else { action.reason() },
                                    onclick: move |_| {
                                        if let Some(handler) = handler {
                                            handler.call(action);
                                        }
                                    },
                                    "{action.label()}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_round_trips_the_selection() {
        let reference = ResourceRef::new("Pod", Some("default".into()), "web-1");
        assert_eq!(reference.query(), "kind=Pod&ns=default&name=web-1");
        assert_eq!(ResourceRef::from_query(&reference.query()), Some(reference));
    }

    #[test]
    fn cluster_scoped_selections_have_no_namespace_parameter() {
        let reference = ResourceRef::new("Node", None, "ip-10-0-4-18");
        assert_eq!(reference.query(), "kind=Node&name=ip-10-0-4-18");
        assert_eq!(
            ResourceRef::from_query("kind=Node&name=ip-10-0-4-18"),
            Some(reference)
        );
    }

    #[test]
    fn unknown_parameters_are_ignored_and_incomplete_ones_select_nothing() {
        assert_eq!(
            ResourceRef::from_query("?tab=events&kind=Service&name=api&ns=prod"),
            Some(ResourceRef::new("Service", Some("prod".into()), "api"))
        );
        assert_eq!(ResourceRef::from_query("kind=Pod"), None);
        assert_eq!(ResourceRef::from_query("name=api"), None);
        assert_eq!(ResourceRef::from_query(""), None);
    }

    #[test]
    fn values_are_percent_encoded_and_decoded() {
        let reference = ResourceRef::new("Application", Some("argocd".into()), "guest/book");
        let query = reference.query();
        assert!(query.contains("name=guest%2Fbook"), "got: {query}");
        assert_eq!(ResourceRef::from_query(&query), Some(reference));
    }

    #[test]
    fn address_carries_the_selection_on_a_route() {
        let reference = ResourceRef::new("Pod", Some("default".into()), "web-1");
        assert_eq!(
            reference.address("/workloads"),
            "/workloads?kind=Pod&ns=default&name=web-1"
        );
    }

    #[test]
    fn keys_ignore_kind_casing_and_the_namespace_half() {
        let pod = ResourceRef::new("Pod", Some("default".into()), "web-1");
        let same = ResourceRef::new("pod", Some("default".into()), "web-1");
        let other = ResourceRef::new("Pod", Some("other".into()), "web-1");
        assert_eq!(pod.key(), same.key());
        assert_ne!(pod.key(), other.key());
    }

    #[test]
    fn widths_clamp_to_the_pane_range() {
        assert_eq!(clamp_pane_width(100.0), PANE_MIN_WIDTH);
        assert_eq!(clamp_pane_width(420.4), 420);
        assert_eq!(clamp_pane_width(9000.0), PANE_MAX_WIDTH);
        assert_eq!(clamp_pane_width(f64::NAN), PANE_MAX_WIDTH);
    }

    #[test]
    fn persisted_widths_parse_only_inside_the_range() {
        assert_eq!(parse_pane_width("512"), Some(512));
        assert_eq!(parse_pane_width(""), None);
        assert_eq!(parse_pane_width("wide"), None);
        assert_eq!(parse_pane_width("12"), None);
        assert_eq!(parse_pane_width("9000"), None);
    }

    #[test]
    fn action_ids_and_classes_are_stable() {
        let ids: Vec<&str> = PaneAction::ALL.iter().map(|action| action.id()).collect();
        assert_eq!(ids, vec!["view-logs", "exec", "edit-yaml", "delete"]);
        assert!(PaneAction::Delete.class().contains("btn-danger"));
        for action in PaneAction::ALL {
            assert!(!action.reason().is_empty(), "{action:?} needs a reason");
        }
    }

    #[test]
    fn the_pane_script_reads_bounds_and_the_storage_key_off_the_element() {
        let script = pane_script();
        assert!(script.contains("data-pane=\"resource\""), "got: {script}");
        assert!(script.contains("--pane-width"), "got: {script}");
        assert!(script.contains(PANE_WIDTH_KEY), "got: {script}");
        assert!(script.contains("inspector-resize"), "got: {script}");
        assert!(script.contains(".inspector-close"), "got: {script}");
    }

    #[test]
    fn the_url_sync_script_sets_and_drops_the_selection_parameters() {
        let set = url_sync_script(Some("kind=Pod&ns=default&name=web-1"));
        assert!(set.contains("kind=Pod"), "got: {set}");
        assert!(set.contains("searchParams.set"), "got: {set}");
        let drop = url_sync_script(None);
        assert!(drop.contains("searchParams.delete"), "got: {drop}");
        assert!(
            drop.contains(KIND_PARAM) && drop.contains(NAME_PARAM),
            "got: {drop}"
        );
    }
}
