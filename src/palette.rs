//! Host command palette (Cmd+P / Ctrl+P overlay).
//!
//! A centered modal overlay that fuzzy-searches a static, host-defined
//! list of commands and runs them with Enter or click. Reuses the P1
//! fuzzy matcher ([`crate::fuzzy::rank`]) for scoring and the OKT-51
//! cluster switcher's overlay chrome ([`crate::switcher`]) for the
//! backdrop / stop-propagation / `Key::*` / install-once JS keybind
//! shape. The actions are a typed [`CommandAction`] enum covering
//! programmatic navigation via [`dioxus::prelude::use_navigator`],
//! opening the cluster switcher, opening the CRUD modal for new
//! resources, and cycling the opaline theme.
//!
//! The palette is the first `use_navigator()` consumer in the openkite
//! repo and the first "non-route-driven" P2 wrapper that mounts global
//! chrome in `AppShell` (sibling to the cluster switcher).

use crate::router::Route;

/// What running a [`Command`] does. Static-only, host-defined for v1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    /// Programmatic nav via `use_navigator().push(Route::X)`.
    Navigate(Route),
    /// Open the cluster switcher overlay (sets `SWITCHER_OPEN = true`).
    /// Reuses the OKT-51 switcher; the palette does not duplicate its
    /// connection logic.
    SwitchCluster,
    /// Cycle through the opaline theme catalog.
    CycleTheme,
    /// Open the CRUD modal for a new resource of the given `kind_str`
    /// (e.g. `"pods"`, `"deployments"`). Delegates to
    /// `runtime::open_new_for(kind_str)`.
    NewResource(&'static str),
}

/// One entry in the palette registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// Stable id (used by the registry tests and the future
    /// plugin-registered `openkite.registerCommand({ id, ... })`
    /// deserialization).
    pub id: &'static str,
    /// User-visible label (fuzzy-matched against the query).
    pub label: &'static str,
    /// Section group for the `.palette-section` row label.
    pub section: &'static str,
    /// Optional secondary line (rendered today as a `title=` attribute).
    pub description: &'static str,
    /// What running the command does.
    pub action: CommandAction,
}

/// The static command registry: the palette's source of truth. ~12
/// entries in three sections. The order is the fallback display order
/// when the query is blank; the fuzzy rank overrides on a non-blank
/// query.
pub fn commands() -> Vec<Command> {
    vec![
        // ── View ────────────────────────────────────────────────
        Command {
            id: "view.workloads",
            label: "Go to Workloads",
            section: "View",
            description: "Open the workloads table",
            action: CommandAction::Navigate(Route::Workloads {}),
        },
        Command {
            id: "view.logs",
            label: "Go to Logs",
            section: "View",
            description: "Open the log viewer",
            action: CommandAction::Navigate(Route::Logs {}),
        },
        Command {
            id: "view.cluster",
            label: "Go to Cluster",
            section: "View",
            description: "Open the cluster overview",
            action: CommandAction::Navigate(Route::Cluster {}),
        },
        Command {
            id: "view.config",
            label: "Go to Config",
            section: "View",
            description: "Open the config views",
            action: CommandAction::Navigate(Route::Config {}),
        },
        Command {
            id: "view.home",
            label: "Go to Home",
            section: "View",
            description: "Return to the home screen",
            action: CommandAction::Navigate(Route::Home {}),
        },
        // ── Cluster ─────────────────────────────────────────────
        Command {
            id: "cluster.switch",
            label: "Switch Cluster…",
            section: "Cluster",
            description: "Open the cluster context switcher",
            action: CommandAction::SwitchCluster,
        },
        // ── Action (CRUD) ───────────────────────────────────────
        Command {
            id: "new.pod",
            label: "New Pod…",
            section: "Action",
            description: "Open the create-pod editor",
            action: CommandAction::NewResource("pods"),
        },
        Command {
            id: "new.deployment",
            label: "New Deployment…",
            section: "Action",
            description: "Open the create-deployment editor",
            action: CommandAction::NewResource("deployments"),
        },
        Command {
            id: "new.service",
            label: "New Service…",
            section: "Action",
            description: "Open the create-service editor",
            action: CommandAction::NewResource("services"),
        },
        Command {
            id: "new.configmap",
            label: "New ConfigMap…",
            section: "Action",
            description: "Open the create-configmap editor",
            action: CommandAction::NewResource("configmaps"),
        },
        Command {
            id: "new.secret",
            label: "New Secret…",
            section: "Action",
            description: "Open the create-secret editor",
            action: CommandAction::NewResource("secrets"),
        },
        // ── Settings ────────────────────────────────────────────
        Command {
            id: "settings.theme",
            label: "Cycle Theme",
            section: "Settings",
            description: "Switch to the next opaline theme",
            action: CommandAction::CycleTheme,
        },
    ]
}

