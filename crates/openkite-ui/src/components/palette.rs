//! Command palette overlay (Cmd+P / Ctrl+P).
//!
//! A centered modal overlay that fuzzy-searches the command registry with
//! [`crate::fuzzy::rank`] and runs the selection with Enter or click. The
//! chrome — backdrop, filter field, sectioned list, install-once JS keybind —
//! is the cluster switcher's overlay shape, so both overlays behave the same.
//!
//! The registry is host-agnostic: [`PaletteHost`] carries the host's hooks and
//! derives [`PaletteActions`] from them, and a command is offered only for an
//! action the host declares it can run. That keeps the browser host's palette
//! to what a browser can actually do (navigation) instead of listing entries
//! that would silently do nothing. The OS-chrome entries are additionally gated
//! by [`crate::runtime::native_chrome_can_render`], the capability descriptor
//! the host publishes at boot.

use dioxus::prelude::*;

use crate::fuzzy::rank;
use crate::runtime::native_chrome_can_render;

/// The OS decoration theme a title-bar command sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleBarTheme {
    /// Follow the OS decoration theme.
    System,
    /// Force the light decoration theme.
    Light,
    /// Force the dark decoration theme.
    Dark,
}

/// What running a [`Command`] does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    /// Navigate the host's router to a route path (e.g. `"/logs"`).
    Navigate(&'static str),
    /// Open the cluster switcher overlay.
    SwitchCluster,
    /// Cycle through the opaline theme catalog.
    CycleTheme,
    /// Toggle the OS window menu bar.
    ToggleMenuBar,
    /// Set the OS decoration (title bar) theme.
    SetTitleBarTheme(TitleBarTheme),
    /// Open the CRUD modal for a new resource of the given kind (e.g.
    /// `"pods"`).
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
    /// Secondary line (rendered today as a `title=` attribute). Owned so the
    /// `System` title-bar entry can append the theme the OS reports.
    pub description: String,
    /// What running the command does.
    pub action: CommandAction,
}

/// The actions a host runs from the palette.
///
/// The registry is a function of this: a host that cannot complete an action is
/// not offered it, so a browser host lists navigation while the desktop hosts
/// list their theme and OS-chrome entries too.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaletteActions {
    /// Navigate to a route path.
    pub navigate: bool,
    /// Cycle the console theme.
    pub cycle_theme: bool,
    /// Toggle the OS window menu bar.
    pub toggle_menu_bar: bool,
    /// Set the OS decoration theme.
    pub set_title_bar_theme: bool,
    /// Open the CRUD modal for a new resource.
    pub new_resource: bool,
}

/// The actions a host runs on the palette's behalf.
///
/// Every hook is optional, and [`PaletteHost::actions`] derives the registry's
/// view of the host from the hooks actually wired — the palette lists what the
/// host can do, not what the desktop can do.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct PaletteHost {
    /// Navigate the host's router to a route path.
    pub navigate: Option<EventHandler<&'static str>>,
    /// Cycle the console theme.
    pub cycle_theme: Option<EventHandler<()>>,
    /// Toggle the OS window menu bar.
    pub toggle_menu_bar: Option<EventHandler<()>>,
    /// Set the OS decoration theme.
    pub set_title_bar_theme: Option<EventHandler<TitleBarTheme>>,
    /// Open the CRUD modal for a new resource of the given kind.
    pub new_resource: Option<EventHandler<&'static str>>,
}

impl PaletteHost {
    /// A host that can navigate, and nothing else.
    pub fn navigation(navigate: EventHandler<&'static str>) -> Self {
        Self {
            navigate: Some(navigate),
            ..Self::default()
        }
    }

    /// The actions this host's hooks cover.
    pub fn actions(&self) -> PaletteActions {
        PaletteActions {
            navigate: self.navigate.is_some(),
            cycle_theme: self.cycle_theme.is_some(),
            toggle_menu_bar: self.toggle_menu_bar.is_some(),
            set_title_bar_theme: self.set_title_bar_theme.is_some(),
            new_resource: self.new_resource.is_some(),
        }
    }
}

