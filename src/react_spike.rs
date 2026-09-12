//! OKT-67 spike: React 19 + Tailwind 4 mounted in the existing wry webview.
//!
//! Additive host half of the spike. It reuses the sanctioned desktop
//! custom-protocol transport (a dioxus asset handler hit by same-origin
//! `fetch`) and leaves the plugin bridge (`/openkite`), kube reflect/watch,
//! and the plugin host untouched:
//!
//! - the React app reads its kube snapshot through the EXISTING
//!   `window.openkite.api.list` bridge;
//! - `/openkite-spike` is a read-only spike endpoint that answers the active
//!   kubeconfig context, because the existing op set has no equivalent.
//!
//! The bundle is vendored under `assets/vendored/openkite-react-spike/` (the
//! same convention as `tools/build-xterm`) so `include_str!` sees a committed
//! file at compile time.

use dioxus::desktop::wry;
use dioxus::desktop::{use_asset_handler, AssetRequest, RequestAsyncResponder};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::config::{MenuBarVisibility, OpenKiteConfig, TitleBarTheme};
use crate::plugin_api::ApiResponse;
use crate::router::json_response;

const SPIKE_JS: &str = include_str!("../assets/vendored/openkite-react-spike/app.js");
const SPIKE_CSS: &str = include_str!("../assets/vendored/openkite-react-spike/app.css");

