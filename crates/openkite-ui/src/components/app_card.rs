//! Application card: the reference's `.app-card` with its kebab- and
//! swipe-revealed action column.

use dioxus::prelude::*;

/// The distance `.card-main` travels when swiped — the action column's width.
pub const SWIPE_REVEAL_PX: f64 = 112.0;

/// A drag must clear this horizontally to count as a swipe (the 44px floor).
pub const SWIPE_THRESHOLD_PX: f64 = 44.0;

/// The brand-spec floor every interactive element in a card measures.
pub const TAP_TARGET_PX: u32 = 44;

/// The `.card-status` edge tone and the pill the footer shows for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CardTone {
    #[default]
    Synced,
    OutOfSync,
    Degraded,
    Progressing,
}

impl CardTone {
    /// The `.card-status` modifier the tone paints.
    pub fn slug(self) -> &'static str {
        match self {
            CardTone::Synced => "synced",
            CardTone::OutOfSync => "outofsync",
            CardTone::Degraded => "degraded",
            CardTone::Progressing => "progressing",
        }
    }

    /// The design-system pill variant for the tone.
    pub fn pill(self) -> &'static str {
        match self {
            CardTone::Synced => "success",
            CardTone::OutOfSync => "warn",
            CardTone::Degraded => "danger",
            CardTone::Progressing => "warn",
        }
    }

    /// The tone's human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            CardTone::Synced => "Synced",
            CardTone::OutOfSync => "OutOfSync",
            CardTone::Degraded => "Degraded",
            CardTone::Progressing => "Progressing",
        }
    }
}

/// One action a card offers, shared by the kebab and the swipe column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardAction {
    pub id: String,
    pub label: String,
    /// Paint the action in the success tone (the reference's `.sync`).
    pub sync: bool,
}

impl CardAction {
    /// A plain action.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            sync: false,
        }
    }

    /// A success-toned action (the reference's `.sync`).
    pub fn sync(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            sync: true,
        }
    }
}

/// What a horizontal pointer drag resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeIntent {
    /// Swipe left: reveal the action column.
    Reveal,
    /// Swipe right: close it.
    Dismiss,
    /// Too short, or vertical: leave the card alone.
    None,
}

/// Resolve a drag from its deltas; horizontal dominance and the 44px floor are
/// both required.
pub fn resolve_swipe(dx: f64, dy: f64) -> SwipeIntent {
    if dx.abs() < SWIPE_THRESHOLD_PX || dx.abs() <= dy.abs() {
        SwipeIntent::None
    } else if dx < 0.0 {
        SwipeIntent::Reveal
    } else {
        SwipeIntent::Dismiss
    }
}

/// What a tap on the card body does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapIntent {
    /// Nothing is open and no swipe just ended: the card may navigate.
    Activate,
    /// The card is open: a tap only closes it.
    Dismiss,
    /// A swipe just ended: swallow the tap the browser fires after it.
    Swallow,
}

/// Resolve a tap against the card's gesture state; only the settled, closed
/// card may navigate.
pub fn resolve_tap(swiped: bool, just_swiped: bool) -> TapIntent {
    if just_swiped {
        TapIntent::Swallow
    } else if swiped {
        TapIntent::Dismiss
    } else {
        TapIntent::Activate
    }
}