/// The static command registry: the palette's source of truth. The order is
/// the fallback display order when the query is blank; the fuzzy rank
/// overrides on a non-blank query.
pub fn commands(actions: PaletteActions) -> Vec<Command> {
    let mut commands: Vec<Command> = Vec::new();

    if actions.navigate {
        commands.extend([
            Command {
                id: "view.workloads",
                label: "Go to Workloads",
                section: "View",
                description: "Open the workloads table".to_string(),
                action: CommandAction::Navigate("/workloads"),
            },
            Command {
                id: "view.logs",
                label: "Go to Logs",
                section: "View",
                description: "Open the log viewer".to_string(),
                action: CommandAction::Navigate("/logs"),
            },
            Command {
                id: "view.cluster",
                label: "Go to Cluster",
                section: "View",
                description: "Open the cluster overview".to_string(),
                action: CommandAction::Navigate("/cluster"),
            },
            Command {
                id: "view.config",
                label: "Go to Config",
                section: "View",
                description: "Open the config views".to_string(),
                action: CommandAction::Navigate("/config"),
            },
            Command {
                id: "view.home",
                label: "Go to Home",
                section: "View",
                description: "Return to the home screen".to_string(),
                action: CommandAction::Navigate("/"),
            },
        ]);
    }

    if native_chrome_can_render() {
        // The insert order is the render order: the menu-bar toggle lands
        // after the Go-to-* entries, then the three title-bar themes push in
        // front of it.
        commands.insert(
            after_view_home(&commands),
            Command {
                id: "view.toggle-menu-bar",
                label: "Toggle Menu Bar",
                section: "View",
                description: "Hide or show the OS menu bar".to_string(),
                action: CommandAction::ToggleMenuBar,
            },
        );

        let after_home = after_view_home(&commands);
        for (offset, (id, label, description, theme)) in title_bar_themes().into_iter().enumerate()
        {
            commands.insert(
                after_home + offset,
                Command {
                    id,
                    label,
                    section: "View",
                    description: description.to_string(),
                    action: CommandAction::SetTitleBarTheme(theme),
                },
            );
        }
    }

    if crate::runtime::cluster_switch_can_render() {
        commands.push(Command {
            id: "cluster.switch",
            label: "Switch Cluster…",
            section: "Cluster",
            description: "Open the cluster context switcher".to_string(),
            action: CommandAction::SwitchCluster,
        });
    }

    if actions.new_resource {
        for (id, label, kind, description) in [
            ("new.pod", "New Pod…", "pods", "Open the create-pod editor"),
            (
                "new.deployment",
                "New Deployment…",
                "deployments",
                "Open the create-deployment editor",
            ),
            (
                "new.service",
                "New Service…",
                "services",
                "Open the create-service editor",
            ),
            (
                "new.configmap",
                "New ConfigMap…",
                "configmaps",
                "Open the create-configmap editor",
            ),
            (
                "new.secret",
                "New Secret…",
                "secrets",
                "Open the create-secret editor",
            ),
        ] {
            commands.push(Command {
                id,
                label,
                section: "Action",
                description: description.to_string(),
                action: CommandAction::NewResource(kind),
            });
        }
    }

    if actions.cycle_theme {
        commands.push(Command {
            id: "settings.theme",
            label: "Cycle Theme",
            section: "Settings",
            description: "Switch to the next opaline theme".to_string(),
            action: CommandAction::CycleTheme,
        });
    }

    commands
}

/// Index just after the `Go to Home` entry, or the list head when the
/// navigation commands were not registered.
fn after_view_home(commands: &[Command]) -> usize {
    commands
        .iter()
        .position(|command| command.id == "view.home")
        .map(|index| index + 1)
        .unwrap_or(0)
}

