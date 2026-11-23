//! The two dialogs that stand between a click and a destructive request.

use dioxus::prelude::*;

/// What the apiserver does with owned objects, phrased for the person choosing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Propagation {
    /// The owner goes first, its children follow.
    #[default]
    Background,
    /// The children go first, then the owner.
    Foreground,
    /// The children are left behind.
    Orphan,
}

impl Propagation {
    /// Every choice, in the order the dialog offers them.
    pub const ALL: [Propagation; 3] = [
        Propagation::Background,
        Propagation::Foreground,
        Propagation::Orphan,
    ];

    /// The value the radio group carries.
    pub fn id(self) -> &'static str {
        match self {
            Propagation::Background => "background",
            Propagation::Foreground => "foreground",
            Propagation::Orphan => "orphan",
        }
    }

    /// The label beside the radio.
    pub fn label(self) -> &'static str {
        match self {
            Propagation::Background => "Background",
            Propagation::Foreground => "Foreground",
            Propagation::Orphan => "Orphan",
        }
    }

    /// The sentence that says what happens to the owned objects.
    pub fn explanation(self) -> &'static str {
        match self {
            Propagation::Background => "delete the workload now; its pods are collected after",
            Propagation::Foreground => "delete the pods first, then the workload",
            Propagation::Orphan => "leave the pods running, with no owner",
        }
    }

    /// The same choice, from the value a radio carries.
    pub fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|choice| choice.id() == id)
            .unwrap_or_default()
    }
}

/// Whether the operator has typed the name they are about to delete.
pub fn confirmation_matches(name: &str, typed: &str) -> bool {
    !name.trim().is_empty() && typed.trim() == name
}

/// Why a drain leaves a pod alone, as the plan badges it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipKind {
    DaemonSet,
    MirrorPod,
    EmptyDirData,
    LocalStorage,
}

impl SkipKind {
    /// The short badge label.
    pub fn badge(self) -> &'static str {
        match self {
            Self::DaemonSet => "daemonset",
            Self::MirrorPod => "mirror pod",
            Self::EmptyDirData => "emptyDir",
            Self::LocalStorage => "local storage",
        }
    }

    /// The sentence the badge stands for.
    pub fn reason(self) -> &'static str {
        match self {
            Self::DaemonSet => "a DaemonSet runs one on every node",
            Self::MirrorPod => "the apiserver owns a mirror pod",
            Self::EmptyDirData => "evicting it would lose its emptyDir data",
            Self::LocalStorage => "it mounts local storage",
        }
    }
}

/// One pod in a drain preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainRow {
    /// `namespace/name`.
    pub key: String,
    /// Why it stays, when it stays.
    pub skip: Option<SkipKind>,
}

impl DrainRow {
    /// A pod the drain will evict.
    pub fn evicting(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            skip: None,
        }
    }

    /// A pod the drain will leave alone, and why.
    pub fn skipping(key: impl Into<String>, kind: SkipKind) -> Self {
        Self {
            key: key.into(),
            skip: Some(kind),
        }
    }
}

/// The typed-name dialog for deleting one workload.
#[derive(Props, Clone, PartialEq)]
pub struct DeleteDialogProps {
    /// Whether the dialog is on screen.
    pub open: bool,
    /// The kind being deleted, as the header names it.
    pub kind: String,
    /// The object's name, which the operator must retype.
    pub name: String,
    /// What the operator has typed so far.
    pub typed: String,
    /// The propagation choice currently selected.
    pub propagation: Propagation,
    /// The operator typed another character.
    pub on_typed: EventHandler<String>,
    /// The operator picked another propagation policy.
    pub on_propagation: EventHandler<Propagation>,
    /// The confirmation is enabled and was pressed.
    pub on_confirm: EventHandler<()>,
    /// The dialog should close.
    pub on_cancel: EventHandler<()>,
}

/// Asks for the object's name before it is deleted.
#[allow(non_snake_case)]
pub fn DeleteDialog(props: DeleteDialogProps) -> Element {
    if !props.open {
        return rsx! {};
    }
    let confirmed = confirmation_matches(&props.name, &props.typed);
    let name = props.name.clone();
    let typed = props.typed.clone();
    rsx! {
        div {
            class: "dlg-scrim",
            "data-dialog": "delete",
            onclick: move |_| props.on_cancel.call(()),
            section {
                class: "dlg wide",
                role: "dialog",
                "aria-modal": "true",
                "aria-label": "Delete {props.kind} {props.name}",
                onclick: move |event| event.stop_propagation(),
                h3 { "Delete {props.kind} {props.name}?" }
                div { class: "dlg-body",
                    p { "The workload is removed from the cluster. This cannot be undone." }
                    label { class: "field", r#for: "confirm-name",
                        "Type {props.name} to confirm"
                        input {
                            id: "confirm-name",
                            r#type: "text",
                            autocomplete: "off",
                            spellcheck: "false",
                            value: "{typed}",
                            "data-confirm-input": "name",
                            oninput: move |event| props.on_typed.call(event.value()),
                        }
                    }
                    fieldset { class: "act-policy",
                        legend { "What happens to the pods" }
                        for choice in Propagation::ALL {
                            label { class: "act-choice",
                                input {
                                    r#type: "radio",
                                    name: "propagation",
                                    value: choice.id(),
                                    checked: props.propagation == choice,
                                    onchange: move |_| props.on_propagation.call(choice),
                                }
                                span { class: "act-choice-label", "{choice.label()}" }
                                span { class: "act-choice-hint", "{choice.explanation()}" }
                            }
                        }
                    }
                }
                div { class: "dlg-actions",
                    button {
                        class: "btn ghost",
                        onclick: move |_| props.on_cancel.call(()),
                        "Cancel"
                    }
                    button {
                        class: "btn btn-danger",
                        "data-action": "confirm-delete",
                        disabled: !confirmed,
                        onclick: move |_| props.on_confirm.call(()),
                        "Delete {name}"
                    }
                }
            }
        }
    }
}