/// One application card: its status stripe, kebab and action column.
#[component]
pub fn AppCard(
    name: String,
    #[props(default)] sub: String,
    #[props(default)] tone: CardTone,
    #[props(default)] repo: String,
    #[props(default)] version: String,
    #[props(default)] age: String,
    /// Every action the card offers, in the order shown.
    #[props(default)]
    actions: Vec<CardAction>,
    /// What the host does with an action the card advertises.
    #[props(default)]
    on_action: Option<EventHandler<String>>,
    /// What the host does when the card itself is activated (navigation).
    #[props(default)]
    on_open: Option<EventHandler<()>>,
) -> Element {
    let mut swiped = use_signal(|| false);
    let mut just_swiped = use_signal(|| false);
    let mut start = use_signal(|| (0.0_f64, 0.0_f64));
    let mut tracking = use_signal(|| false);
    let open = swiped();
    let actions_id = format!("card-actions-{}", name.replace(' ', "-"));

    let on_pointerdown = move |evt: Event<PointerData>| {
        let point = evt.data().client_coordinates();
        start.set((point.x, point.y));
        tracking.set(true);
        just_swiped.set(false);
    };
    let on_pointerup = move |evt: Event<PointerData>| {
        if !tracking() {
            return;
        }
        tracking.set(false);
        let point = evt.data().client_coordinates();
        let origin = start();
        match resolve_swipe(point.x - origin.0, point.y - origin.1) {
            SwipeIntent::Reveal => {
                swiped.set(true);
                just_swiped.set(true);
            }
            SwipeIntent::Dismiss => {
                swiped.set(false);
                just_swiped.set(true);
            }
            SwipeIntent::None => {}
        }
    };
    let on_pointercancel = move |_| tracking.set(false);
    let on_card_tap = move |_evt: Event<MouseData>| match resolve_tap(swiped(), just_swiped()) {
        TapIntent::Activate => {
            if let Some(handler) = on_open {
                handler.call(());
            }
        }
        TapIntent::Dismiss => swiped.set(false),
        TapIntent::Swallow => just_swiped.set(false),
    };
    let toggle_actions = move |evt: Event<MouseData>| {
        evt.stop_propagation();
        let next = !swiped();
        swiped.set(next);
        just_swiped.set(false);
    };

    rsx! {
        article {
            class: if open { "app-card swiped" } else { "app-card" },
            "data-swiped": if open { "true" } else { "false" },
            div {
                class: "card-swipe-actions",
                id: "{actions_id}",
                role: "group",
                aria_label: "Card actions",
                for action in actions.iter().cloned() {
                    button {
                        class: if action.sync { "card-action sync" } else { "card-action" },
                        r#type: "button",
                        "data-action": "{action.id}",
                        onclick: move |evt: Event<MouseData>| {
                            evt.stop_propagation();
                            if let Some(handler) = on_action {
                                handler.call(action.id.clone());
                            }
                        },
                        "{action.label}"
                    }
                }
            }
            div {
                class: "card-main",
                onpointerdown: on_pointerdown,
                onpointerup: on_pointerup,
                onpointercancel: on_pointercancel,
                onclick: on_card_tap,
                div { class: "card-status {tone.slug()}" }
                div { class: "card-body",
                    button {
                        class: "card-menu-btn",
                        r#type: "button",
                        aria_label: "Open {name} actions",
                        "aria-haspopup": "true",
                        "aria-expanded": if open { "true" } else { "false" },
                        "aria-controls": "{actions_id}",
                        onclick: toggle_actions,
                        svg {
                            class: "icon",
                            "viewBox": "0 0 24 24",
                            "aria-hidden": "true",
                            circle { cx: "12", cy: "5", r: "1.7" }
                            circle { cx: "12", cy: "12", r: "1.7" }
                            circle { cx: "12", cy: "19", r: "1.7" }
                        }
                    }
                    div { class: "card-title-row",
                        span { class: "source-icon",
                            svg {
                                class: "icon",
                                "viewBox": "0 0 24 24",
                                "aria-hidden": "true",
                                path { d: "M4 7h16M4 12h16M4 17h10" }
                            }
                        }
                        div {
                            h2 { class: "app-name", "{name}" }
                            if !sub.is_empty() {
                                p { class: "app-sub", "{sub}" }
                            }
                        }
                    }
                    if !repo.is_empty() || !version.is_empty() {
                        div { class: "tag-row",
                            if !repo.is_empty() {
                                span { class: "tag", "{repo}" }
                            }
                            if !version.is_empty() {
                                span { class: "tag", "{version}" }
                            }
                        }
                    }
                    div { class: "card-footer",
                        div { class: "badges",
                            span { class: "pill {tone.pill()}", "{tone.label()}" }
                        }
                        if !age.is_empty() {
                            span { class: "card-meta", "{age}" }
                        }
                    }
                }
            }
        }
    }
}