/// The title-bar theme entries, in System → Light → Dark order.
fn title_bar_themes() -> [(&'static str, &'static str, &'static str, TitleBarTheme); 3] {
    [
        (
            "view.title-bar-theme-system",
            "Title Bar Theme: System",
            "Follow the OS decoration theme",
            TitleBarTheme::System,
        ),
        (
            "view.title-bar-theme-light",
            "Title Bar Theme: Light",
            "Force the light OS decoration theme",
            TitleBarTheme::Light,
        ),
        (
            "view.title-bar-theme-dark",
            "Title Bar Theme: Dark",
            "Force the dark OS decoration theme",
            TitleBarTheme::Dark,
        ),
    ]
}

/// Filter commands by a fuzzy query. A blank (or whitespace-only) query
/// returns all commands in registry order. Otherwise calls
/// [`crate::fuzzy::rank`] verbatim against `Command.label` (the v1 contract:
/// description is a tooltip, not a search target).
pub fn filter_commands(commands: &[Command], query: &str) -> Vec<Command> {
    if query.trim().is_empty() {
        return commands.to_vec();
    }
    rank(query, commands.iter().map(|c| (c.label, c.clone())))
        .into_iter()
        .map(|(_, c)| c)
        .collect()
}

/// Append the concrete theme the OS reports to the `System` entry's
/// description, so a user on "follow the OS" can see which decoration theme is
/// actually in effect. Pure — the caller supplies the read-back value.
pub fn annotate_system_with_effective(commands: &mut [Command], effective: Option<&str>) {
    let Some(effective) = effective else {
        return;
    };
    if let Some(command) = commands
        .iter_mut()
        .find(|command| command.action == CommandAction::SetTitleBarTheme(TitleBarTheme::System))
    {
        command.description = format!("Follow the OS decoration theme (OS reports {effective})");
    }
}

/// Advance the selection by `delta` with wrapping. `None` when the list is
/// empty; a stale out-of-range selection is clamped first.
pub fn advance_cursor(selected: Option<usize>, len: usize, delta: isize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let base = selected.unwrap_or(0).min(len - 1) as isize;
    Some((base + delta).rem_euclid(len as isize) as usize)
}

/// Whether the palette overlay is open.
pub static PALETTE_OPEN: GlobalSignal<bool> = Signal::global(|| false);

/// The current palette filter query (cleared on every open/close).
pub static PALETTE_QUERY: GlobalSignal<String> = Signal::global(String::new);

/// Close the palette and reset its transient state.
fn close_palette() {
    *PALETTE_OPEN.write() = false;
    *PALETTE_QUERY.write() = String::new();
}

/// Keybind listener source: installs once per webview. Cmd+P (mac) or Ctrl+P
/// (linux/win) toggles the palette (preventing the native default), Escape
/// closes it from anywhere — both flow back over the eval channel.
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

/// Webview-level Cmd+P / Escape keybind. Mounted once by the app shell; the
/// effect runs post-mount (DOM ready) and serves channel messages from a
/// spawned task for the life of the process.
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
///
/// `title_bar_effective` is the decoration theme the host reads back from the
/// OS, appended to the `System` entry's description when present.
#[component]
pub fn CommandPalette(
    #[props(default)] host: PaletteHost,
    #[props(default)] title_bar_effective: Option<String>,
) -> Element {
    let open = *PALETTE_OPEN.read();
    rsx! {
        if open {
            PalettePanel { host: host, title_bar_effective: title_bar_effective.clone() }
        }
    }
}

/// Dispatch a [`CommandAction`]. Closes the palette first, then runs the
/// action through the host hook that owns it; the cluster switcher is the
/// crate's own overlay, so the palette toggles it directly.
fn run_command(cmd: Command, host: &PaletteHost) {
    close_palette();
    match cmd.action {
        CommandAction::Navigate(path) => call(&host.navigate, path),
        CommandAction::SwitchCluster => *crate::components::switcher::SWITCHER_OPEN.write() = true,
        CommandAction::CycleTheme => call(&host.cycle_theme, ()),
        CommandAction::ToggleMenuBar => call(&host.toggle_menu_bar, ()),
        CommandAction::SetTitleBarTheme(theme) => call(&host.set_title_bar_theme, theme),
        CommandAction::NewResource(kind) => call(&host.new_resource, kind),
    }
}

/// Run `handler` with `value` when the host declared one.
fn call<T: 'static>(handler: &Option<EventHandler<T>>, value: T) {
    if let Some(handler) = handler {
        handler.call(value);
    }
}

/// Overlay panel: filter field + command list. Owns the selection cursor;
/// selecting (click or Enter) dispatches the [`CommandAction`].
#[component]
fn PalettePanel(
    host: PaletteHost,
    #[props(default)] title_bar_effective: Option<String>,
) -> Element {
    let mut all = commands(host.actions());
    annotate_system_with_effective(&mut all, title_bar_effective.as_deref());
    let query = PALETTE_QUERY.read().clone();
    let candidates = filter_commands(&all, &query);
    let mut selected = use_signal(|| 0usize);
    let cursor = (*selected.read()).min(candidates.len().saturating_sub(1));

    let rows: Vec<(usize, Command)> = candidates.iter().cloned().enumerate().collect();
    let mut grouped: Vec<(String, Vec<(usize, Command)>)> = Vec::new();
    for (idx, cmd) in rows {
        match grouped.last_mut() {
            Some((section, _)) if section.as_str() == cmd.section => {}
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
                        let keys_host = host;
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
                                    run_command(cmd.clone(), &keys_host);
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
                                    host: host,
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One row in the palette list. Click or Enter (handled at the panel level)
/// runs the command; the `title=` attribute surfaces the description as a v1
/// hover tooltip.
#[component]
fn PaletteRow(cmd: Command, is_selected: bool, host: PaletteHost) -> Element {
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
            onclick: move |_| run_command(cmd_for_click.clone(), &host),
            "{cmd.label}"
        }
    }
}