/// The options a drain exposes, matching `kubectl drain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrainToggles {
    /// Leave DaemonSet pods alone.
    pub ignore_daemonsets: bool,
    /// Evict pods whose `emptyDir` data would be lost.
    pub delete_empty_dir_data: bool,
    /// Evict pods that mount local storage.
    pub force: bool,
}

impl Default for DrainToggles {
    fn default() -> Self {
        Self {
            ignore_daemonsets: true,
            delete_empty_dir_data: false,
            force: false,
        }
    }
}

/// The dialog that shows what a drain will do before it does it.
#[derive(Props, Clone, PartialEq)]
pub struct DrainDialogProps {
    /// Whether the dialog is on screen.
    pub open: bool,
    /// The node being drained.
    pub node: String,
    /// Every pod on the node, with the drain's decision.
    pub rows: Vec<DrainRow>,
    /// The options currently set.
    pub toggles: DrainToggles,
    /// One of the checkboxes changed.
    pub on_toggles: EventHandler<DrainToggles>,
    /// The drain was confirmed.
    pub on_confirm: EventHandler<()>,
    /// The dialog should close.
    pub on_cancel: EventHandler<()>,
}

/// Shows the plan, then drains.
#[allow(non_snake_case)]
pub fn DrainDialog(props: DrainDialogProps) -> Element {
    if !props.open {
        return rsx! {};
    }
    let toggles = props.toggles;
    let evicting = props.rows.iter().filter(|row| row.skip.is_none()).count();
    let staying = props.rows.len() - evicting;
    rsx! {
        div {
            class: "dlg-scrim",
            "data-dialog": "drain",
            onclick: move |_| props.on_cancel.call(()),
            section {
                class: "dlg wide",
                role: "dialog",
                "aria-modal": "true",
                "aria-label": "Drain {props.node}",
                onclick: move |event| event.stop_propagation(),
                h3 { "Drain {props.node}?" }
                div { class: "dlg-body",
                    p { "The node is marked unschedulable, then {evicting} pod(s) are evicted. {staying} stay." }
                    div { class: "act-policy",
                        label { class: "act-choice",
                            input {
                                r#type: "checkbox",
                                name: "ignore-daemonsets",
                                checked: toggles.ignore_daemonsets,
                                onchange: move |_| props.on_toggles.call(DrainToggles {
                                    ignore_daemonsets: !toggles.ignore_daemonsets,
                                    ..toggles
                                }),
                            }
                            span { class: "act-choice-label", "Ignore DaemonSets" }
                            span { class: "act-choice-hint", "a DaemonSet pod belongs on every node" }
                        }
                        label { class: "act-choice",
                            input {
                                r#type: "checkbox",
                                name: "delete-emptydir-data",
                                checked: toggles.delete_empty_dir_data,
                                onchange: move |_| props.on_toggles.call(DrainToggles {
                                    delete_empty_dir_data: !toggles.delete_empty_dir_data,
                                    ..toggles
                                }),
                            }
                            span { class: "act-choice-label", "Delete emptyDir data" }
                            span { class: "act-choice-hint", "the data in those volumes is lost" }
                        }
                        label { class: "act-choice",
                            input {
                                r#type: "checkbox",
                                name: "force",
                                checked: toggles.force,
                                onchange: move |_| props.on_toggles.call(DrainToggles {
                                    force: !toggles.force,
                                    ..toggles
                                }),
                            }
                            span { class: "act-choice-label", "Force" }
                            span { class: "act-choice-hint", "evict pods that mount local storage" }
                        }
                    }
                    ul { class: "act-plan",
                        for row in props.rows.iter() {
                            li {
                                class: if row.skip.is_some() { "act-row skip" } else { "act-row" },
                                span { class: "act-key", "{row.key}" }
                                match row.skip {
                                    Some(kind) => rsx! {
                                        div { class: "act-badges",
                                            span {
                                                class: "pill warn",
                                                title: "the drain leaves it running",
                                                "protected"
                                            }
                                            span {
                                                class: "pill neutral",
                                                title: "{kind.reason()}",
                                                "{kind.badge()}"
                                            }
                                        }
                                    },
                                    None => rsx! {
                                        div { class: "act-badges",
                                            span { class: "pill danger", "evicted" }
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
                div { class: "dlg-actions",
                    button {
                        class: "btn ghost",
                        onclick: move |_| props.on_cancel.call(()),
                        "Cancel"
                    }
                    button {
                        class: "btn btn-danger",
                        "data-action": "confirm-drain",
                        disabled: evicting == 0,
                        onclick: move |_| props.on_confirm.call(()),
                        "Drain {props.node}"
                    }
                }
            }
        }
    }
}