/// Filter commands by a fuzzy query. A blank (or whitespace-only)
/// query returns all commands in registry order. Otherwise calls
/// [`crate::fuzzy::rank`] verbatim against `Command.label` (the v1
/// contract: description is a tooltip, not a search target).
pub fn filter_commands(commands: &[Command], query: &str) -> Vec<Command> {
    if query.trim().is_empty() {
        return commands.to_vec();
    }
    crate::fuzzy::rank(query, commands.iter().map(|c| (c.label, c.clone())))
        .into_iter()
        .map(|(_, c)| c)
        .collect()
}

/// Advance the selection by `delta` with wrapping. `None` when the
/// list is empty. Mirrors `switcher::advance_index` at
/// `src/switcher.rs:51-57` (the two-helpers-aren't-shared-at-N=2
/// tradeoff documented in the OKT-41 plan).
pub fn advance_cursor(selected: Option<usize>, len: usize, delta: isize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let base = selected.unwrap_or(0).min(len - 1) as isize;
    Some((base + delta).rem_euclid(len as isize) as usize)
}

// ─────────────────────────────────────────────────────────────────────
// Dioxus components live below this line.
//
// The `use dioxus::prelude::*;` glob MUST be at module scope (not
// inside an `#[component]` body) per the openkite-dev Dioxus 0.7
// gotchas. The pure-logic helpers above deliberately do not import
// the dioxus prelude so they stay unit-testable without glib-2.0.
// ─────────────────────────────────────────────────────────────────────

use dioxus::prelude::*;

/// Whether the palette overlay is open.
pub static PALETTE_OPEN: GlobalSignal<bool> = Signal::global(|| false);

/// The current palette filter query (cleared on every open/close).
pub static PALETTE_QUERY: GlobalSignal<String> = Signal::global(String::new);

/// Close the palette and reset its transient state.
fn close_palette() {
    *PALETTE_OPEN.write() = false;
    *PALETTE_QUERY.write() = String::new();
}

/// Keybind listener source: installs once per webview. Cmd+P (mac)
/// or Ctrl+P (linux/win) toggles the palette (preventing the native
/// default), Escape closes it from anywhere — both flow back over the
/// eval channel.
const KEYBIND_JS: &str = r#"
if (!window.__openkite_palette_keys) {
  window.__openkite_palette_keys = true;
  document.addEventListener('keydown', (event) => {
    if ((event.metaKey || event.ctrlKey) && !event.altKey && event.key === 'p') {
      event.preventDefault();
      dioxus.send('toggle');
    } else if (event.key === 'Escape') {
      dioxus.send('close');
    }
  });
}
"#;

/// Webview-level Cmd+P / Escape keybind. Mounted once by the app
/// shell; the effect runs post-mount (DOM ready) and serves channel
/// messages from a spawned task for the life of the process.
#[component]
pub fn PaletteKeybind() -> Element {
    use_effect(move || {
        let mut eval = document::eval(KEYBIND_JS);
        spawn(async move {
            while let Ok(action) = eval.recv::<String>().await {
                match action.as_str() {
                    "toggle" => {
                        if *PALETTE_OPEN.read() {
                            close_palette();
                        } else {
                            *PALETTE_OPEN.write() = true;
                        }
                    }
                    "close" => close_palette(),
                    _ => {}
                }
            }
        });
    });
    rsx! {}
}

/// The mounted overlay: renders only while open (`PALETTE_OPEN`).
#[component]
pub fn CommandPalette() -> Element {
    let open = *PALETTE_OPEN.read();
    rsx! {
        if open {
            PalettePanel {}
        }
    }
}

/// Dispatch a [`CommandAction`]. Closes the palette first, then runs
/// the action. `nav` is the `use_navigator()` handle (Copy, so the
/// per-row click closures capture it by copy).
fn run_command(cmd: Command, nav: Navigator) {
    match cmd.action {
        CommandAction::Navigate(route) => {
            close_palette();
            nav.push(route);
        }
        CommandAction::SwitchCluster => {
            close_palette();
            *crate::switcher::SWITCHER_OPEN.write() = true;
        }
        CommandAction::CycleTheme => {
            close_palette();
            cycle_theme();
        }
        CommandAction::NewResource(kind) => {
            close_palette();
            crate::runtime::open_new_for(kind.to_string());
        }
    }
}

/// Cycle to the next opaline theme in `theme_catalog::catalog()`. Sync
/// `OpenKiteConfig::load()` (one disk read) + `theme::resolve` + a
/// `document::eval` applying the CSS variables + best-effort persist.
/// The persisted `OpenKiteConfig.theme` is the source of truth across
/// reloads; the eval sets `documentElement.style.cssText` until then.
fn cycle_theme() {
    let mut config = crate::config::OpenKiteConfig::load();
    let current = config.theme.as_deref();
    let catalog = crate::theme_catalog::catalog();
    let next = catalog
        .iter()
        .position(|t| crate::theme_catalog::matches_current(&t.id, current))
        .map(|i| (i + 1) % catalog.len())
        .and_then(|i| catalog.get(i).map(|t| t.id.clone()))
        .unwrap_or_else(|| "default".to_string());
    let resolved = crate::theme::resolve(Some(&next));
    let css = resolved.to_css_vars();
    let source = format!(
        r#"document.documentElement.style.cssText = {css_json};"#,
        css_json = serde_json::to_string(&css).unwrap_or_else(|_| "\"\"".into()),
    );
    let _ = document::eval(&source);
    config.theme = Some(next);
    let _ = config.save();
}

