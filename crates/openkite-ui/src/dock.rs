//! The bottom dock's tab model: one pane of tabs, or two after a split.
//!
//! Pure logic — no Dioxus, no cluster. The `DockView` component in
//! `components/dock.rs` renders whatever this module decides; the toolbar
//! buttons, the tab strips and the pane bodies all go through here.
//!
//! # Panes
//!
//! The dock opens as **one pane**: [`DockState::tabs`] holds its tabs and
//! [`DockState::active`] names the focused one. That is the single-pane dock
//! as it always was, and a default [`DockState`] is still exactly that.
//!
//! [`split`] adds a *second* pane ([`SplitDock::pane`]) with its own tab list
//! and its own active tab; [`DockState::split`] is `None` until it does, so
//! "split" is structural rather than a flag that can drift from the storage.
//! Every rule that used to pick a tab now picks a tab *and its pane*, so a
//! selection while split targets the focused pane instead of guessing.
//!
//! # Swap is layout, not state
//!
//! [`swap`] exchanges only [`SplitDock::order`] — the visual left/right order.
//! No tab, active tab or session moves: a terminal tab stays attached to its
//! own pane (and so to its own session), and a log tab keeps the follow
//! position that belongs to it. The view keys each pane by its slot for the
//! same reason, so reordering preserves the mounted terminal.
//!
//! # Narrow viewports
//!
//! [`visible_panes`] is the pure rule behind the `<=767px` collapse: the dock
//! shows the focused pane only and restores the split when the viewport
//! widens. The stylesheet enforces the same rule without unmounting a pane, so
//! the hidden terminal survives the collapse.

use openkite_api::pod::PodObject;

/// Dock height in px when nothing has been dragged.
pub const DEFAULT_HEIGHT: u32 = 264;
/// The dock never gets shorter than this.
pub const MIN_HEIGHT: u32 = 120;
/// The dock never gets taller than this.
pub const MAX_HEIGHT: u32 = 720;
/// Below this viewport width the dock collapses to the focused pane alone.
///
/// `<= 767px` shows one pane; `>= 768px` shows the split. The stylesheet's
/// `@media (max-width: 767px)` rule carries the same boundary.
pub const SPLIT_MIN_WIDTH: f64 = 768.0;

/// Which pane of the dock a tab belongs to.
///
/// A slot is the pane's identity, not its position: [`swap`] reorders the
/// slots and leaves their contents alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSlot {
    /// The pane the dock opens with. Present in every layout.
    A,
    /// The pane a split adds. Absent until [`split`] runs.
    B,
}

impl PaneSlot {
    /// The `data-pane` value the view stamps on the pane.
    pub fn key(self) -> &'static str {
        match self {
            PaneSlot::A => "a",
            PaneSlot::B => "b",
        }
    }
}

/// A pane's tabs and the tab it has focused.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DockPane {
    pub tabs: Vec<DockTab>,
    pub active: Option<String>,
}

/// The second pane, plus the layout facts that only exist while split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitDock {
    /// The pane slot `B`'s tabs and focus.
    pub pane: DockPane,
    /// Left-to-right render order. [`swap`] reverses it.
    pub order: [PaneSlot; 2],
    /// The pane the toolbar, selections and closes act on.
    pub focused: PaneSlot,
}

impl SplitDock {
    fn new() -> Self {
        Self {
            pane: DockPane::default(),
            order: [PaneSlot::A, PaneSlot::B],
            focused: PaneSlot::A,
        }
    }
}

/// What kind of surface a dock tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockTabKind {
    Terminal,
    Logs,
}

impl DockTabKind {
    /// The noun used when a tab has no pod (or names the kind).
    pub fn label(self) -> &'static str {
        match self {
            DockTabKind::Terminal => "Terminal",
            DockTabKind::Logs => "Logs",
        }
    }
}

/// One tab in the dock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockTab {
    pub id: String,
    pub kind: DockTabKind,
    pub pod: Option<PodObject>,
    /// The selection tab: retargeted by the next pod pick instead of piling up.
    pub temporary: bool,
}

impl DockTab {
    /// The pod name, falling back to the kind for a tab with no pod.
    pub fn label(&self) -> String {
        self.pod
            .as_ref()
            .map(|pod| pod.name.clone())
            .unwrap_or_else(|| self.kind.label().to_string())
    }
}

