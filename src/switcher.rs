//! Cluster context switcher: a Ctrl+Tab overlay (omarchy-style launcher).
//!
//! [`SwitcherKeybind`] installs a webview-level key listener via the eval
//! channel (works regardless of focus) and toggles [`SWITCHER_OPEN`];
//! [`ClusterSwitcher`] renders the centered overlay while open. Selecting a
//! context connects through the process-global [`crate::cluster::SHARED`]
//! registry (cached clients — switching back is instant), swaps the bridge
//! client, and publishes the new client/context so every view re-renders and
//! re-aims its reflector.

use dioxus::prelude::*;

/// Whether the switcher overlay is open.
pub static SWITCHER_OPEN: GlobalSignal<bool> = Signal::global(|| false);

/// The current filter query (cleared on every open/close).
pub static SWITCHER_QUERY: GlobalSignal<String> = Signal::global(String::new);

/// Last switch error, rendered under the search field until dismissed.
pub static SWITCHER_ERROR: GlobalSignal<Option<String>> = Signal::global(|| None::<String>);

/// Close the overlay and reset its transient state.
fn close_switcher() {
    *SWITCHER_OPEN.write() = false;
    *SWITCHER_QUERY.write() = String::new();
    *SWITCHER_ERROR.write() = None;
}

/// Filter context names by a case-insensitive substring query. A blank query
/// returns all names in kubeconfig order; matches are ordered by match
/// position (earliest first, stable for ties).
pub fn filter_contexts(names: &[String], query: &str) -> Vec<String> {
    if query.trim().is_empty() {
        return names.to_vec();
    }
    let needle = query.trim().to_lowercase();
    let mut hits: Vec<(usize, String)> = names
        .iter()
        .filter_map(|name| {
            name.to_lowercase()
                .find(&needle)
                .map(|pos| (pos, name.clone()))
        })
        .collect();
    hits.sort_by_key(|(pos, _)| *pos);
    hits.into_iter().map(|(_, name)| name).collect()
}

/// Advance a selection index by `delta` with wrapping. `None` when the list
/// is empty; a stale out-of-range selection is clamped first.
pub fn advance_index(selected: Option<usize>, len: usize, delta: isize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let base = selected.unwrap_or(0).min(len - 1) as isize;
    Some((base + delta).rem_euclid(len as isize) as usize)
}

/// Connect to `context` through the shared cluster registry, wire the new
/// client into the runtime globals and the plugin bridge, and publish the
/// context name. Views re-run their fetch effects on the client change.
async fn select_context(context: String) -> Result<(), String> {
    let registry = crate::cluster::SHARED
        .get()
        .ok_or_else(|| "cluster registry unavailable".to_string())?;
    let mut guard = registry.lock().await;
    let client = guard
        .connect(&context)
        .await
        .map_err(|error| format!("{error:#}"))?;
    crate::runtime::set_client(Some(client.clone()));
    crate::runtime::set_context(Some(context));
    if let Some(bridge) = crate::runtime::bridge() {
        bridge.set_client(Some(client));
    }
    Ok(())
}

/// Begin switching to `context`: close the overlay, connect in the
/// background; reopen with the error visible if the switch fails.
fn pick_context(context: String) {
    close_switcher();
    spawn(async move {
        if let Err(error) = select_context(context).await {
            *SWITCHER_ERROR.write() = Some(error);
            *SWITCHER_OPEN.write() = true;
        }
    });
}

/// Keybind listener source: installs once per webview. Ctrl+Tab toggles the
/// switcher (preventing the native focus-cycle default), Escape closes it
/// from anywhere — both flow back over the eval channel.
const KEYBIND_JS: &str = r#"
if (!window.__openkite_switcher_keys) {
  window.__openkite_switcher_keys = true;
  document.addEventListener('keydown', (event) => {
    if (event.ctrlKey && !event.altKey && !event.metaKey && event.key === 'Tab') {
      event.preventDefault();
      dioxus.send('toggle');
    } else if (event.key === 'Escape') {
      dioxus.send('close');
    }
  });
}
"#;

