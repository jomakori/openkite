use dioxus::prelude::Element;

/// A route a plugin handles. Merged into the app router via a wildcard
/// dispatcher (Dioxus Router is compile-time; plugin routes resolve at
/// runtime through a route lookup table).
pub struct PluginRoute {
    /// e.g. "/argocd/apps/:name"
    pub path: String,
    /// Dioxus component function for this route.
    pub render: fn() -> Element,
}