/// The whole dock: pane `A`'s tabs inline, pane `B` only while split.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DockState {
    /// Pane `A`'s tabs. Every tab of the single-pane dock lives here.
    pub tabs: Vec<DockTab>,
    /// Pane `A`'s focused tab.
    pub active: Option<String>,
    /// The second pane and its order, or `None` while single-pane.
    pub split: Option<SplitDock>,
    next_id: u32,
}

/// One pane as the view renders it: tabs, focus, and whether it is the
/// focused one.
#[derive(Debug, Clone, Copy)]
pub struct PaneView<'a> {
    pub slot: PaneSlot,
    pub tabs: &'a [DockTab],
    pub active: Option<&'a str>,
    pub focused: bool,
}

impl DockState {
    /// Whether the dock is split in two.
    pub fn is_split(&self) -> bool {
        self.split.is_some()
    }

    /// Whether any pane holds a tab. An empty dock renders nothing.
    pub fn is_open(&self) -> bool {
        !self.tabs.is_empty() || self.split.as_ref().is_some_and(|s| !s.pane.tabs.is_empty())
    }

    /// The pane the toolbar and new tabs act on.
    pub fn focused(&self) -> PaneSlot {
        self.split.as_ref().map_or(PaneSlot::A, |s| s.focused)
    }

    /// The tab `id`, in whichever pane holds it.
    pub fn tab(&self, id: &str) -> Option<&DockTab> {
        self.tabs.iter().find(|tab| tab.id == id).or_else(|| {
            self.split
                .as_ref()?
                .pane
                .tabs
                .iter()
                .find(|tab| tab.id == id)
        })
    }

    /// Which pane holds tab `id`, and its index there.
    pub fn tab_slot(&self, id: &str) -> Option<(PaneSlot, usize)> {
        if let Some(index) = self.tabs.iter().position(|tab| tab.id == id) {
            return Some((PaneSlot::A, index));
        }
        let index = self
            .split
            .as_ref()?
            .pane
            .tabs
            .iter()
            .position(|tab| tab.id == id)?;
        Some((PaneSlot::B, index))
    }

    /// Pane `slot`'s tabs. Always empty when `slot` is `B` and the dock is not
    /// split.
    pub fn pane_tabs(&self, slot: PaneSlot) -> &[DockTab] {
        match slot {
            PaneSlot::A => &self.tabs,
            PaneSlot::B => self
                .split
                .as_ref()
                .map_or(&[][..], |s| s.pane.tabs.as_slice()),
        }
    }

    /// Pane `slot`'s focused tab.
    pub fn pane_active(&self, slot: PaneSlot) -> Option<&str> {
        match slot {
            PaneSlot::A => self.active.as_deref(),
            PaneSlot::B => self.split.as_ref()?.pane.active.as_deref(),
        }
    }

