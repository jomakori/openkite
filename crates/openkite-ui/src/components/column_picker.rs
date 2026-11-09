//! Column visibility for the resource table: the toolbar picker, plus the
//! session-scoped preference it reads and writes.

use std::collections::BTreeSet;

use dioxus::prelude::*;

use crate::components::resource_table::ColumnDef;

/// Session-scoped column visibility, keyed by [`ColumnDef::key`].
///
/// Keyed by column key rather than by table, so a column reads the same way on
/// every list, and held process-wide so leaving the route and coming back —
/// which remounts the table — keeps the choice. Not persisted to disk: a new
/// session starts with every column visible.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColumnPrefs {
    hidden: BTreeSet<String>,
}

impl ColumnPrefs {
    /// Is `key` shown?
    pub fn is_visible(&self, key: &str) -> bool {
        !self.hidden.contains(key)
    }

    /// How many columns the user has hidden.
    pub fn hidden_count(&self) -> usize {
        self.hidden.len()
    }

    /// Indices into `columns` of the columns still shown, in table order.
    pub fn visible_indices(&self, columns: &[ColumnDef]) -> Vec<usize> {
        columns
            .iter()
            .enumerate()
            .filter(|(_, column)| self.is_visible(&column.key))
            .map(|(index, _)| index)
            .collect()
    }

    /// The prefs after toggling `key`. Unknown keys are a no-op, and hiding
    /// the last visible column is refused: a table with no columns has no
    /// affordance left to get them back.
    pub fn toggled(&self, key: &str, columns: &[ColumnDef]) -> Self {
        let mut next = self.clone();
        if !columns.iter().any(|column| column.key == key) {
            return next;
        }
        if self.is_visible(key) {
            if self.visible_indices(columns).len() > 1 {
                next.hidden.insert(key.to_string());
            }
        } else {
            next.hidden.remove(key);
        }
        next
    }
}

/// The session's column visibility: one process-wide store, so a table that is
/// rebuilt by navigation reads the choice the picker made before it left.
///
/// A plain lock rather than a signal on purpose — the store outlives every
/// mount, and components only need a signal for the redraw, which they own.
static SESSION: std::sync::OnceLock<std::sync::RwLock<ColumnPrefs>> = std::sync::OnceLock::new();

fn session() -> &'static std::sync::RwLock<ColumnPrefs> {
    SESSION.get_or_init(|| std::sync::RwLock::new(ColumnPrefs::default()))
}

/// The session preference as currently stored.
pub fn column_prefs() -> ColumnPrefs {
    session()
        .read()
        .map(|prefs| prefs.clone())
        .unwrap_or_default()
}

/// Replace the session preference.
pub fn set_column_prefs(prefs: ColumnPrefs) {
    if let Ok(mut stored) = session().write() {
        *stored = prefs;
    }
}

/// Toggle one column in the session preference, returning the new value for
/// the component's own signal.
pub fn toggle_column(key: &str, columns: &[ColumnDef]) -> ColumnPrefs {
    let next = column_prefs().toggled(key, columns);
    set_column_prefs(next.clone());
    next
}

/// Show every column again.
pub fn show_all_columns() {
    set_column_prefs(ColumnPrefs::default());
}

/// What a keydown inside the picker should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerAction {
    /// Escape: close the panel and put focus back on the trigger.
    CloseAndRefocus,
    /// Everything else: let the browser have the key (Tab, Space on a
    /// checkbox, Enter on the trigger).
    Ignore,
}

/// The picker's keyboard contract, typed so the component and its tests read
/// the same rule.
pub fn picker_action(key: &Key) -> PickerAction {
    if *key == Key::Escape {
        PickerAction::CloseAndRefocus
    } else {
        PickerAction::Ignore
    }
}

/// Focuses this picker's trigger. `id` is the picker root's DOM id, so a
/// toolbar holding more than one picker restores focus to the right trigger.
///
/// Escape is handled in Rust; only the focus call needs the document, which is
/// why it lives here as a string the component evals.
pub fn restore_focus_script(id: &str) -> String {
    format!("document.getElementById('{id}')?.querySelector('.column-picker-trigger')?.focus();")
}

/// Per-mount id for the picker root, so the refocus script has a stable target.
fn next_picker_id() -> String {
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("column-picker-{n}")
}