/// The spike-only request ops; responses reuse the bridge's [`ApiResponse`].
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum SpikeRequest {
    Context,
    /// Read the persisted settings snapshot.
    SettingsGet,
    /// Persist a full settings snapshot and apply the OS-level parts.
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

#[derive(Debug, Serialize)]
struct SpikeContext {
    context: Option<String>,
    connected: bool,
    version: String,
    /// Whether cluster mutations are wired (crud::apply_mutation is Phase 1).
    mutations: bool,
}

/// The settings payload the React preferences surface renders.
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

/// The persisted settings plus the derived catalog/capability/metadata the
/// surface needs.
fn settings_snapshot() -> SettingsSnapshot {
    let config = OpenKiteConfig::load();
    let theme_vars = crate::theme::resolve(config.theme.as_deref())
        .vars()
        .clone();
    let themes = crate::theme_catalog::catalog()
        .into_iter()
        .map(|entry| ThemeOption {
            id: entry.id,
            name: entry.display_name,
            variant: entry.variant,
            is_default: entry.is_default,
        })
        .collect();
    SettingsSnapshot {
        theme: config.theme,
        font_size: config.font_size,
        metrics_enabled: config.metrics_enabled,
        menu_bar: config.menu_bar,
        title_bar_theme: config.title_bar_theme,
        theme_vars,
        themes,
        menu_bar_hideable: crate::menubar::hideable(),
        title_bar_overridable: crate::titlebar::overridable(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        // `crud::apply_mutation` is the Phase-1 placeholder, so mutations are
        // off; list-based logs/events are served today.
        capabilities: Capabilities {
            mutations: false,
            logs: true,
            events: true,
        },
    }
}

/// Reject a font size the UI cannot render; `None` keeps the default.
fn validate_font_size(size: Option<u16>) -> Result<(), String> {
    match size {
        Some(size) if !(9..=24).contains(&size) => Err(format!("font size {size} is outside 9-24")),
        _ => Ok(()),
    }
}

/// Persist a full snapshot, then ask the Dioxus side to apply the OS parts.
///
/// The apply is a ping, not a direct call: this runs on the asset-handler
/// thread, where `desktop::window()` is unavailable.
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
    crate::runtime::request_settings_apply();
    Ok(settings_snapshot())
}

/// Mount the React console: register its context endpoint, render the
/// container, then evaluate the vendored bundle once (the bundle self-mounts).
///
/// `route` is the console nav id that matches the host route; changes are
/// pushed into the already-running bundle so host-side navigation (e.g. the
/// command palette) re-points the console without a remount. The bundle also
/// reads the `data-console-route` attribute on first mount.
#[component]
pub fn ReactConsole(route: String) -> Element {
    use_asset_handler(
        "openkite-spike",
        |req: AssetRequest, responder: RequestAsyncResponder| {
            dispatch(req, responder);
        },
    );

    use_effect(move || {
        document::eval(SPIKE_JS);
    });

    use_effect(use_reactive((&route,), |(route,)| {
        let source = format!(
            "window.__openkite_react_console && \
             window.__openkite_react_console.setRoute({route:?});"
        );
        document::eval(&source);
    }));

    rsx! {
        style { dangerous_inner_html: SPIKE_CSS }
        div {
            id: "openkite-react-spike-root",
            class: "openkite-react-spike",
            "data-console-route": "{route}",
        }
    }
}

/// Answer one `/openkite-spike` POST. Never panics; errors are answered.
fn dispatch(req: AssetRequest, responder: RequestAsyncResponder) {
    if req.method() != wry::http::Method::POST {
        responder.respond(json_response(ApiResponse::Error {
            error: "method not allowed: spike requests must be POST".into(),
        }));
        return;
    }
    let Ok(text) = std::str::from_utf8(req.body()) else {
        responder.respond(json_response(ApiResponse::Error {
            error: "request body is not utf-8".into(),
        }));
        return;
    };
    let outcome = match serde_json::from_str::<SpikeRequest>(text) {
        Ok(SpikeRequest::Context) => serde_json::to_value(SpikeContext {
            context: crate::runtime::context_name(),
            connected: crate::runtime::client().is_some(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            mutations: false,
        })
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
    let response = match outcome {
        Ok(result) => ApiResponse::Ok { result },
        Err(error) => ApiResponse::Error { error },
    };
    responder.respond(json_response(response));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_payload_carries_cluster_connection_and_version() {
        let value = serde_json::to_value(SpikeContext {
            context: Some("prod-us-east-1".into()),
            connected: true,
            version: "0.1.0".into(),
            mutations: false,
        })
        .unwrap();
        assert_eq!(value["context"], "prod-us-east-1");
        assert_eq!(value["connected"].as_bool(), Some(true));
        assert_eq!(value["version"], "0.1.0");
        assert_eq!(value["mutations"].as_bool(), Some(false));
    }

    #[test]
    fn context_payload_serializes_a_disconnected_shell() {
        let value = serde_json::to_value(SpikeContext {
            context: None,
            connected: false,
            version: "0.1.0".into(),
            mutations: false,
        })
        .unwrap();
        assert!(value["context"].is_null());
        assert_eq!(value["connected"].as_bool(), Some(false));
    }

    #[test]
    fn settings_set_request_parses_camel_case_fields() {
        let json = r#"{"op":"settings_set","theme":"catppuccin-mocha","fontSize":14,
            "metricsEnabled":false,"menuBar":"hide","titleBarTheme":"dark"}"#;
        match serde_json::from_str::<SpikeRequest>(json).unwrap() {
            SpikeRequest::SettingsSet {
                theme,
                font_size,
                metrics_enabled,
                menu_bar,
                title_bar_theme,
            } => {
                assert_eq!(theme.as_deref(), Some("catppuccin-mocha"));
                assert_eq!(font_size, Some(14));
                assert!(!metrics_enabled);
                assert_eq!(menu_bar, MenuBarVisibility::Hide);
                assert_eq!(title_bar_theme, TitleBarTheme::Dark);
            }
            other => panic!("expected SettingsSet, got {other:?}"),
        }
    }

    #[test]
    fn settings_get_request_parses_the_tagged_op() {
        assert!(matches!(
            serde_json::from_str::<SpikeRequest>(r#"{"op":"settings_get"}"#).unwrap(),
            SpikeRequest::SettingsGet
        ));
    }

    #[test]
    fn font_size_validation_bounds() {
        assert!(validate_font_size(None).is_ok());
        assert!(validate_font_size(Some(9)).is_ok());
        assert!(validate_font_size(Some(24)).is_ok());
        assert!(validate_font_size(Some(8)).is_err());
        assert!(validate_font_size(Some(25)).is_err());
    }

    #[test]
    fn theme_option_serialises_the_catalog_shape() {
        let value = serde_json::to_value(ThemeOption {
            id: "one-dark".into(),
            name: "One Dark".into(),
            variant: "dark".into(),
            is_default: true,
        })
        .unwrap();
        assert_eq!(value["id"], "one-dark");
        assert_eq!(value["name"], "One Dark");
        assert_eq!(value["variant"], "dark");
        assert_eq!(value["isDefault"].as_bool(), Some(true));
    }
}
