//! Plugin data types shared by the UI crate and both hosts: the nav/status
//! entries a plugin registers and the control declarations its surface
//! offers.
//!
//! Extracted from `openkite_ui::plugin_api` unchanged — field names, derives
//! and doc comments are the originals. Types that depend on other
//! `openkite-ui` modules (`PluginRegistration::validate` needs
//! `plugin_css::MAX_STYLE_BYTES`, and `RegistrationStore` is rendered
//! straight from UI state) stay in `openkite-ui` rather than drag that
//! coupling into this crate.

use serde::{Deserialize, Serialize};

/// A sidebar entry a plugin contributes (shown under a plugin section).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarItem {
    pub label: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub route: String,
}

/// A routed plugin view (`route` must start with `/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteSpec {
    pub path: String,
    #[serde(default)]
    pub title: String,
}

/// A status-bar widget a plugin registers (label + optional dot color).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusItem {
    pub label: String,
    #[serde(default)]
    pub color: String,
}

/// A renderer declaration: the plugin says "I render body content for this
/// path" so the host's `Route::Plugin` wildcard can route into the JS slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererSpec {
    pub path: String,
}

/// A control a plugin's surface offers, and — when the plugin cannot honour it
/// on this host — why not (plugin system v1.1).
///
/// The plugin owns the label and the action id; the crate owns the button, so
/// a control the plugin declares it cannot support still shows up on the
/// surface, disabled, with `data-unsupported` and the reason — instead of
/// disappearing and leaving the user to guess.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginControl {
    pub label: String,
    /// What the host's handler switches on when it wires this control.
    #[serde(default)]
    pub action: String,
    /// Why the plugin cannot honour this control here. `None` = it can, and
    /// the control is live once the host wires a handler.
    #[serde(default)]
    pub unsupported: Option<String>,
    /// The plugin that declared it, stamped by the registry on registration.
    /// The crate attributes the painted button with it, so the plugin's own
    /// scoped sheet can reach controls the host renders for it.
    #[serde(default)]
    pub plugin: String,
}
