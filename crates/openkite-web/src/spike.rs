//! The console's context and settings ops (`/openkite-spike`).
//!
//! The desktop answers these from `openkite::react_spike`, which is
//! desktop-gated because it hangs off the wry asset handler. The payloads are
//! the console's wire contract (`web/src/bridge.ts::fetchClusterContext` and
//! `web/src/settings.ts`), and the persisted fields are the same
//! `OpenKiteConfig`, so both hosts answer field for field.
//!
//! What a server has no counterpart for, it says so: there is no menu bar and no
//! window chrome here, so `menuBarHideable` and `titleBarOverridable` are false
//! and the console gates those controls off.

use std::collections::BTreeMap;

use openkite::config::{MenuBarVisibility, OpenKiteConfig, TitleBarTheme};
use openkite::plugin_api::ApiResponse;
use openkite::{theme, theme_catalog};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One `/openkite-spike` request.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum SpikeRequest {
    Context,
    /// Read the persisted settings snapshot.
    SettingsGet,
    /// Persist a full settings snapshot.
    SettingsSet {
        #[serde(default)]
        theme: Option<String>,
        #[serde(default, rename = "fontSize")]
        font_size: Option<u16>,
        #[serde(default, rename = "metricsEnabled")]
        metrics_enabled: bool,
        #[serde(rename = "menuBar")]
        menu_bar: MenuBarVisibility,
        #[serde(rename = "titleBarTheme")]
        title_bar_theme: TitleBarTheme,
    },
}

/// The cluster the host is reading, for the console's status surface.
#[derive(Debug, Serialize)]
struct SpikeContext {
    context: Option<String>,
    connected: bool,
    version: String,
    /// Whether cluster mutations are reachable. The bridge's op set is read-only
    /// — `exec` answers "not supported yet" and there is no apply or delete — so
    /// this is false on every host.
    mutations: bool,
}

/// The settings payload the console's preferences surface renders.
#[derive(Debug, Serialize)]
struct SettingsSnapshot {
    theme: Option<String>,
    #[serde(rename = "fontSize")]
    font_size: Option<u16>,
    #[serde(rename = "metricsEnabled")]
    metrics_enabled: bool,
    #[serde(rename = "menuBar")]
    menu_bar: MenuBarVisibility,
    #[serde(rename = "titleBarTheme")]
    title_bar_theme: TitleBarTheme,
    #[serde(rename = "themeVars")]
    theme_vars: BTreeMap<String, String>,
    themes: Vec<ThemeOption>,
    #[serde(rename = "menuBarHideable")]
    menu_bar_hideable: bool,
    #[serde(rename = "titleBarOverridable")]
    title_bar_overridable: bool,
    version: String,
    capabilities: Capabilities,
}

/// One theme catalog entry offered by the Appearance picker.
#[derive(Debug, Serialize)]
struct ThemeOption {
    id: String,
    name: String,
    variant: String,
    #[serde(rename = "isDefault")]
    is_default: bool,
}

/// What the backend can honour today, so the console can gate controls instead
/// of offering silent no-ops.
#[derive(Debug, Serialize)]
struct Capabilities {
    mutations: bool,
    logs: bool,
    events: bool,
}

/// Answer one `/openkite-spike` POST. Never panics; errors are answered.
pub fn handle(body: &str, connected: bool) -> ApiResponse {
    let outcome: Result<Value, String> = match serde_json::from_str::<SpikeRequest>(body) {
        Ok(SpikeRequest::Context) => serde_json::to_value(context_payload(connected))
            .map_err(|err| format!("serialize context: {err}")),
        Ok(SpikeRequest::SettingsGet) => serde_json::to_value(settings_snapshot())
            .map_err(|err| format!("serialize settings: {err}")),
        Ok(SpikeRequest::SettingsSet {
            theme,
            font_size,
            metrics_enabled,
            menu_bar,
            title_bar_theme,
        }) => apply_settings(theme, font_size, metrics_enabled, menu_bar, title_bar_theme)
            .and_then(|snapshot| {
                serde_json::to_value(snapshot).map_err(|err| format!("serialize settings: {err}"))
            }),
        Err(err) => Err(format!("parse spike request: {err}")),
    };
    match outcome {
        Ok(result) => ApiResponse::Ok { result },
        Err(error) => ApiResponse::Error { error },
    }
}

/// The cluster context label the console shows.
fn context_payload(connected: bool) -> SpikeContext {
    SpikeContext {
        context: Some(identity_label()),
        connected,
        version: env!("CARGO_PKG_VERSION").to_string(),
        mutations: false,
    }
}

/// The identity the host reads the cluster with.
///
/// In-cluster that is the pod's ServiceAccount token, which has no kubeconfig
/// context name; the label names that source rather than inventing a cluster.
fn identity_label() -> String {
    if crate::in_cluster() {
        return "in-cluster".to_string();
    }
    kube::config::Kubeconfig::read()
        .ok()
        .and_then(|config| config.current_context)
        .unwrap_or_else(|| "kubeconfig".to_string())
}

/// The persisted settings plus the derived catalog, capability and metadata the
/// surface needs.
fn settings_snapshot() -> SettingsSnapshot {
    let config = OpenKiteConfig::load();
    SettingsSnapshot {
        theme_vars: theme::resolve(config.theme.as_deref()).vars().clone(),
        themes: theme_catalog::catalog()
            .into_iter()
            .map(|entry| ThemeOption {
                id: entry.id,
                name: entry.display_name,
                variant: entry.variant,
                is_default: entry.is_default,
            })
            .collect(),
        theme: config.theme,
        font_size: config.font_size,
        metrics_enabled: config.metrics_enabled,
        menu_bar: config.menu_bar,
        title_bar_theme: config.title_bar_theme,
        menu_bar_hideable: false,
        title_bar_overridable: false,
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: Capabilities {
            mutations: false,
            logs: true,
            events: true,
        },
    }
}

/// Validate and persist a settings snapshot.
///
/// The OS-level half of the desktop's apply (menu bar visibility, title-bar
/// theme) has no server counterpart, so the persisted snapshot is the whole
/// outcome here.
fn apply_settings(
    theme: Option<String>,
    font_size: Option<u16>,
    metrics_enabled: bool,
    menu_bar: MenuBarVisibility,
    title_bar_theme: TitleBarTheme,
) -> Result<SettingsSnapshot, String> {
    validate_font_size(font_size)?;
    let mut config = OpenKiteConfig::load();
    config.theme = theme;
    config.font_size = font_size;
    config.metrics_enabled = metrics_enabled;
    config.menu_bar = menu_bar;
    config.title_bar_theme = title_bar_theme;
    config
        .save()
        .map_err(|err| format!("save settings: {err}"))?;
    Ok(settings_snapshot())
}

/// Reject a font size the UI cannot render; `None` keeps the default.
fn validate_font_size(size: Option<u16>) -> Result<(), String> {
    match size {
        Some(size) if !(9..=24).contains(&size) => Err(format!("font size {size} is outside 9-24")),
        _ => Ok(()),
    }
}