/// Overlay panel: filter field + command list. Owns the selection
/// cursor; selecting (click or Enter) dispatches the [`CommandAction`].
#[component]
fn PalettePanel() -> Element {
    let nav = use_navigator();
    let all = commands();
    let query = PALETTE_QUERY.read().clone();
    let candidates = filter_commands(&all, &query);
    let mut selected = use_signal(|| 0usize);
    // Clamp each render: a shrinking query can orphan the cursor.
    let cursor = (*selected.read()).min(candidates.len().saturating_sub(1));

    // Precompute rows + section groups OUTSIDE rsx! (skill rule: no
    // `let` with method-call / nested-call RHS inside the macro).
    let rows: Vec<(usize, Command)> =
        candidates.iter().cloned().enumerate().collect();
    let mut grouped: Vec<(String, Vec<(usize, Command)>)> = Vec::new();
    for (idx, cmd) in rows {
        match grouped.last_mut() {
            Some((section, _)) if section == &cmd.section => {}
            _ => grouped.push((cmd.section.to_string(), Vec::new())),
        }
        if let Some((_, bucket)) = grouped.last_mut() {
            bucket.push((idx, cmd));
        }
    }

    rsx! {
        div {
            class: "palette-backdrop",
            onclick: move |_| close_palette(),
            div {
                class: "palette",
                onclick: move |event| event.stop_propagation(),
                input {
                    class: "palette-input",
                    r#type: "text",
                    placeholder: "Type a command… (⌘P)",
                    autofocus: true,
                    value: "{query}",
                    oninput: move |event| {
                        *PALETTE_QUERY.write() = event.value();
                        selected.set(0);
                    },
                    onkeydown: {
                        let list = candidates.clone();
                        let nav = nav;
                        move |event| match event.key() {
                            Key::ArrowDown => {
                                if let Some(next) = advance_cursor(Some(cursor), list.len(), 1) {
                                    selected.set(next);
                                }
                            }
                            Key::ArrowUp => {
                                if let Some(prev) = advance_cursor(Some(cursor), list.len(), -1) {
                                    selected.set(prev);
                                }
                            }
                            Key::Enter => {
                                if let Some(cmd) = list.get(cursor) {
                                    run_command(cmd.clone(), nav);
                                }
                            }
                            Key::Escape => close_palette(),
                            _ => {}
                        }
                    },
                }
                div { class: "palette-list",
                    if candidates.is_empty() {
                        div { class: "palette-empty", "no matching command" }
                    } else {
                        for (section, bucket) in grouped.iter() {
                            div { class: "palette-section", "{section}" }
                            for (idx, cmd) in bucket.iter().cloned() {
                                PaletteRow {
                                    key: "{cmd.id}",
                                    cmd: cmd,
                                    is_selected: idx == cursor,
                                    nav: nav,
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One row in the palette list. Click or Enter (handled at the panel
/// level) runs the command; the `title=` attribute surfaces the
/// description as a v1 hover tooltip.
#[component]
fn PaletteRow(cmd: Command, is_selected: bool, nav: Navigator) -> Element {
    let row_class = if is_selected {
        "palette-row selected"
    } else {
        "palette-row"
    };
    let cmd_for_click = cmd.clone();
    rsx! {
        div {
            class: row_class,
            title: "{cmd.description}",
            onclick: move |_| run_command(cmd_for_click.clone(), nav),
            "{cmd.label}"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_wraps_both_directions() {
        assert_eq!(advance_cursor(Some(0), 5, 1), Some(1));
        assert_eq!(advance_cursor(Some(0), 5, -1), Some(4));
        assert_eq!(advance_cursor(Some(4), 5, 1), Some(0));
    }

    #[test]
    fn advance_empty_list_is_none() {
        assert_eq!(advance_cursor(Some(0), 0, 1), None);
        assert_eq!(advance_cursor(None, 0, -1), None);
    }

    #[test]
    fn advance_clamps_stale_selection() {
        assert_eq!(advance_cursor(Some(9), 2, 1), Some(0));
        assert_eq!(advance_cursor(Some(9), 2, -1), Some(0));
    }

    #[test]
    fn advance_none_starts_at_zero() {
        assert_eq!(advance_cursor(None, 3, 1), Some(1));
        assert_eq!(advance_cursor(None, 3, 0), Some(0));
    }
}