/// Toolbar control that toggles table columns; the choice is session-scoped
/// (see [`column_prefs`]).
///
/// `prefs` is the owning table's signal: the picker writes it so the table
/// re-renders, and every write also lands in the session store so the next
/// mount starts from the same choice. `initial_open` starts the panel expanded
/// — the toolbar opens it on click, tests seed it to assert the panel's markup.
#[component]
pub fn ColumnPicker(
    columns: Vec<ColumnDef>,
    prefs: Signal<ColumnPrefs>,
    #[props(default = false)] initial_open: bool,
) -> Element {
    let id = use_hook(next_picker_id);
    let mut open = use_signal(|| initial_open);
    let mut refocus = use_signal(|| false);
    let mut prefs = prefs;
    let current = prefs.read().clone();
    let shown = current.visible_indices(&columns).len();
    let total = columns.len();
    let columns_for_toggle = columns.clone();
    let id_for_effect = id.clone();

    // Runs after the panel has left the DOM, so the trigger is the only thing
    // left to focus.
    use_effect(move || {
        if refocus() {
            refocus.set(false);
            let script = restore_focus_script(&id_for_effect);
            spawn(async move {
                let _ = document::eval(&script).await;
            });
        }
    });

    rsx! {
        div {
            id: "{id}",
            class: "column-picker",
            onkeydown: move |event| {
                if open() && picker_action(&event.key()) == PickerAction::CloseAndRefocus {
                    refocus.set(true);
                    open.set(false);
                }
            },
            button {
                r#type: "button",
                class: "btn btn-secondary column-picker-trigger",
                "aria-haspopup": "true",
                "aria-expanded": if open() { "true" } else { "false" },
                onclick: move |_| open.toggle(),
                "Columns"
                span { class: "column-picker-count", "{shown}/{total}" }
            }
            if open() {
                div { class: "column-picker-panel", role: "group", "aria-label": "Visible columns",
                    for column in columns.iter().cloned() {
                        label { class: "column-picker-option",
                            input {
                                r#type: "checkbox",
                                checked: current.is_visible(&column.key),
                                onchange: {
                                    let key = column.key.clone();
                                    let columns = columns_for_toggle.clone();
                                    move |_| {
                                        let next = toggle_column(&key, &columns);
                                        prefs.set(next);
                                    }
                                },
                            }
                            span { "{column.label}" }
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

    fn columns() -> Vec<ColumnDef> {
        vec![
            ColumnDef {
                key: "name".to_string(),
                label: "Name".to_string(),
                width: None,
                sortable: true,
            },
            ColumnDef {
                key: "namespace".to_string(),
                label: "Namespace".to_string(),
                width: None,
                sortable: true,
            },
            ColumnDef {
                key: "node".to_string(),
                label: "Node".to_string(),
                width: None,
                sortable: false,
            },
        ]
    }

    #[test]
    fn toggling_a_column_off_and_on_round_trips() {
        let columns = columns();
        let all: Vec<usize> = (0..columns.len()).collect();
        let hidden = ColumnPrefs::default().toggled("namespace", &columns);
        assert_eq!(hidden.visible_indices(&columns), vec![0, 2]);
        let shown = hidden.toggled("namespace", &columns);
        assert_eq!(shown.visible_indices(&columns), all);
        assert_eq!(shown, ColumnPrefs::default());
    }

    #[test]
    fn the_last_visible_column_cannot_be_hidden() {
        let columns = columns();
        let prefs = ColumnPrefs::default()
            .toggled("name", &columns)
            .toggled("namespace", &columns);
        assert_eq!(prefs.visible_indices(&columns), vec![2]);
        let refused = prefs.toggled("node", &columns);
        assert_eq!(refused.visible_indices(&columns), vec![2]);
    }

    #[test]
    fn unknown_column_keys_do_not_hide_anything() {
        let columns = columns();
        let prefs = ColumnPrefs::default().toggled("age", &columns);
        assert!(prefs.is_visible("age"));
        assert_eq!(prefs.visible_indices(&columns).len(), columns.len());
    }

    #[test]
    fn escape_closes_and_everything_else_is_left_to_the_browser() {
        assert_eq!(picker_action(&Key::Escape), PickerAction::CloseAndRefocus);
        assert_eq!(picker_action(&Key::Tab), PickerAction::Ignore);
        assert_eq!(
            picker_action(&Key::Character(" ".to_string())),
            PickerAction::Ignore
        );
        assert_eq!(picker_action(&Key::Enter), PickerAction::Ignore);
    }

    #[test]
    fn refocus_script_targets_this_pickers_trigger() {
        let script = restore_focus_script("column-picker-7");
        assert!(script.contains("column-picker-7"), "{script}");
        assert!(script.contains(".column-picker-trigger"), "{script}");
        assert!(script.contains(".focus()"), "{script}");
    }
}