    /// The panes in render order: one, or two while split. [`swap`] changes
    /// the order, never the contents.
    pub fn panes(&self) -> Vec<PaneView<'_>> {
        let Some(split) = self.split.as_ref() else {
            return vec![PaneView {
                slot: PaneSlot::A,
                tabs: &self.tabs,
                active: self.active.as_deref(),
                focused: true,
            }];
        };
        split
            .order
            .iter()
            .map(|&slot| PaneView {
                slot,
                tabs: self.pane_tabs(slot),
                active: self.pane_active(slot),
                focused: slot == split.focused,
            })
            .collect()
    }

    /// Every tab, both panes, in render order.
    pub fn all_tabs(&self) -> Vec<&DockTab> {
        self.panes()
            .into_iter()
            .flat_map(|pane| pane.tabs)
            .collect()
    }

    /// The panes a viewport of `viewport_width` px can show.
    ///
    /// One pane when the dock is single-pane, the focused pane alone below
    /// [`SPLIT_MIN_WIDTH`], otherwise both in [`SplitDock::order`].
    pub fn visible_panes(&self, viewport_width: f64) -> Vec<PaneSlot> {
        if self.is_split() && viewport_width < SPLIT_MIN_WIDTH {
            return vec![self.focused()];
        }
        self.panes().into_iter().map(|pane| pane.slot).collect()
    }

    /// Pane `slot`'s tabs, for mutation. `None` for slot `B` while single-pane.
    fn pane_tabs_mut(&mut self, slot: PaneSlot) -> Option<&mut Vec<DockTab>> {
        match slot {
            PaneSlot::A => Some(&mut self.tabs),
            PaneSlot::B => self.split.as_mut().map(|s| &mut s.pane.tabs),
        }
    }

    fn tab_mut(&mut self, id: &str) -> Option<&mut DockTab> {
        if let Some(index) = self.tabs.iter().position(|tab| tab.id == id) {
            return self.tabs.get_mut(index);
        }
        let split = self.split.as_mut()?;
        let index = split.pane.tabs.iter().position(|tab| tab.id == id)?;
        split.pane.tabs.get_mut(index)
    }

    fn set_active(&mut self, slot: PaneSlot, id: Option<String>) {
        match slot {
            PaneSlot::A => self.active = id,
            PaneSlot::B => {
                if let Some(split) = self.split.as_mut() {
                    split.pane.active = id;
                }
            }
        }
        self.focus_pane(slot);
    }

    /// Move the toolbar's target to `slot`. A single-pane dock has one pane,
    /// so this is a no-op there.
    pub fn focus_pane(&mut self, slot: PaneSlot) {
        if let Some(split) = self.split.as_mut() {
            split.focused = slot;
        }
    }

    fn find_tab(&self, pred: impl Fn(&DockTab) -> bool) -> Option<(PaneSlot, usize)> {
        if let Some(index) = self.tabs.iter().position(&pred) {
            return Some((PaneSlot::A, index));
        }
        let index = self.split.as_ref()?.pane.tabs.iter().position(pred)?;
        Some((PaneSlot::B, index))
    }

    fn alloc_id(&mut self) -> String {
        self.next_id += 1;
        format!("tab-{}", self.next_id)
    }
}

// --- height ----------------------------------------------------------------

/// Clamp a dragged height into the dock's band. A non-finite drag keeps the
/// default.
pub fn clamp_height(px: f64) -> u32 {
    if !px.is_finite() {
        return DEFAULT_HEIGHT;
    }
    px.round().clamp(MIN_HEIGHT as f64, MAX_HEIGHT as f64) as u32
}

/// A resize drag: `current` plus `delta`, clamped.
pub fn resize_by(current: u32, delta: f64) -> u32 {
    clamp_height(current as f64 + delta)
}

// --- layout ----------------------------------------------------------------

/// Split the dock into two panes.
///
/// Pane `A` keeps every tab it had, focus included; pane `B` starts empty and
/// fills from the focused pane's next selection. Splitting an already split
/// dock does nothing.
pub fn split(state: &mut DockState) {
    if state.split.is_none() {
        state.split = Some(SplitDock::new());
    }
}

/// Exchange the two panes' positions.
///
/// Layout only: no tab, active tab or session moves between panes, so each
/// pane's state stays attached to its slot.
pub fn swap(state: &mut DockState) {
    if let Some(split) = state.split.as_mut() {
        split.order.swap(0, 1);
    }
}

/// Back to one pane, keeping the focused pane's tabs and active tab.
///
/// The other pane's tabs are carried over rather than dropped — a terminal
/// whose shell is running must not die on a layout change — with at most one
/// temporary tab in the result (the focused pane's wins; an adopted temporary
/// tab only fills the gap when the focused pane has none).
pub fn unsplit(state: &mut DockState) {
    let Some(split) = state.split.take() else {
        return;
    };
    let (kept, adopted) = match split.focused {
        PaneSlot::A => (
            DockPane {
                tabs: std::mem::take(&mut state.tabs),
                active: state.active.take(),
            },
            split.pane,
        ),
        PaneSlot::B => (
            split.pane,
            DockPane {
                tabs: std::mem::take(&mut state.tabs),
                active: state.active.take(),
            },
        ),
    };
    let mut temporary_taken = kept.tabs.iter().any(|tab| tab.temporary);
    let mut tabs = kept.tabs;
    for tab in adopted.tabs {
        if tab.temporary {
            if temporary_taken {
                continue;
            }
            temporary_taken = true;
        }
        tabs.push(tab);
    }
    let active = kept
        .active
        .filter(|id| tabs.iter().any(|tab| &tab.id == id))
        .or_else(|| tabs.first().map(|tab| tab.id.clone()));
    state.tabs = tabs;
    state.active = active;
}

