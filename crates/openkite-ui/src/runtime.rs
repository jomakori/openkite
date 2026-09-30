//! Console state the host publishes and the components read.
//!
//! Every global here is written by a host (at boot, on connect, or on a user
//! action) and read by the shared UI. Dioxus global signals are backed by the
//! runtime, so a host must write them inside the VirtualDom's runtime.

use std::sync::{Arc, Mutex, OnceLock};

use dioxus::prelude::*;
use openkite_api::capability::{Capabilities, GatewayKind};
use openkite_api::gateway::Gateway;
use openkite_api::pod::{LineBuffer, PodObject};
use openkite_api::secret::SecretObject;
use serde_json::Value;

/// The secret the detail slide-over shows (`None` = closed).
pub static SELECTED_SECRET: GlobalSignal<Option<SecretObject>> = Signal::global(|| None);

/// The pod the detail slide-over shows (`None` = closed). Holds the owned
/// contract shape so both hosts can publish a pod without importing a kube
/// type.
pub static SELECTED_POD: GlobalSignal<Option<PodObject>> = Signal::global(|| None);

/// Open the pod slide-over for `pod`. Convenience for the host adapters that
/// own a kube `Pod` — this helper takes the owned contract shape directly so
/// the host never imports Dioxus globals.
pub fn set_selected_pod(pod: Option<PodObject>) {
    *SELECTED_POD.write() = pod;
}

/// Close the pod slide-over.
pub fn clear_selected_pod() {
    set_selected_pod(None);
}

/// Streaming log buffer the log viewer renders. The host populates this when
/// `SELECTED_POD`, the chosen container, or the follow flag changes; the
/// viewer's `use_effect` only reads. Keeping the buffer global means the
/// viewer's mount/unmount is a no-op while logs stream.
pub static LOGS_BUFFER: GlobalSignal<LineBuffer> = Signal::global(LineBuffer::default);

/// Stop the host's log stream and clear the buffer. Idempotent — the host
/// adapter is the authority on what is actually running.
pub fn reset_logs_buffer() {
    LOGS_BUFFER.write().clear();
}

/// The container the log viewer is currently streaming. The host's stream
/// controller watches this and switches the kube stream on change. Stored as
/// a global so the controller survives the viewer's mount/unmount lifecycle.
pub static LOGS_CONTAINER: GlobalSignal<String> = Signal::global(String::new);

/// Whether the viewer is in follow-tail mode. Same rationale as
/// [`LOGS_CONTAINER`]: a global so the host-side stream controller can
/// pause/resume without owning the Dioxus component tree.
pub static LOGS_FOLLOW: GlobalSignal<bool> = Signal::global(|| true);

/// The resource the CRUD overlay is currently showing, or `None` when the
/// overlay is closed. Dispatched on by
/// [`crate::components::crud_modal::CrudOverlay`].
#[derive(Debug, Clone, PartialEq)]
pub enum CrudTarget {
    /// Edit a live resource; the editor round-trips through `ApiRequest::Get`
    /// and pre-loads with the current manifest.
    Edit { doc: Value, kind: String },
    /// Destructive confirm modal (typed-name gate).
    Delete {
        kind: String,
        namespace: Option<String>,
        name: String,
    },
    /// Non-destructive scale confirm (number input + 2-button row).
    Scale {
        kind: String,
        namespace: Option<String>,
        name: String,
        current_replicas: u32,
    },
    /// Create a new resource; the editor opens with a kind-specific starter.
    New { kind: String },
}

pub static CRUD_TARGET: GlobalSignal<Option<CrudTarget>> = Signal::global(|| None);

/// Open the overlay for one of the four CRUD operations. `None` closes it.
pub fn set_crud_target(target: Option<CrudTarget>) {
    *CRUD_TARGET.write() = target;
}

/// Close the overlay.
pub fn clear_crud_target() {
    set_crud_target(None);
}

/// Open the editor for an existing resource.
pub fn open_editor_for(kind: String, doc: Value) {
    set_crud_target(Some(CrudTarget::Edit { kind, doc }));
}

/// Open the destructive confirm modal.
pub fn open_delete_for(kind: String, namespace: Option<String>, name: String) {
    set_crud_target(Some(CrudTarget::Delete {
        kind,
        namespace,
        name,
    }));
}

/// Open the scale modal.
pub fn open_scale_for(
    kind: String,
    namespace: Option<String>,
    name: String,
    current_replicas: u32,
) {
    set_crud_target(Some(CrudTarget::Scale {
        kind,
        namespace,
        name,
        current_replicas,
    }));
}

/// Open the editor for a brand-new resource.
pub fn open_new_for(kind: String) {
    set_crud_target(Some(CrudTarget::New { kind }));
}

/// The host's gateway, installed at boot (and on every connect). `None` means
/// no cluster is reachable, which is what the CRUD overlay reports verbatim.
static GATEWAY: OnceLock<Mutex<Option<Arc<dyn Gateway>>>> = OnceLock::new();

/// The descriptor the host itself publishes at boot, before any cluster
/// connect. Capabilities are a property of the host, so they must not wait
/// for a gateway; the gateway's own value remains the fallback.
static PUBLISHED_CAPABILITIES: OnceLock<Mutex<Option<Capabilities>>> = OnceLock::new();

/// Publish what the host itself can do, independently of any cluster connect.
pub fn set_published_capabilities(caps: Option<Capabilities>) {
    let slot = PUBLISHED_CAPABILITIES.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = caps;
    }
}

/// Install the host's gateway, replacing any previous one.
pub fn set_gateway(gateway: Option<Arc<dyn Gateway>>) {
    let slot = GATEWAY.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = gateway;
    }
}

/// The host's gateway, if one is installed.
pub fn gateway() -> Option<Arc<dyn Gateway>> {
    GATEWAY
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|slot| slot.clone()))
}

/// What the host can do: the published descriptor when the host has stated
/// one, else the gateway's own verdict once a gateway exists.
pub fn capabilities() -> Option<Capabilities> {
    if let Some(slot) = PUBLISHED_CAPABILITIES.get() {
        if let Ok(slot) = slot.lock() {
            if slot.is_some() {
                return *slot;
            }
        }
    }
    gateway().map(|gateway| gateway.capabilities())
}

/// True when the host advertises the standalone terminal surface.
pub fn terminal_can_render() -> bool {
    capabilities()
        .map(|caps| caps.supports_terminal())
        .unwrap_or(false)
}

/// True when the host can list and switch cluster contexts.
///
/// The in-process gateway owns a kubeconfig, so the desktop can offer the
/// context list; a server-side host serves exactly one cluster and has no
/// contexts to switch between, so its sidebar cluster button renders
/// read-only instead of opening a menu it cannot fill.
pub fn cluster_switch_can_render() -> bool {
    capabilities()
        .map(|caps| caps.gateway == GatewayKind::InProcess)
        .unwrap_or(false)
}

/// True when the host reports either window menu bar or title-bar override.
pub fn native_chrome_can_render() -> bool {
    capabilities()
        .map(|caps| caps.supports_native_menu_bar() || caps.supports_title_bar_override())
        .unwrap_or(false)
}