/// Webview-level Ctrl+Tab / Escape keybind. Mounted once by the app shell;
/// the effect runs post-mount (DOM ready) and serves channel messages from
/// a spawned task for the life of the process.
#[component]
pub fn SwitcherKeybind() -> Element {
    use_effect(move || {
        let mut eval = document::eval(KEYBIND_JS);
        spawn(async move {
            while let Ok(action) = eval.recv::<String>().await {
                match action.as_str() {
                    "toggle" => {
                        if *SWITCHER_OPEN.read() {
                            close_switcher();
                        } else {
                            *SWITCHER_ERROR.write() = None;
                            *SWITCHER_OPEN.write() = true;
                        }
                    }
                    "close" => close_switcher(),
                    _ => {}
                }
            }
        });
    });
    rsx! {}
}

/// The mounted overlay: renders only while open (`SWITCHER_OPEN`).
#[component]
pub fn ClusterSwitcher() -> Element {
    let open = *SWITCHER_OPEN.read();
    rsx! {
        if open {
            SwitcherPanel {}
        }
    }
}

/// Overlay panel: filter field + context list + inline error. Owns the
/// selection cursor; selecting (click or Enter) connects immediately.
#[component]
fn SwitcherPanel() -> Element {
    let contexts = crate::runtime::CONTEXTS.read().clone();
    let query = SWITCHER_QUERY.read().clone();
    let error_line = SWITCHER_ERROR.read().clone().unwrap_or_default();
    let active = crate::runtime::context_name();
    let candidates = filter_contexts(&contexts, &query);
    let mut selected = use_signal(|| 0usize);
    // Clamp each render: a shrinking query can orphan the cursor.
    let cursor = (*selected.read()).min(candidates.len().saturating_sub(1));

    rsx! {
        div {
            class: "switcher-backdrop",
            onclick: move |_| close_switcher(),
            div {
                class: "switcher",
                onclick: move |event| event.stop_propagation(),
                input {
                    class: "switcher-input",
                    r#type: "text",
                    placeholder: "filter contexts…",
                    autofocus: true,
                    value: "{query}",
                    oninput: move |event| {
                        *SWITCHER_QUERY.write() = event.value();
                        selected.set(0);
                    },
                    onkeydown: {
                        let list = candidates.clone();
                        move |event| match event.key() {
                            Key::ArrowDown => {
                                if let Some(next) = advance_index(Some(cursor), list.len(), 1) {
                                    selected.set(next);
                                }
                            }
                            Key::ArrowUp => {
                                if let Some(prev) = advance_index(Some(cursor), list.len(), -1) {
                                    selected.set(prev);
                                }
                            }
                            Key::Enter => {
                                if let Some(name) = list.get(cursor) {
                                    pick_context(name.clone());
                                }
                            }
                            Key::Escape => close_switcher(),
                            _ => {}
                        }
                    },
                }
                if !error_line.is_empty() {
                    div { class: "switcher-error", "{error_line}" }
                }
                div {
                    class: "switcher-list",
                    {candidates.iter().enumerate().map(|(idx, name)| {
                        let pick = name.clone();
                        let is_selected = idx == cursor;
                        let is_active = Some(name.as_str()) == active.as_deref();
                        let row_class = if is_selected {
                            "switcher-row selected"
                        } else {
                            "switcher-row"
                        };
                        rsx! {
                            div {
                                key: "{name}",
                                class: row_class,
                                onclick: move |_| pick_context(pick.clone()),
                                if is_active {
                                    span {
                                        class: "switcher-connected-dot",
                                        title: "connected",
                                    }
                                }
                                "{name}"
                            }
                        }
                    })}
                    if candidates.is_empty() {
                        div { class: "switcher-empty", "no matching context" }
                    }
                }
            }
        }
    }
}