// --- tabs ------------------------------------------------------------------

/// Open (or focus) the log tab for `pod` in the focused pane.
///
/// Reuses that pane's temporary tab, so however many pods are picked a pane
/// holds at most one temporary tab.
pub fn open_pod(state: &mut DockState, pod: PodObject) {
    if let Some((slot, index)) = state.find_tab(|tab| tab.pod.as_ref() == Some(&pod)) {
        let id = state.pane_tabs(slot)[index].id.clone();
        state.set_active(slot, Some(id));
        return;
    }
    let slot = state.focused();
    let temporary = state
        .pane_tabs_mut(slot)
        .and_then(|tabs| tabs.iter().position(|tab| tab.temporary));
    match temporary {
        Some(index) => {
            let tabs = state.pane_tabs_mut(slot).expect("the focused pane exists");
            tabs[index].kind = DockTabKind::Logs;
            tabs[index].pod = Some(pod);
            let id = tabs[index].id.clone();
            state.set_active(slot, Some(id));
        }
        None => {
            let id = state.alloc_id();
            let tabs = state.pane_tabs_mut(slot).expect("the focused pane exists");
            tabs.push(DockTab {
                id: id.clone(),
                kind: DockTabKind::Logs,
                pod: Some(pod),
                temporary: true,
            });
            state.set_active(slot, Some(id));
        }
    }
}

/// Open (or focus) a permanent terminal tab for `pod` in the focused pane.
pub fn open_terminal(state: &mut DockState, pod: PodObject) {
    if let Some((slot, index)) =
        state.find_tab(|tab| tab.kind == DockTabKind::Terminal && tab.pod.as_ref() == Some(&pod))
    {
        let id = state.pane_tabs(slot)[index].id.clone();
        state.set_active(slot, Some(id));
        return;
    }
    let slot = state.focused();
    let id = state.alloc_id();
    let tabs = state.pane_tabs_mut(slot).expect("the focused pane exists");
    tabs.push(DockTab {
        id: id.clone(),
        kind: DockTabKind::Terminal,
        pod: Some(pod),
        temporary: false,
    });
    state.set_active(slot, Some(id));
}

/// Focus tab `id` and the pane that holds it.
pub fn activate(state: &mut DockState, id: &str) {
    let Some((slot, _)) = state.tab_slot(id) else {
        return;
    };
    state.set_active(slot, Some(id.to_string()));
}

/// Promote tab `id` from temporary to permanent.
pub fn promote(state: &mut DockState, id: &str) {
    if let Some(tab) = state.tab_mut(id) {
        tab.temporary = false;
    }
}

/// Close tab `id`, focusing a neighbour in the same pane.
pub fn close(state: &mut DockState, id: &str) {
    let Some((slot, index)) = state.tab_slot(id) else {
        return;
    };
    let (closed, neighbour) = {
        let tabs = state.pane_tabs_mut(slot).expect("the owning pane exists");
        tabs.remove(index);
        let neighbour = tabs
            .get(index)
            .or_else(|| tabs.last())
            .map(|tab| tab.id.clone());
        (tabs.is_empty(), neighbour)
    };
    if state.pane_active(slot) == Some(id) || closed {
        let active = if closed { None } else { neighbour };
        state.set_active(slot, active);
    }
}

/// Close every tab but `id`, in both panes. The other pane keeps its slot and
/// goes empty.
pub fn close_others(state: &mut DockState, id: &str) {
    let Some((slot, _)) = state.tab_slot(id) else {
        return;
    };
    state.tabs.retain(|tab| tab.id == id);
    if let Some(split) = state.split.as_mut() {
        split.pane.tabs.retain(|tab| tab.id == id);
    }
    match slot {
        PaneSlot::A => {
            if let Some(split) = state.split.as_mut() {
                split.pane.active = None;
            }
        }
        PaneSlot::B => state.active = None,
    }
    state.set_active(slot, Some(id.to_string()));
}

/// Close every tab. The dock collapses back to single-pane: an empty dock has
/// no layout worth keeping.
pub fn close_all(state: &mut DockState) {
    state.tabs.clear();
    state.active = None;
    state.split = None;
}
