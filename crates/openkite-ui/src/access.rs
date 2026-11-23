//! What the connected identity may do, as a view sees it.

use std::collections::BTreeMap;

use dioxus::prelude::*;

/// The verbs a screen asks about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verb {
    /// Read one object.
    Get,
    /// Read a collection.
    List,
    /// Create an object.
    Create,
    /// Modify an object.
    Update,
    /// Remove an object.
    Delete,
}

/// Per-verb answers, carrying the apiserver's reason on every denial.
///
/// The default is allow-all: preview and mock scenarios have no identity to ask
/// about, and the host replaces this with the real answers as soon as it has
/// them. A view never renders a control this map denies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Permissions {
    decisions: BTreeMap<Verb, (bool, Option<String>)>,
}

impl Default for Permissions {
    fn default() -> Self {
        Self::allow_all()
    }
}

impl Permissions {
    /// Every verb allowed.
    pub fn allow_all() -> Self {
        let mut decisions = BTreeMap::new();
        for verb in [
            Verb::Get,
            Verb::List,
            Verb::Create,
            Verb::Update,
            Verb::Delete,
        ] {
            decisions.insert(verb, (true, None));
        }
        Self { decisions }
    }

    /// Every verb denied with the same reason.
    pub fn deny_all(reason: &str) -> Self {
        let mut decisions = BTreeMap::new();
        for verb in [
            Verb::Get,
            Verb::List,
            Verb::Create,
            Verb::Update,
            Verb::Delete,
        ] {
            decisions.insert(verb, (false, Some(reason.to_owned())));
        }
        Self { decisions }
    }

    /// Records one answer.
    pub fn set(&mut self, verb: Verb, allowed: bool, reason: Option<&str>) {
        self.decisions
            .insert(verb, (allowed, reason.map(str::to_owned)));
    }

    /// Whether the verb is allowed; a verb with no answer is denied.
    pub fn allows(&self, verb: Verb) -> bool {
        matches!(self.decisions.get(&verb), Some((true, _)))
    }

    /// Whether one object can be read.
    pub fn can_get(&self) -> bool {
        self.allows(Verb::Get)
    }

    /// Whether the collection can be read.
    pub fn can_list(&self) -> bool {
        self.allows(Verb::List)
    }

    /// Whether an object can be created.
    pub fn can_create(&self) -> bool {
        self.allows(Verb::Create)
    }

    /// Whether an object can be modified.
    pub fn can_update(&self) -> bool {
        self.allows(Verb::Update)
    }

    /// Whether an object can be removed.
    pub fn can_delete(&self) -> bool {
        self.allows(Verb::Delete)
    }

    /// Whether any write verb is allowed.
    pub fn can_write(&self) -> bool {
        self.can_create() || self.can_update() || self.can_delete()
    }

    /// Why reading the collection was denied, when the apiserver said why.
    pub fn list_denial(&self) -> Option<&str> {
        match self.decisions.get(&Verb::List) {
            Some((false, reason)) => reason.as_deref(),
            _ => None,
        }
    }
}

/// The verb an inspector action needs.
pub fn verb_for_action(id: &str) -> Verb {
    match id {
        "delete" => Verb::Delete,
        "edit" | "scale" | "restart" | "cordon" | "drain" => Verb::Update,
        "trigger" => Verb::Create,
        _ => Verb::Get,
    }
}

/// Shown in place of a resource's content when the identity may not read it.
#[component]
pub fn DeniedPanel(resource: String, reason: Option<String>) -> Element {
    rsx! {
        div { class: "panel denied", "data-od-id": "denied-panel", role: "alert",
            h2 { class: "panel-title", "You cannot read {resource}" }
            p { class: "denied-reason",
                {reason.unwrap_or_else(|| "The cluster did not grant this identity access.".to_string())}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_verb_with_no_answer_is_denied() {
        let mut permissions = Permissions::deny_all("forbidden");
        permissions.set(Verb::List, true, None);

        assert!(permissions.can_list());
        assert!(!permissions.can_delete());
        assert!(!permissions.can_write());
    }
}
