//! The bottom dock's tab model: the single reusable temporary tab a pod
//! selection owns, the permanent tabs beside it, and the dock height.
//!
//! Pure logic — no Dioxus, no cluster. The `DockView` component in
//! [`crate::components::dock`] renders what these rules produce.

use openkite_api::pod::PodObject;

/// Height the dock opens at, in CSS pixels.
pub const DEFAULT_HEIGHT: u32 = 264;

/// Shortest the dock may be dragged.
pub const MIN_HEIGHT: u32 = 120;

/// Tallest the dock may be dragged.
pub const MAX_HEIGHT: u32 = 720;

/// What a dock tab shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockTabKind {
    Terminal,
    Logs,
}

impl DockTabKind {
    /// The tab's kind label.
    pub fn label(self) -> &'static str {
        match self {
            DockTabKind::Terminal => "Terminal",
            DockTabKind::Logs => "Logs",
        }
    }
}

/// One dock tab. `temporary` marks the single reusable tab a pod selection
/// owns; a terminal tab is never temporary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockTab {
    pub id: String,
    pub kind: DockTabKind,
    pub pod: Option<PodObject>,
    pub temporary: bool,
}

impl DockTab {
    /// The tab's caption: the pod name, else the kind label.
    pub fn label(&self) -> String {
        match &self.pod {
            Some(pod) => pod.name.clone(),
            None => self.kind.label().to_string(),
        }
    }
}

/// The dock's tabs, its active tab and the id counter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockState {
    pub tabs: Vec<DockTab>,
    pub active: Option<String>,
    pub next_id: u32,
}

impl Default for DockState {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            next_id: 1,
        }
    }
}

impl DockState {
    /// Whether the dock has any tab to show. An empty dock renders nothing.
    pub fn is_open(&self) -> bool {
        !self.tabs.is_empty()
    }

    /// The tab `id` addresses.
    pub fn tab(&self, id: &str) -> Option<&DockTab> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    fn alloc_id(&mut self) -> String {
        let id = format!("dock-{}", self.next_id);
        self.next_id += 1;
        id
    }

    fn focus(&mut self, index: usize) {
        self.active = Some(self.tabs[index].id.clone());
    }
}

/// Open the pod a selection names, reusing the dock's one temporary tab.
///
/// A pod that already has an open tab is focused instead of duplicated; else
/// an existing temporary tab is replaced in place; else one temporary tab is
/// pushed. However many pods are picked, the dock holds at most one
/// temporary tab.
pub fn open_pod(state: &mut DockState, pod: PodObject) {
    if let Some(index) = state
        .tabs
        .iter()
        .position(|tab| tab.pod.as_ref() == Some(&pod))
    {
        state.focus(index);
        return;
    }
    if let Some(index) = state.tabs.iter().position(|tab| tab.temporary) {
        state.tabs[index].kind = DockTabKind::Logs;
        state.tabs[index].pod = Some(pod);
        state.focus(index);
        return;
    }
    let id = state.alloc_id();
    state.tabs.push(DockTab {
        id: id.clone(),
        kind: DockTabKind::Logs,
        pod: Some(pod),
        temporary: true,
    });
    state.active = Some(id);
}

/// Open a permanent terminal tab for `pod`, focusing it when one already
/// exists. A terminal is never temporary.
pub fn open_terminal(state: &mut DockState, pod: PodObject) {
    if let Some(index) = state.tabs.iter().position(|tab| {
        tab.kind == DockTabKind::Terminal && tab.pod.as_ref().map(|p| &p.name) == Some(&pod.name)
    }) {
        state.focus(index);
        return;
    }
    let id = state.alloc_id();
    state.tabs.push(DockTab {
        id: id.clone(),
        kind: DockTabKind::Terminal,
        pod: Some(pod),
        temporary: false,
    });
    state.active = Some(id);
}

/// Focus `id` when it is an open tab.
pub fn activate(state: &mut DockState, id: &str) {
    if state.tab(id).is_some() {
        state.active = Some(id.to_string());
    }
}

/// Promote a temporary tab to permanent so later selections cannot replace it.
pub fn promote(state: &mut DockState, id: &str) {
    if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == id) {
        tab.temporary = false;
    }
}

/// Close `id`, handing focus to the next tab. Closing the last tab empties
/// the dock and clears the active tab.
pub fn close(state: &mut DockState, id: &str) {
    let Some(index) = state.tabs.iter().position(|tab| tab.id == id) else {
        return;
    };
    state.tabs.remove(index);
    if state.active.as_deref() == Some(id) {
        state.active = state
            .tabs
            .get(index)
            .or_else(|| state.tabs.last())
            .map(|tab| tab.id.clone());
    }
}

/// Close every tab but `id`, which keeps focus. Unknown ids are ignored.
pub fn close_others(state: &mut DockState, id: &str) {
    if state.tab(id).is_none() {
        return;
    }
    state.tabs.retain(|tab| tab.id == id);
    state.active = Some(id.to_string());
}

/// Close every tab.
pub fn close_all(state: &mut DockState) {
    state.tabs.clear();
    state.active = None;
}

/// Clamp a requested dock height to the draggable range.
pub fn clamp_height(px: f64) -> u32 {
    if !px.is_finite() {
        return DEFAULT_HEIGHT;
    }
    (px.round() as i64).clamp(MIN_HEIGHT as i64, MAX_HEIGHT as i64) as u32
}

/// The height a drag yields when the pointer moves `delta_px` upward from
/// `start_height` (a positive delta grows the dock).
pub fn resize_by(start_height: u32, delta_px: f64) -> u32 {
    clamp_height(start_height as f64 + delta_px)
}
