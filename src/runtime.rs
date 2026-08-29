//! Shared runtime state bridging `run()` (bootstrap) to the UI views.

use dioxus::prelude::*;
use kube::Client;
use std::sync::{Arc, OnceLock};

use crate::bridge::Bridge;

/// The active cluster client, published by `run()` after connect and read by
/// views that need a live `Api`.
pub static CLIENT: GlobalSignal<Option<Client>> = Signal::global(|| None);

/// The active context name, published by `run()` (status bar/footer).
pub static CONTEXT: GlobalSignal<Option<String>> = Signal::global(|| None);

/// Mirror of the bridge's registration store: refreshed by the
/// `/openkite` asset handler after register POSTs; the sidebar and status
/// footer render from it.
pub static REGISTRATIONS: GlobalSignal<crate::plugin_api::RegistrationStore> =
    Signal::global(crate::plugin_api::RegistrationStore::new);

/// The plugin bridge, shared between bootstrap and the app shell's asset
/// handler. `OnceLock`: set once before launch, read from the
/// handler thread and the UI alike; never swapped in place.
static BRIDGE: OnceLock<Arc<Bridge>> = OnceLock::new();

/// Publish the active client (or `None` when disconnected).
pub fn set_client(client: Option<Client>) {
    *CLIENT.write() = client;
}

/// Publish the active context name (or `None` when disconnected).
pub fn set_context(name: Option<String>) {
    *CONTEXT.write() = name;
}

/// The current client, if connected.
pub fn client() -> Option<Client> {
    CLIENT.read().clone()
}

/// The current context name, if a kubeconfig is loaded.
pub fn context_name() -> Option<String> {
    CONTEXT.read().clone()
}

/// Install the plugin bridge (idempotent: first write wins).
pub fn set_bridge(bridge: Bridge) {
    let _ = BRIDGE.set(Arc::new(bridge));
}

/// The shared bridge, if bootstrap has installed it.
pub fn bridge() -> Option<Arc<Bridge>> {
    BRIDGE.get().cloned()
}

/// Discovered JS plugin bundles for the shell to eval (set once in `run`).
static JS_PLUGINS: OnceLock<Vec<crate::plugin_js::JsBundle>> = OnceLock::new();

/// Publish the discovered JS plugin bundles (idempotent: first write wins).
pub fn set_js_plugins(bundles: Vec<crate::plugin_js::JsBundle>) {
    let _ = JS_PLUGINS.set(bundles);
}

/// The discovered JS plugin bundles (empty before bootstrap).
pub fn js_plugins() -> Vec<crate::plugin_js::JsBundle> {
    JS_PLUGINS.get().cloned().unwrap_or_default()
}
